//! Recording of a `.rep` replay from the player's level
//! (`replay.c:replay_start_recording`, `replay_capture_frame`,
//! `replay_stop_recording`). The rules (key, messages, folder, file
//! name) are in `replay_recording.gd`; here, only the `Sim`.

use godot::prelude::*;

use crate::TyrianSim;

#[godot_api(secondary)]
impl TyrianSim {
    /// To be called before `load_level`: the loaded level will be recorded
    /// (`replay_level_start_hook` with an armed request). `section`:
    /// `mainLevel` (0 = unknown, found again in `levelsN.dat`), `seed`: header
    /// seed of the file, `game_speed`: `gameSpeed` 1..5. Consumed by the next
    /// `load_level`; `is_recording` tells whether it could start.
    #[func]
    fn request_recording(&mut self, section: i32, seed: i64, game_speed: i32) {
        self.record_request = Some((section.clamp(0, 255) as u8, seed as u64, game_speed.clamp(1, 25) as u8));
    }

    /// A recording is in progress in the loaded level.
    #[func]
    fn is_recording(&self) -> bool {
        self.recorder.is_some()
    }

    /// Number of frames captured so far.
    #[func]
    fn recording_frames(&self) -> i64 {
        self.recorder.as_ref().map_or(0, |r| r.frame_count() as i64)
    }

    /// `replay_stop_recording`: ends the recording and returns the contents
    /// of the file (empty if there was none).
    #[func]
    fn finish_recording(&mut self) -> PackedByteArray {
        match self.recorder.take() {
            Some(rec) => PackedByteArray::from(rec.finish().as_slice()),
            None => PackedByteArray::new(),
        }
    }
}
