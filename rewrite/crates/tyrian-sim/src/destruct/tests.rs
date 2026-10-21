//! Behavior tests of `destruct.rs`: ballistics of a shot, damage to the
//! terrain, end of round, configuration identical to the C one.

use super::*;

/// Test `VGAScreen`: pixels only, sprite and text drawing
/// counted but not rendered.
struct TestCanvas {
    px: Vec<u8>,
    blits: usize,
    texts: Vec<Vec<u8>>,
}

impl TestCanvas {
    fn new() -> Self {
        TestCanvas { px: vec![0; SCREEN], blits: 0, texts: Vec::new() }
    }
}

impl Canvas for TestCanvas {
    fn pixels(&mut self) -> &mut [u8] {
        &mut self.px
    }
    fn blit_sprite2(&mut self, _x: i32, _y: i32, _index: u32) {
        self.blits += 1;
    }
    fn out_text(&mut self, _x: i32, _y: i32, text: &[u8], _colorbank: u8, _brightness: i32) {
        self.texts.push(text.to_vec());
    }
}

/// No unit left: `units_remaining` also at zero, otherwise
/// `DE_RunTickCycleDeadUnits` searches forever for a valid unit (like the C code).
fn clear_units(d: &mut Destruct) {
    for p in &mut d.players {
        p.units_remaining = 0;
        for u in &mut p.units {
            u.health = 0;
        }
    }
}

fn game(mode: usize, seed: u64) -> (Destruct, TestCanvas) {
    let cfg = DestructConfig { ai: [false, false], ..Default::default() };
    let mut d = Destruct::new(cfg, seed);
    d.reset_players();
    let mut c = TestCanvas::new();
    d.start_round(&mut c, mode);
    (d, c)
}

/// The C `float` constants, written by their bits, do match the
/// literals of destruct.c.
#[test]
fn float_constants_match_c_literals() {
    let pairs: [(F32, f32); 13] = [
        (F_0_0001, 0.0001),
        (F_0_01, 0.01),
        (F_0_02, 0.02),
        (F_0_03, 0.03),
        (F_0_05, 0.05),
        (F_0_1, 0.1),
        (F_0_2, 0.2),
        (F_0_3, 0.3),
        (F_0_5, 0.5),
        (F_0_8, 0.8),
        (F_0_9, 0.9),
        (F_1_5, 1.5),
        (F_9_99, 9.99),
    ];
    for (f, v) in pairs {
        assert_eq!(f.0, v.to_bits(), "{v}");
    }
}

/// `DE_RunTickShots`: a shot without bounce advances by (xmov, ymov) and
/// gravity adds 0.05 to ymov at each tick (C float computations).
#[test]
fn shot_follows_ballistic_curve() {
    let (mut d, mut c) = game(1, 7);
    // Clear sky: no more terrain or units on the trajectory.
    d.temp.iter_mut().for_each(|p| *p = 0);
    clear_units(&mut d);
    d.shots[0] = Shot {
        is_available: false,
        x: fi(100),
        y: fi(60),
        xmov: F32::from_bits(2.0f32.to_bits()),
        ymov: F32::from_bits((-1.5f32).to_bits()),
        shottype: SHOT_SMALL as u32,
        ..Shot::default()
    };
    let (mut x, mut y, mut xm, mut ym) = (100f32, 60f32, 2f32, -1.5f32);
    for _ in 0..40 {
        d.tick_draw(&mut c);
        x += xm;
        y += ym;
        ym += 0.05;
        let s = d.shots[0];
        assert!(!s.is_available);
        assert_eq!((s.x.0, s.y.0, s.xmov.0, s.ymov.0), (x.to_bits(), y.to_bits(), xm.to_bits(), ym.to_bits()));
        xm += 0.0;
    }
    // Apex passed: the shot comes back down.
    assert!(d.shots[0].ymov.gt(F32::ZERO));
}

/// A shot that hits dirt explodes: the explosion digs into the terrain
/// (star pixels 246..255 that fade out to black).
#[test]
fn explosion_destroys_terrain() {
    let (mut d, mut c) = game(1, 11);
    clear_units(&mut d);
    // 40×40 block of dirt under the impact point.
    d.temp.iter_mut().for_each(|p| *p = 0);
    for y in 100..140 {
        for x in 140..180 {
            d.temp[y * W + x] = PIXEL_DIRT;
        }
    }
    let dirt_before = d.temp.iter().filter(|&&p| p == PIXEL_DIRT).count();
    d.shots[0] = Shot {
        is_available: false,
        x: fi(160),
        y: fi(96),
        xmov: F32::ZERO,
        ymov: fi(2),
        shottype: SHOT_LARGE as u32,
        ..Shot::default()
    };
    for _ in 0..5 {
        d.tick_draw(&mut c);
    }
    assert!(d.shots[0].is_available, "the shot hit the dirt");
    for _ in 0..80 {
        d.tick_draw(&mut c);
    }
    assert_eq!(d.active_explosions(), 0, "the explosion is over");
    let dirt_after = d.temp.iter().filter(|&&p| p == PIXEL_DIRT).count();
    assert!(dirt_after + 300 < dirt_before, "crater dug: {dirt_before} -> {dirt_after}");
    assert!(d.temp.iter().all(|&p| p < 241), "the flames have died out");
}

/// An explosion on a unit removes one hit point from it per spark; at
/// zero it is destroyed and the opponent scores a point.
#[test]
fn explosion_damages_and_destroys_unit() {
    let (mut d, mut c) = game(1, 3);
    let u = d.players[PLAYER_RIGHT].units[0];
    assert!(u.is_valid());
    d.players[PLAYER_RIGHT].units[0].health = 3;
    let x = u.unit_x + 5;
    let y = to_u32(fi(u.unit_y.roundf()).sub(fi(5)));
    d.make_explosion(x, y, SHOT_LARGE);
    for _ in 0..40 {
        d.tick_draw(&mut c);
    }
    assert!(!d.players[PLAYER_RIGHT].units[0].is_valid());
    assert_eq!(d.players[PLAYER_RIGHT].units_remaining, 0);
    assert_eq!(d.players[PLAYER_LEFT].score, 1);
}

/// `DE_RunTickCheckEndgame`: a side with no unit ends the round; the
/// winner scores the mode's points, then 80 ticks later
/// `STATE_RELOAD` restarts a map.
#[test]
fn round_ends_eighty_ticks_after_a_side_is_wiped_out() {
    let (mut d, mut c) = game(0, 5);
    let keys = [[false; MAX_MOVE]; MAX_PLAYERS];
    assert_eq!(d.tick_input(keys), TickEnd::Continue);
    d.players[PLAYER_RIGHT].units_remaining = 0;
    d.tick_draw(&mut c);
    assert_eq!(d.tick_input(keys), TickEnd::Continue);
    assert_eq!(d.end_delay(), 80);
    // 5-Card War: ModeScore[PLAYER_RIGHT][0] = 1.
    assert_eq!(d.players[PLAYER_LEFT].score, 1);
    assert!(d.take_sounds().iter().any(|s| s.channel == 7 && s.sample == V_CLEARED_PLATFORM));
    for i in 0..79 {
        d.tick_draw(&mut c);
        assert_eq!(d.tick_input(keys), TickEnd::Continue, "tick {i}");
    }
    d.tick_draw(&mut c);
    assert_eq!(d.tick_input(keys), TickEnd::Reload);
    // Score gained only once.
    assert_eq!(d.players[PLAYER_LEFT].score, 1);
}

/// Fire held: a single shot, then the weapon's delay (`shotDelay`) blocks
/// the following ones; weapon change downwards from Small: Tracer.
#[test]
fn fire_respects_weapon_delay_and_weapon_cycles() {
    let (mut d, mut c) = game(1, 9);
    let mut keys = [[false; MAX_MOVE]; MAX_PLAYERS];
    keys[PLAYER_LEFT][MOVE_FIRE] = true;
    d.tick_draw(&mut c);
    d.tick_input(keys);
    assert_eq!(d.active_shots(), 1);
    let sounds = d.take_sounds();
    assert!(sounds.iter().any(|s| s.channel == 0 && s.sample == SHOT_SOUND[SHOT_SMALL as usize]));
    for _ in 0..(SHOT_DELAY[SHOT_SMALL as usize] - 1) {
        d.tick_draw(&mut c);
        d.tick_input(keys);
    }
    assert!(d.active_shots() <= 1, "no second shot before the delay");
    let mut keys = [[false; MAX_MOVE]; MAX_PLAYERS];
    keys[PLAYER_LEFT][MOVE_CYDN] = true;
    d.tick_draw(&mut c);
    d.tick_input(keys);
    let sel = d.players[PLAYER_LEFT].unit_selected as usize;
    assert_eq!(d.players[PLAYER_LEFT].units[sel].shot_type, SHOT_TRACER);
}

/// Same seed and inputs: same maps and same frames.
#[test]
fn same_seed_same_game() {
    let run = || {
        let cfg = DestructConfig { ai: [true, true], ..Default::default() };
        let mut d = Destruct::new(cfg, 42);
        d.reset_players();
        let mut c = TestCanvas::new();
        d.start_round(&mut c, 0);
        let mut h = 0u64;
        for _ in 0..600 {
            d.tick_draw(&mut c);
            if d.tick_input([[false; MAX_MOVE]; MAX_PLAYERS]) == TickEnd::Reload {
                d.start_round(&mut c, 0);
            }
            for &p in &c.px {
                h = (h ^ p as u64).wrapping_mul(0x0100_0000_01b3);
            }
        }
        (h, d.rng.state_hash(), d.players[0].score, d.players[1].score)
    };
    assert_eq!(run(), run());
}

/// Terrain: heights clamped to [40, 195], dirt drawn below the line.
#[test]
fn terrain_is_bounded_and_drawn() {
    for seed in 1..40 {
        let (d, c) = game(0, seed);
        for x in 1..=318 {
            let h = d.base_height(x);
            assert!((40..=195).contains(&h) || h >= BASE_MAP_ALIAS - 100, "x={x} h={h}");
            // The holes (MAP_HOLES) pierce the dirt after it is drawn.
            if h <= 195 && d.map_flags & MAP_HOLES == 0 {
                assert_eq!(c.px[199 * W + x], PIXEL_DIRT);
            }
        }
    }
}

/// `destruct.cfg` written by the rewrite without a starting file: identical
/// to that of the C version (captured from `opentyrian2000 --goto=destruct`).
#[test]
fn default_config_file_matches_c() {
    let expected = include_bytes!("c_default_destruct.cfg");
    let (cfg, out, warnings) = crate::destruct_config::load(b"");
    assert!(warnings.is_empty());
    assert_eq!(String::from_utf8_lossy(&out), String::from_utf8_lossy(expected));
    assert_eq!(cfg.max_shots, 40);
    assert_eq!(cfg.ai, [true, false]);
    assert_eq!(cfg.keys, crate::destruct_config::DEFAULT_KEYS);
    // Read back, the file no longer changes.
    let (_, again, _) = crate::destruct_config::load(&out);
    assert_eq!(again, out);
}

/// Values read, clamped, keys remapped and Custom squad.
#[test]
fn config_values_are_read_and_bounded() {
    let text = b"section 'destruct'\n\titem 'left ai' 'no'\n\titem 'max shots' '0'\n\titem 'min walls' '30'\n\titem 'max walls' '5'\n\titem 'antialias craters' 'yes'\n\
section 'destruct keyboard' 'left'\n\titem 'fire' 'F'\n\tlist 'change' 'Nope'\n\
section 'destruct custom'\n\titem 'enable' 'yes'\n\titem 'left num units' '3'\n\tlist 'left unit' 'heli'\n\tlist 'left unit' 'laser'\n";
    let (cfg, out, warnings) = crate::destruct_config::load(text);
    assert_eq!(cfg.ai, [false, false]);
    assert_eq!(cfg.max_shots, 40, "0 out of range: default");
    assert_eq!((cfg.min_walls, cfg.max_walls), (20, 20), "min > max: 20/20");
    assert!(cfg.alwaysalias);
    assert_eq!(warnings.len(), 2);
    assert_eq!(cfg.keys[0][MOVE_FIRE], [9, 0, 0, 0], "F only");
    assert_eq!(cfg.keys[0][MOVE_CHANGE], [226, 0, 0, 0], "unknown name: default");
    assert!(cfg.allow_custom);
    assert_eq!(&cfg.custom_units[0][..4], &[3, UNIT_HELI as u8, UNIT_LASER as u8, UNIT_TANK as u8]);
    let out = String::from_utf8_lossy(&out);
    assert!(out.contains("\titem 'max shots' '0'\n"), "faulty value left in the file");
    assert!(out.contains("\titem 'fire' 'F'\n"));
    assert!(out.contains("\tlist 'change' 'Left Alt'\n"));
}
