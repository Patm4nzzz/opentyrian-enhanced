//! Audio effects of the Godot buses that reproduce the C mixer
//! (`loudness.c:audioCallback`). They run in Godot's audio thread:
//! settings go through atomics shared between the resource
//! (`AudioEffect`, main thread) and its instances (audio thread), never through
//! a Godot object.
//!
//! - [`TyrianPseudoStereo`]: Setup > Sound > Music Widen
//!   (`loudness.c:ps_widen`, `music_pseudo_stereo`), on the OPL output;
//! - [`TyrianStereoGain`]: left / right gain of an effects channel
//!   (`loudness.c`, `chanGainL` / `chanGainR` derived from `channelPan`),
//!   Setup > Sound > Stereo.

use std::ffi::c_void;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use godot::classes::native::AudioFrame;
use godot::classes::{AudioEffect, AudioEffectInstance, AudioServer, IAudioEffect, IAudioEffectInstance};
use godot::meta::conv::RawPtr;
use godot::prelude::*;

/// `loudness.c:PS_DELAY_BUF` (power of two).
pub const PS_DELAY_BUF: usize = 64;

/// `loudness.c`: delay ~ 0.6 ms, `audioSampleRate / 1700` bounded to 1..63.
pub fn ps_delay_frames(mix_rate: f32) -> usize {
    ((mix_rate as i64) / 1700).clamp(1, PS_DELAY_BUF as i64 - 1) as usize
}

/// State of `loudness.c:ps_widen`: ring of the mono signal and cursor.
#[derive(Debug, Clone)]
pub struct PseudoStereo {
    ring: [f32; PS_DELAY_BUF],
    widx: usize,
    delay: usize,
}

impl PseudoStereo {
    /// Empty ring (the C resets it to zero on switching to "enabled",
    /// `music_pseudo_stereo_set`).
    pub fn new(delay: usize) -> Self {
        Self { ring: [0.0; PS_DELAY_BUF], widx: 0, delay: delay.clamp(1, PS_DELAY_BUF - 1) }
    }

    /// `loudness.c:ps_widen`: l = (2x + xd) / 3, r = (2xd + x) / 3, xd = x
    /// delayed by `delay` samples. The C integer scales (Sint16,
    /// truncated division) become floats in the Godot mixer.
    pub fn widen(&mut self, x: f32) -> (f32, f32) {
        let xd = self.ring[(self.widx + PS_DELAY_BUF - self.delay) & (PS_DELAY_BUF - 1)];
        self.ring[self.widx & (PS_DELAY_BUF - 1)] = x;
        self.widx = (self.widx + 1) & (PS_DELAY_BUF - 1);
        ((2.0 * x + xd) / 3.0, (2.0 * xd + x) / 3.0)
    }
}

/// `loudness.c`: left / right gains of a channel for a pan of
/// 0..255 (0 left, 128 center, 255 right), scale 0..256 brought down to 0..1.
/// At the center both sides stay at full volume; at the extremes, linear
/// fade of the opposite side.
pub fn pan_gains(pan: u8) -> (f32, f32) {
    let p = pan as i32;
    let l = if p <= 128 { 256 } else { (255 - p) * 256 / 127 };
    let r = if p >= 128 { 256 } else { p * 256 / 128 };
    (l as f32 / 256.0, r as f32 / 256.0)
}

/// `loudness.h:screen_x_to_pan`: screen x (PLAY_AREA_X = 24,
/// PLAY_AREA_WIDTH = 264) -> pan 0..255.
pub fn screen_x_to_pan(x: i32) -> u8 {
    let rel = (x - 24).clamp(0, 264);
    (rel * 255 / 264) as u8
}

/// Applies `f` to each frame: AudioServer passes `frame_count` valid
/// frames in source and destination during the call
/// (audio_server.cpp: bus buffer -> temporary buffer). Read then
/// write frame by frame, through pointers, with no slice that would assume
/// the two buffers are disjoint.
unsafe fn for_each_frame(
    src: RawPtr<*const c_void>,
    dst: RawPtr<*mut AudioFrame>,
    count: i32,
    mut f: impl FnMut(f32, f32) -> (f32, f32),
) {
    let src = src.ptr() as *const AudioFrame;
    let dst = dst.ptr();
    for i in 0..count.max(0) as usize {
        // SAFETY: see the function header.
        unsafe {
            let s = std::ptr::read(src.add(i));
            let (l, r) = f(s.left, s.right);
            std::ptr::write(dst.add(i), AudioFrame { left: l, right: r });
        }
    }
}

// --- Music Widen --------------------------------------------------------------

/// Setup > Sound > Music Widen: pseudo-stereo of the C on the OPL music.
/// The input is reduced to mono ((l + r) / 2; the OPL OGGs are mono, so
/// l = r = x like `oplMonoBuf`).
#[derive(GodotClass)]
#[class(base = AudioEffect)]
pub struct TyrianPseudoStereo {
    base: Base<AudioEffect>,
    /// Delay in frames, computed at creation (main thread): `instantiate`
    /// is called by the audio server when the effect is enabled, and querying
    /// `AudioServer` there blocked the main loop for several seconds.
    delay: usize,
}

#[godot_api]
impl IAudioEffect for TyrianPseudoStereo {
    fn init(base: Base<AudioEffect>) -> Self {
        Self { base, delay: ps_delay_frames(AudioServer::singleton().get_mix_rate()) }
    }

    fn instantiate(&mut self) -> Option<Gd<AudioEffectInstance>> {
        let delay = self.delay;
        let inst = Gd::from_init_fn(|base| TyrianPseudoStereoInstance { base, state: PseudoStereo::new(delay) });
        Some(inst.upcast())
    }
}

#[godot_api]
impl TyrianPseudoStereo {
    /// Delay in frames at the mixer rate (tools, tests).
    #[func]
    fn delay_frames() -> i32 {
        ps_delay_frames(AudioServer::singleton().get_mix_rate()) as i32
    }
}

#[derive(GodotClass)]
#[class(no_init, base = AudioEffectInstance)]
pub struct TyrianPseudoStereoInstance {
    base: Base<AudioEffectInstance>,
    state: PseudoStereo,
}

#[godot_api]
impl IAudioEffectInstance for TyrianPseudoStereoInstance {
    unsafe fn process_rawptr(
        &mut self,
        src_buffer: RawPtr<*const c_void>,
        r_dst_buffer: RawPtr<*mut AudioFrame>,
        frame_count: i32,
    ) {
        let state = &mut self.state;
        unsafe { for_each_frame(src_buffer, r_dst_buffer, frame_count, |l, r| state.widen((l + r) * 0.5)) }
    }
}

// --- Stereo (pan of a channel) -----------------------------------------

#[derive(Debug)]
struct Gains {
    left: AtomicU32,
    right: AtomicU32,
}

impl Gains {
    fn new() -> Self {
        Self { left: AtomicU32::new(1.0f32.to_bits()), right: AtomicU32::new(1.0f32.to_bits()) }
    }

    fn get(&self) -> (f32, f32) {
        (f32::from_bits(self.left.load(Ordering::Relaxed)), f32::from_bits(self.right.load(Ordering::Relaxed)))
    }
}

/// Left / right gain of an effects channel (buses Sfx0..Sfx7), set by
/// `set_pan` at each sound (`multiSamplePlayPanPitch`): the effect applies to the
/// next sound without recreating an instance.
#[derive(GodotClass)]
#[class(base = AudioEffect)]
pub struct TyrianStereoGain {
    base: Base<AudioEffect>,
    gains: Arc<Gains>,
}

#[godot_api]
impl IAudioEffect for TyrianStereoGain {
    fn init(base: Base<AudioEffect>) -> Self {
        Self { base, gains: Arc::new(Gains::new()) }
    }

    fn instantiate(&mut self) -> Option<Gd<AudioEffectInstance>> {
        let gains = self.gains.clone();
        let inst = Gd::from_init_fn(|base| TyrianStereoGainInstance { base, gains });
        Some(inst.upcast())
    }
}

#[godot_api]
impl TyrianStereoGain {
    /// Pan 0..255 of the C (`channelPan`) -> gains of the channel.
    #[func]
    fn set_pan(&mut self, pan: i32) {
        let (l, r) = pan_gains(pan.clamp(0, 255) as u8);
        self.gains.left.store(l.to_bits(), Ordering::Relaxed);
        self.gains.right.store(r.to_bits(), Ordering::Relaxed);
    }

    #[func]
    fn left_gain(&self) -> f32 {
        self.gains.get().0
    }

    #[func]
    fn right_gain(&self) -> f32 {
        self.gains.get().1
    }

    /// `loudness.h:screen_x_to_pan`.
    #[func]
    fn screen_x_to_pan(x: i32) -> i32 {
        screen_x_to_pan(x) as i32
    }
}

#[derive(GodotClass)]
#[class(no_init, base = AudioEffectInstance)]
pub struct TyrianStereoGainInstance {
    base: Base<AudioEffectInstance>,
    gains: Arc<Gains>,
}

#[godot_api]
impl IAudioEffectInstance for TyrianStereoGainInstance {
    unsafe fn process_rawptr(
        &mut self,
        src_buffer: RawPtr<*const c_void>,
        r_dst_buffer: RawPtr<*mut AudioFrame>,
        frame_count: i32,
    ) {
        let (gl, gr) = self.gains.get();
        unsafe { for_each_frame(src_buffer, r_dst_buffer, frame_count, |l, r| (l * gl, r * gr)) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widen_matches_c_formula() {
        assert_eq!(ps_delay_frames(44100.0), 25);
        assert_eq!(ps_delay_frames(48000.0), 28);
        let mut ps = PseudoStereo::new(3);
        // Impulse: at t = 0, l = 2/3, r = 1/3; at t = 3 (delay), l = 1/3, r = 2/3.
        let out: Vec<(f32, f32)> = (0..6).map(|t| ps.widen(if t == 0 { 1.0 } else { 0.0 })).collect();
        assert_eq!(out[0], (2.0 / 3.0, 1.0 / 3.0));
        assert_eq!(out[1], (0.0, 0.0));
        assert_eq!(out[3], (1.0 / 3.0, 2.0 / 3.0));
        // Constant signal: l = r = x once the ring is full.
        let mut ps = PseudoStereo::new(25);
        let mut last = (0.0, 0.0);
        for _ in 0..100 {
            last = ps.widen(0.5);
        }
        assert!((last.0 - 0.5).abs() < 1e-6 && (last.1 - 0.5).abs() < 1e-6);
    }

    #[test]
    fn pan_matches_c_gains() {
        assert_eq!(pan_gains(128), (1.0, 1.0));
        assert_eq!(pan_gains(0), (1.0, 0.0));
        assert_eq!(pan_gains(255), (0.0, 1.0));
        assert_eq!(pan_gains(64), (1.0, 0.5));
        assert_eq!(screen_x_to_pan(160), 131);
        assert_eq!(screen_x_to_pan(0), 0);
        assert_eq!(screen_x_to_pan(400), 255);
    }
}
