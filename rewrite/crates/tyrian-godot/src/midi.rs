//! MIDI music (ADR 0130): the `data/midi/NN-*.mid` tracks played with the
//! SoundFont `data/soundfont.sf2`, like the "Enhanced" engine of the C
//! version (`midi_music.c`, FluidSynth). The synth is `rustysynth` (a
//! SoundFont synth and MIDI sequencer in Rust, MIT license), not a home-made synth;
//! `midly` (public domain) reads the tempo map to place the loop
//! point.
//!
//! The client (`music.gd`) pulls the samples through `render` and pushes them
//! into an `AudioStreamGenerator`; nothing here is read by the simulation.

use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use godot::prelude::*;
use rustysynth::{MidiFile, MidiFileLoopType, MidiFileSequencer, SoundFont, Synthesizer, SynthesizerSettings};

/// Render rate (midi_music.c: synth.sample-rate 44,100).
pub const SAMPLE_RATE: i32 = 44_100;

/// Fixed synth gain, the only level stage on this side: the music
/// slider is applied by the Godot player (dB table of the C, without the x2
/// of the OPL). Equal to `midi_music.c:MIDI_SYNTH_GAIN` (0.18). Highest peak
/// measured by `tests::loudest_tracks_do_not_clip`: 0.39 (track 1).
pub const MASTER_VOLUME: f32 = 0.18;

/// Polyphony: maximum accepted by rustysynth (the C takes 512 with
/// FluidSynth; 256 voices are enough for the 41 tracks).
const POLYPHONY: usize = 256;

/// File of track `song` (0-based, like `play_song`) in `dir`: the
/// first name starting with "NN-" (`midi_music.c:midi_play_song`, prefix
/// "%02d-"), in alphabetical order so as not to depend on the order of the
/// file system.
pub fn song_file(dir: &Path, song: i32) -> Option<PathBuf> {
    if song < 0 {
        return None;
    }
    let prefix = format!("{song:02}-");
    let mut names: Vec<PathBuf> = std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(&prefix)))
        .collect();
    names.sort();
    names.into_iter().next()
}

/// MIDI tick played at `seconds` (tempo map of all the tracks, initial tempo
/// of 120 per quarter note like rustysynth); None if the file is not
/// in metric time or is unreadable.
fn tick_at(bytes: &[u8], seconds: f64) -> Option<usize> {
    use midly::{MetaMessage, Smf, Timing, TrackEventKind};
    let smf = Smf::parse(bytes).ok()?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int() as f64,
        Timing::Timecode(..) => return None,
    };
    let mut tempos: Vec<(u64, f64)> = Vec::new();
    for track in &smf.tracks {
        let mut t = 0u64;
        for ev in track {
            t += ev.delta.as_int() as u64;
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = ev.kind {
                tempos.push((t, us.as_int() as f64));
            }
        }
    }
    tempos.sort_by_key(|x| x.0);
    let (mut tick, mut time, mut us) = (0u64, 0.0f64, 500_000.0f64);
    for (t, u) in tempos {
        let dt = (t - tick) as f64 * us / 1e6 / ppq;
        if time + dt >= seconds {
            break;
        }
        time += dt;
        tick = t;
        us = u;
    }
    Some((tick as f64 + (seconds - time) * 1e6 * ppq / us).round() as usize)
}

/// Synth loaded with the SoundFont and current track.
pub struct MidiMusic {
    sequencer: MidiFileSequencer,
    looping: bool,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl MidiMusic {
    pub fn new(soundfont: &Path) -> Result<Self, String> {
        let mut reader = BufReader::new(File::open(soundfont).map_err(|e| format!("{}: {e}", soundfont.display()))?);
        let font = Arc::new(SoundFont::new(&mut reader).map_err(|e| format!("{}: {e:?}", soundfont.display()))?);
        let mut settings = SynthesizerSettings::new(SAMPLE_RATE);
        settings.maximum_polyphony = POLYPHONY;
        // midi_music.c: chorus and reverb active.
        settings.enable_reverb_and_chorus = true;
        let mut synth = Synthesizer::new(&font, &settings).map_err(|e| format!("synth: {e:?}"))?;
        synth.set_master_volume(MASTER_VOLUME);
        Ok(Self { sequencer: MidiFileSequencer::new(synth), looping: false, left: Vec::new(), right: Vec::new() })
    }

    /// Plays `midi` from the start. `looping`: at the end of the file, resumes
    /// at `loop_from` seconds (0 = start, like fluid_player_set_loop(-1));
    /// otherwise plays once. A loop point outside the file goes back to the
    /// start.
    pub fn play(&mut self, midi: &Path, looping: bool, loop_from: f64) -> Result<(), String> {
        let bytes = std::fs::read(midi).map_err(|e| format!("{}: {e}", midi.display()))?;
        let parse = |tick: usize| {
            MidiFile::new_with_loop_type(&mut Cursor::new(&bytes), MidiFileLoopType::LoopPoint(tick))
                .map_err(|e| format!("{}: {e:?}", midi.display()))
        };
        let mut file = parse(0)?;
        if looping && loop_from > 0.0 && loop_from < file.get_length() {
            if let Some(tick) = tick_at(&bytes, loop_from).filter(|&t| t > 0) {
                file = parse(tick)?;
            }
        }
        self.looping = looping;
        self.sequencer.play(&Arc::new(file), looping);
        Ok(())
    }

    pub fn stop(&mut self) {
        self.sequencer.stop();
    }

    /// `frames` stereo samples (left, right), from -1 to 1.
    pub fn render(&mut self, frames: usize) -> (&[f32], &[f32]) {
        self.left.resize(frames, 0.0);
        self.right.resize(frames, 0.0);
        self.sequencer.render(&mut self.left, &mut self.right);
        (&self.left, &self.right)
    }

    /// Position in the file, in seconds (goes back to the loop point).
    pub fn position(&self) -> f64 {
        self.sequencer.get_position()
    }

    /// End of the file reached (never for a looping track):
    /// `midi_is_done`, read by `Music.is_playing` (lds_play.c:playing).
    pub fn is_done(&self) -> bool {
        !self.looping && self.sequencer.end_of_sequence()
    }
}

/// MIDI synth as seen from GDScript (`Music` autoload).
#[derive(GodotClass)]
#[class(init, base = RefCounted)]
pub struct TyrianMidi {
    music: Option<MidiMusic>,
}

#[godot_api]
impl TyrianMidi {
    #[constant]
    const SAMPLE_RATE: i32 = SAMPLE_RATE;

    /// Loads the SoundFont; false (with a warning) if it is missing
    /// or unreadable: the client then falls back to the OPL.
    #[func]
    fn load_soundfont(&mut self, path: GString) -> bool {
        match MidiMusic::new(Path::new(&path.to_string())) {
            Ok(m) => {
                self.music = Some(m);
                true
            }
            Err(e) => {
                godot_warn!("midi: unreadable SoundFont ({e})");
                self.music = None;
                false
            }
        }
    }

    #[func]
    fn has_soundfont(&self) -> bool {
        self.music.is_some()
    }

    /// Path of the MIDI file of track `song` in `dir` (data/midi), ""
    /// if none.
    #[func]
    fn song_file(dir: GString, song: i32) -> GString {
        crate::midi::song_file(Path::new(&dir.to_string()), song)
            .map(|p| GString::from(p.to_string_lossy().as_ref()))
            .unwrap_or_default()
    }

    /// False (with a warning) if no SoundFont is loaded or if
    /// the file is unreadable.
    #[func]
    fn play_file(&mut self, path: GString, looping: bool, loop_from: f64) -> bool {
        let Some(m) = self.music.as_mut() else { return false };
        match m.play(Path::new(&path.to_string()), looping, loop_from) {
            Ok(()) => true,
            Err(e) => {
                godot_warn!("midi: unreadable file ({e})");
                false
            }
        }
    }

    #[func]
    fn stop(&mut self) {
        if let Some(m) = self.music.as_mut() {
            m.stop();
        }
    }

    /// `frames` stereo frames for `AudioStreamGeneratorPlayback.push_buffer`.
    #[func]
    fn render(&mut self, frames: i32) -> PackedVector2Array {
        let Some(m) = self.music.as_mut() else {
            return PackedVector2Array::new();
        };
        let (l, r) = m.render(frames.max(0) as usize);
        l.iter().zip(r).map(|(&a, &b)| Vector2::new(a, b)).collect()
    }

    #[func]
    fn position(&self) -> f64 {
        self.music.as_ref().map_or(0.0, |m| m.position())
    }

    #[func]
    fn is_done(&self) -> bool {
        self.music.as_ref().is_none_or(|m| m.is_done())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../data")
    }

    fn synth() -> MidiMusic {
        MidiMusic::new(&data().join("soundfont.sf2")).unwrap()
    }

    fn song(n: i32) -> PathBuf {
        song_file(&data().join("midi"), n).unwrap()
    }

    /// (RMS, peak) of `seconds` seconds of rendering.
    fn stats(m: &mut MidiMusic, seconds: f64) -> (f64, f32) {
        let mut sum = 0.0f64;
        let mut peak = 0.0f32;
        let mut n = 0usize;
        let mut left = (seconds * SAMPLE_RATE as f64) as usize;
        while left > 0 {
            let chunk = left.min(4096);
            let (l, r) = m.render(chunk);
            for &s in l.iter().chain(r) {
                assert!(s.is_finite());
                sum += (s as f64) * (s as f64);
                peak = peak.max(s.abs());
                n += 1;
            }
            left -= chunk;
        }
        ((sum / n as f64).sqrt(), peak)
    }

    #[test]
    fn every_song_has_its_midi_file() {
        let dir = data().join("midi");
        for n in 0..41 {
            let f = song_file(&dir, n).unwrap_or_else(|| panic!("track {n} without a .mid"));
            assert!(f.file_name().unwrap().to_str().unwrap().starts_with(&format!("{n:02}-")));
        }
        assert_eq!(song_file(&dir, 41), None);
        assert_eq!(song_file(&dir, -1), None);
        assert_eq!(song_file(&data().join("absent"), 0), None);
    }

    /// In-memory rendering, without an audio device: one second of the title.
    #[test]
    fn title_renders_audible_audio() {
        let mut m = synth();
        m.play(&song(29), true, 0.0).unwrap();
        let (rms, peak) = stats(&mut m, 1.0);
        assert!(rms > 0.001, "RMS {rms}");
        assert!(peak < 1.0, "peak {peak}");
    }

    #[test]
    fn missing_soundfont_and_file_are_errors() {
        assert!(MidiMusic::new(&data().join("absent.sf2")).is_err());
        assert!(MidiMusic::new(&data().join("tyrian.hdt")).is_err());
        let mut m = synth();
        assert!(m.play(&data().join("midi/absent.mid"), true, 0.0).is_err());
        assert!(m.play(&data().join("tyrian.hdt"), true, 0.0).is_err());
    }

    /// A track without a loop (30, Power Up) ends; the same one looping
    /// replays from the start.
    #[test]
    fn one_shot_ends_and_loop_restarts() {
        let mut once = synth();
        once.play(&song(30), false, 0.0).unwrap();
        assert!(!once.is_done());
        stats(&mut once, 12.0);
        assert!(once.is_done());
        let mut looped = synth();
        looped.play(&song(30), true, 0.0).unwrap();
        stats(&mut looped, 12.0);
        assert!(!looped.is_done());
        assert!(looped.position() < 3.0, "position {}", looped.position());
        let (rms, _) = stats(&mut looped, 3.0);
        assert!(rms > 0.001, "silent loop: RMS {rms}");
    }

    /// Camanis (3): the OPL resumes at 25.9 s of the track (music.json: first
    /// pass 116.1 s, next pass 90.2 s); the MIDI (116.0 s) resumes at the same
    /// instant. After 120 s: 25.9 + (120 - 116.0) = 29.9 s.
    #[test]
    fn loop_point_matches_opl() {
        let mut m = synth();
        m.play(&song(3), true, 116.11 - 90.24).unwrap();
        stats(&mut m, 120.0);
        let pos = m.position();
        assert!((29.0..31.0).contains(&pos), "position after the loop: {pos}");
        // Point beyond the end: resumes at the start.
        m.play(&song(30), true, 500.0).unwrap();
        stats(&mut m, 4.0);
        assert!(m.position() < 2.5, "position {}", m.position());
    }

    #[test]
    fn tick_at_follows_tempo_map() {
        let bytes = std::fs::read(song(3)).unwrap();
        assert_eq!(tick_at(&bytes, 0.0), Some(0));
        let a = tick_at(&bytes, 10.0).unwrap();
        let b = tick_at(&bytes, 20.0).unwrap();
        assert!(a > 0 && b > a);
        assert_eq!(tick_at(b"not a midi", 1.0), None);
    }

    /// Level aligned with the C (midi_music.c:MIDI_SYNTH_GAIN = 0.18).
    #[test]
    fn master_volume_matches_c_synth_gain() {
        assert!((MASTER_VOLUME - 0.18).abs() < 1e-6, "{MASTER_VOLUME}");
    }

    /// D3: the file 24-ending-3.mid contains no note before tick
    /// 576 (2.4 s at 120 ppq, 500,000 us/beat): the initial silence comes from the
    /// track, not from the engine. The "-36.5 dB" measured in the C over these 2 s
    /// is the tail (reverb) of the previous track, the C synth
    /// not being reset between two tracks. The track must, however,
    /// sound right after its first note.
    #[test]
    fn song_24_is_silent_only_until_its_first_note() {
        let mut m = synth();
        m.play(&song(24), false, 0.0).unwrap();
        let (head, _) = stats(&mut m, 2.3);
        assert!(head < 1e-6, "nothing before the first note, rms {head}");
        let (rms, peak) = stats(&mut m, 8.0);
        println!("track 24, 2.3 s to 10.3 s: RMS {:.1} dBFS, peak {peak:.4}", 20.0 * rms.log10());
        assert!(rms > 1e-3, "rms {rms}");
    }

    /// Level of the 41 tracks at fixed gain: no peak at full scale
    /// (MASTER_VOLUME measurement; `--nocapture` to read the values).
    #[test]
    fn loudest_tracks_do_not_clip() {
        let mut m = synth();
        let mut worst = 0.0f32;
        for n in 0..41 {
            m.play(&song(n), false, 0.0).unwrap();
            let (rms, peak) = stats(&mut m, 40.0);
            println!("track {n:2}: RMS {:6.1} dBFS, peak {peak:.3}", 20.0 * rms.log10());
            worst = worst.max(peak);
        }
        assert!(worst < 0.95, "peak {worst}");
    }
}
