//! Queue of the level loop's sound effects (`varz.c:soundQueue`,
//! `soundQueueX`, `soundQueueDmg`, flushed by `loudness.c:JE_flushSoundQueue`).
//!
//! Rendering only: the simulation writes to this queue at the same places
//! as the C code but never reads it; it enters neither the state hash
//! (`Sim::state_hash`, `Sim::snapshot`) nor the golden master. The Godot
//! client drains the emitted sounds (`Sim::take_sounds`) and plays them with
//! panning (`soundQueueX`, Setup > Sound > Stereo) and volume / pitch
//! according to damage (`soundQueueDmg`, SFX Impact Scaling).
//!
//! The C random draws that choose a channel or a sample
//! (`rng_next() % 8`, `% 3`, `% 5`) already exist in the simulation: their
//! results are only re-read here, no draw is added.

use crate::world::World;

/// Number of channels (`loudness.c:CHANNEL_COUNT`, `soundQueue[8]`).
pub const SOUND_CHANNELS: usize = 8;
/// `sndmast.h:SOUND_COUNT` (31 effects + 9 voices).
pub const SOUND_COUNT: u8 = 40;
/// `soundQueueX` at rest: screen center (`tyrian2.c`, `loudness.c`).
pub const SOUND_CENTER_X: u16 = 160;
/// Emitted sounds kept at most when nobody drains the queue (golden
/// master tool, bots): the oldest are forgotten.
const MAX_PLAYED: usize = 256;
/// `level_events.c:voice_dedup_allow`: 3 s of clock, in game loop iterations
/// (28.75 ms each, see level.gd).
const VOICE_RATE_LIMIT_TICKS: u64 = 3000 * 100 / 2875;

/// Samples of `sndmast.h` (1-based) used by the level loop.
pub const S_ENEMY_HIT: u8 = 3;
pub const S_EXPLOSION_4: u8 = 4;
pub const S_WEAPON_7: u8 = 7;
pub const S_EXPLOSION_8: u8 = 8;
pub const S_EXPLOSION_9: u8 = 9;
pub const S_EXPLOSION_11: u8 = 11;
pub const S_CLICK: u8 = 24;
pub const S_WARNING: u8 = 17;
pub const S_ITEM: u8 = 18;
pub const S_HULL_HIT: u8 = 19;
pub const S_SOUL_OF_ZINGLON: u8 = 21;
pub const S_EXPLOSION_22: u8 = 22;
pub const S_CLINK: u8 = 23;
pub const S_WEAPON_26: u8 = 26;
pub const S_SHIELD_HIT: u8 = 27;
pub const S_POWERUP: u8 = 29;
pub const V_GOOD_LUCK: u8 = 35;
pub const V_DATA_CUBE: u8 = 39;
/// `level_events.c:VOICE_SAMPLE_FIRST`.
const VOICE_SAMPLE_FIRST: u8 = 32;
/// `varz.c:randomEnemyLaunchSounds`.
pub const RANDOM_ENEMY_LAUNCH_SOUNDS: [u8; 3] = [13, 6, 26];
/// `sndmast.c:windowTextSamples` (V_DANGER, V_BOSS, V_ENEMIES,
/// V_CLEARED_PLATFORM, V_DANGER, V_SPIKES, V_ACCELERATE, V_DANGER, V_ENEMIES).
pub const WINDOW_TEXT_SAMPLES: [u8; 9] = [37, 33, 34, 32, 37, 38, 40, 37, 34];
/// `shots.c:player_shot_create`, `soundChannel[bay_i]`.
pub const SHOT_SOUND_CHANNEL: [usize; 11] = [0, 2, 4, 4, 2, 2, 5, 5, 1, 4, 1];

/// A sound emitted by `JE_flushSoundQueue`: channel, sample (1-based),
/// screen x (`soundQueueX`, 160 = center) and damage magnitude
/// (`soundQueueDmg`: 0 none, 1 small, 2 big enemy, 3 boss).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SoundEvent {
    pub channel: u8,
    pub sample: u8,
    pub x: u16,
    pub dmg: u16,
}

/// Sound state of the level (rendering only, see the module header).
#[derive(Debug, Clone)]
pub struct SoundQueue {
    pub sample: [u8; SOUND_CHANNELS],
    pub x: [u16; SOUND_CHANNELS],
    pub dmg: [u16; SOUND_CHANNELS],
    /// Sounds emitted since the last `Sim::take_sounds`.
    pub played: Vec<SoundEvent>,
    /// `fonthand.c:warningSoundDelay` (low armor alarm).
    pub warning_sound_delay: u16,
    /// `level_events.c:voice_played_at_event` (index = eventLoc - 1).
    pub voice_played_at_event: Vec<bool>,
    /// `level_events.c:sample_voice_last_ticks`, in iterations (None: never).
    pub sample_last_tick: [Option<u64>; SOUND_COUNT as usize + 1],
}

impl SoundQueue {
    /// `tyrian2.c:JE_main`: "Clear Sound Queue" (center everywhere) then
    /// `soundQueue[3] = V_GOOD_LUCK`; `warningSoundDelay = 0` ;
    /// `level_events_reset_voice_dedup()`.
    pub fn new() -> Self {
        let mut q = Self {
            sample: [0; SOUND_CHANNELS],
            x: [SOUND_CENTER_X; SOUND_CHANNELS],
            dmg: [0; SOUND_CHANNELS],
            played: Vec::new(),
            warning_sound_delay: 0,
            voice_played_at_event: vec![false; crate::world::EVENT_MAXIMUM],
            sample_last_tick: [None; SOUND_COUNT as usize + 1],
        };
        q.sample[3] = V_GOOD_LUCK;
        q
    }
}

impl Default for SoundQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl World {
    /// `soundQueue[chan] = sample` (soundQueueX unchanged).
    pub(crate) fn queue_sound(&mut self, chan: usize, sample: u8) {
        self.sound.sample[chan] = sample;
    }

    /// `soundQueue[chan] = sample; soundQueueX[chan] = (uint16_t)x`.
    pub(crate) fn queue_sound_at(&mut self, chan: usize, sample: u8, x: i32) {
        self.sound.sample[chan] = sample;
        self.sound.x[chan] = x as u16;
    }

    /// `collision.c:collision_process_kill_chain`: explosion sound with
    /// position and magnitude (`soundQueueDmg`).
    pub(crate) fn queue_sound_dmg(&mut self, chan: usize, sample: u8, x: i32, dmg: u16) {
        self.queue_sound_at(chan, sample, x);
        self.sound.dmg[chan] = dmg;
    }

    /// `level_events.c:voice_dedup_allow` (events 16 and 62): a voice
    /// plays only once per script position and no more than once in
    /// 3 s; a scripted effect, no more than once in 3 s. The C wall clock
    /// becomes the iteration count.
    pub(crate) fn voice_dedup_allow(&mut self, sfx: u8, event_loc: u16) -> bool {
        let now = self.iteration;
        let s = &mut self.sound;
        let i = (sfx as usize).min(SOUND_COUNT as usize);
        if let Some(last) = s.sample_last_tick[i] {
            if now.wrapping_sub(last) < VOICE_RATE_LIMIT_TICKS {
                return false;
            }
        }
        if sfx >= VOICE_SAMPLE_FIRST && event_loc > 0 && (event_loc as usize) <= s.voice_played_at_event.len() {
            let idx = event_loc as usize - 1;
            if s.voice_played_at_event[idx] {
                return false;
            }
            s.voice_played_at_event[idx] = true;
        }
        s.sample_last_tick[i] = Some(now);
        true
    }

    /// `loudness.c:JE_flushSoundQueue`: nothing while game over is
    /// reached (`!firstGameOver`); otherwise each busy channel emits its sound
    /// (sample outside 1..40 ignored, like the C guard) then returns to
    /// rest (sample 0, center, no damage).
    pub(crate) fn flush_sound_queue(&mut self) {
        if !self.ending.first_game_over {
            return;
        }
        let s = &mut self.sound;
        for chan in 0..SOUND_CHANNELS {
            let sample = s.sample[chan];
            if sample == 0 {
                continue;
            }
            if sample <= SOUND_COUNT {
                s.played.push(SoundEvent { channel: chan as u8, sample, x: s.x[chan], dmg: s.dmg[chan] });
            }
            s.sample[chan] = 0;
            s.x[chan] = SOUND_CENTER_X;
            s.dmg[chan] = 0;
        }
        if s.played.len() > MAX_PLAYED {
            let extra = s.played.len() - MAX_PLAYED;
            s.played.drain(..extra);
        }
    }
}
