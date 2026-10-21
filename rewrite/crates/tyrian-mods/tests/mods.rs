//! Integration tests of the mod system (blocks 8 and 16).
//!
//! - the five shipped packs (`rewrite/mods`) load together without error and
//!   without a full table: 14 ships with their five poses, 18 weapons;
//!   the examples retired to `rewrite/mods/_retired` (converted from the C
//!   version) still resolve; a C-version mod is rejected;
//! - the retired example mod `rewrite/mods/_retired/example_behaviors` creates a
//!   new weapon and a new enemy that appear, move according to their behavior and
//!   fire in a real game, deterministically;
//! - the original game is unchanged when no mod is active;
//! - untrusted files (booby-trapped JSON, truncated PNG, out-of-bounds values)
//!   produce diagnostics, never a panic.

use std::path::{Path, PathBuf};
use tyrian_mods::{episode_items, json, ModSet, Severity};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};
use tyrian_sim::{LevelSetup, Loadout, PlayerInput, Sim};

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

fn data() -> PathBuf {
    repo().join("data")
}

fn example_mods() -> PathBuf {
    repo().join("rewrite/mods")
}

/// Mods removed from the release, kept as reference examples (never loaded by
/// the game, never packaged): alien_ships, alien_pack, example_nova_weapons,
/// example_behaviors.
fn retired_mods() -> PathBuf {
    example_mods().join("_retired")
}

/// The packs shipped with the game.
const SHIPPED: [&str; 5] =
    ["alien_ship_pack", "alien_weapon_pack", "bio_ships_pack", "bio_weapons_pack", "elements_weapon_pack"];

fn have_data() -> bool {
    data().join("tyrian1.lvl").is_file()
}

const BASE_LOADOUT: Loadout = Loadout {
    ship: ShipId(1),
    front_weapon: WeaponId(1),
    front_power: 1,
    rear_weapon: WeaponId(0),
    rear_power: 1,
    shield: ShieldId(4),
    generator: GeneratorId(2),
    left_sidekick: SidekickId(0),
    right_sidekick: SidekickId(0),
    special: SpecialId(0),
    cash: 0,
};

fn setup_with(set: &ModSet, episode: u8, num: usize) -> (LevelSetup, tyrian_mods::ResolvedMods) {
    let (lvl, items) = episode_items(&data(), episode).unwrap();
    let res = set.resolve(episode, &items);
    let mut loadout = BASE_LOADOUT;
    res.starter.apply(&mut loadout);
    let level = lvl.level(num).unwrap().clone();
    let mut setup = LevelSetup {
        episode,
        section: 0,
        difficulty: 2,
        level,
        items,
        loadout,
        seed: 7,
        assist: true,
        initial_difficulty: None,
        second_player_loadout: None,
        carry_over: None,
        modes: Default::default(),
        endless_armor_mult: None,
        patch: Default::default(),
        processor_type: 4,
    };
    res.apply_to(&mut setup, num as u16);
    (setup, res)
}

fn inputs(t: u64) -> [PlayerInput; 2] {
    [
        PlayerInput(
            PlayerInput::FIRE | if (t / 60).is_multiple_of(2) { PlayerInput::LEFT } else { PlayerInput::RIGHT },
        ),
        PlayerInput(0),
    ]
}

#[test]
fn shipped_mods_load_without_errors() {
    if !have_data() {
        return;
    }
    let mut set = ModSet::load_from(&data(), &example_mods());
    let mut names: Vec<String> = set.folders.iter().map(|f| f.folder.clone()).collect();
    names.sort();
    assert_eq!(names, SHIPPED, "rewrite/mods must contain exactly the 5 shipped packs");
    for n in &names {
        set.set_enabled(n, true);
    }
    for ep in 1..=5u8 {
        let (_, items) = episode_items(&data(), ep).unwrap();
        let res = set.resolve(ep, &items);
        let errors: Vec<_> = res.diagnostics.items.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "episode {ep}: {errors:?}");
        // All together, nothing is dropped: no full table, no missing sprite.
        assert!(res.diagnostics.items.is_empty(), "episode {ep}: {:?}", res.diagnostics.items);
        let r = |f: &str| res.reports.iter().find(|r| r.folder == f).unwrap().clone();
        assert_eq!(r("alien_ship_pack").ships, 8);
        assert_eq!(r("bio_ships_pack").ships, 6);
        assert_eq!(r("alien_weapon_pack").weapons, 6);
        assert_eq!(r("bio_weapons_pack").weapons, 6);
        assert_eq!(r("elements_weapon_pack").weapons, 6);
        // 14 of the 14 ship slots, 18 of the 20 weapon slots.
        let t = res.tables.as_ref().unwrap();
        assert!((19..=32).all(|i| !t.ships[i].name.text().is_empty()), "ship slots 19..32");
        assert_eq!((61..=80).filter(|&i| !t.ports[i].name.text().is_empty()).count(), 18);
        // Every pack item is offered in the shops (always_available).
        assert_eq!(res.shop["ships"].len(), 14);
        assert_eq!(res.shop["weapons_front"].len(), 9);
        assert_eq!(res.shop["weapons_rear"].len(), 9);
    }
}

#[test]
fn retired_example_mods_still_resolve() {
    if !have_data() {
        return;
    }
    // Reference examples kept in _retired: cloning by "base", tint, starter by name.
    let mut set = ModSet::load_from(&data(), &retired_mods());
    for n in ["alien_pack", "example_nova_weapons"] {
        assert!(set.set_enabled(n, true), "{n}");
    }
    for ep in 1..=5u8 {
        let (_, items) = episode_items(&data(), ep).unwrap();
        let res = set.resolve(ep, &items);
        let errors: Vec<_> = res.diagnostics.items.iter().filter(|d| d.severity == Severity::Error).collect();
        assert!(errors.is_empty(), "episode {ep}: {errors:?}");
        let r = |f: &str| res.reports.iter().find(|r| r.folder == f).unwrap().clone();
        assert_eq!(r("alien_pack").weapons, 4);
        assert_eq!(r("alien_pack").ships, 1);
        assert_eq!(r("example_nova_weapons").weapons, 3);
        // Weapons cloned by "base" keep their original projectiles.
        let t = res.tables.as_ref().unwrap();
        let vulcan = (1..=80).find(|&i| t.ports[i].name.text() == "Alien Vulcan Cannon").expect("cloned weapon");
        assert!(t.ports[vulcan].op[0][0] != 0);
        // Start: ship and weapon of the alien_pack mod (starter by name).
        assert!(res.starter.ship >= 19 && res.starter.front_weapon >= 61, "{:?}", res.starter);
    }
}

/// ADR 0134: the 14 mod ship slots, each with its five poses as five PNGs (the
/// two shipped ship packs), all resolve. Each pose sits at shipgraphic + bank * 2,
/// where level.gd draws it, and no mod image lands on an original ship.
#[test]
fn fourteen_mod_ships_with_five_poses_resolve() {
    if !have_data() {
        return;
    }
    let mut set = ModSet::load_from(&data(), &example_mods());
    for n in ["alien_ship_pack", "bio_ships_pack"] {
        assert!(set.set_enabled(n, true), "{n}");
    }
    let (_, items) = episode_items(&data(), 1).unwrap();
    let original_graphics: Vec<i32> = (1..19).map(|i| items.ships[i].shipgraphic as i32).collect();
    let res = set.resolve(1, &items);
    assert!(res.diagnostics.items.is_empty(), "{:?}", res.diagnostics.items);
    let t = res.tables.as_ref().unwrap();
    let ships: Vec<_> = res.sprites.iter().filter(|s| s.table == tyrian_mods::SpriteTable::Ships).collect();
    let image = |i: i32| {
        ships.iter().find(|s| s.index == i).map(|s| s.path.file_stem().unwrap().to_string_lossy().into_owned())
    };
    let mut seen = std::collections::BTreeSet::new();
    for id in 19..=32 {
        let g = t.ships[id].shipgraphic as i32;
        let neutral = image(g).unwrap_or_else(|| panic!("ship {id}: no image at {g}"));
        for (bank, suffix) in [(-2, "_left_full"), (-1, "_left"), (0, ""), (1, "_right"), (2, "_right_full")] {
            let i = g + bank * 2;
            assert_eq!(image(i), Some(format!("{neutral}{suffix}")), "ship {id}, bank {bank}");
            assert!(seen.insert(i), "ship {id}: index {i} shared with another ship");
        }
    }
    assert_eq!(seen.len(), 14 * 5);
    for g in original_graphics.into_iter().filter(|&g| g > 0) {
        for i in g - 4..=g + 4 {
            assert!(image(i).is_none(), "mod image {:?} over an original ship image ({i})", image(i));
        }
    }
}

#[test]
fn retired_alien_ships_is_not_listed_by_the_loader() {
    if !have_data() {
        return;
    }
    // The archived folder has no mod.json at the rewrite/mods level: it is not loaded.
    let set = ModSet::load_from(&data(), &example_mods());
    for retired in ["_retired", "alien_ships", "alien_pack", "example_nova_weapons", "example_behaviors"] {
        assert!(!set.folders.iter().any(|f| f.folder == retired), "{retired} is listed");
    }
}

#[test]
fn too_many_ship_mods_overflow_ship_slots_with_a_clear_error() {
    if !have_data() {
        return;
    }
    // The retired mod (12 ships) loaded by hand with Alien Ship Pack (8) and
    // alien_pack (1): 21 ships for 14 slots, named error, no panic.
    let mut set = ModSet::load_dirs(&data(), &[example_mods(), retired_mods()]);
    for n in ["alien_pack", "alien_ships", "alien_ship_pack"] {
        assert!(set.set_enabled(n, true), "{n}");
    }
    let (_, items) = episode_items(&data(), 1).unwrap();
    let res = set.resolve(1, &items);
    let e: Vec<_> = res.diagnostics.items.iter().filter(|d| d.severity == Severity::Error).collect();
    assert!(e.iter().any(|d| d.message.contains("no free slot left")), "{e:?}");
}

#[test]
fn disabled_mods_change_nothing() {
    if !have_data() {
        return;
    }
    // The shipped mods are disabled: the game must be the original one.
    let set = ModSet::load_from(&data(), &example_mods());
    assert_eq!(set.enabled_count(), 0);
    let (setup, res) = setup_with(&set, 1, 9);
    assert!(setup.patch.is_empty());
    assert!(res.levels.is_empty());
    let (lvl, items) = episode_items(&data(), 1).unwrap();
    let plain = LevelSetup {
        episode: 1,
        section: 0,
        difficulty: 2,
        level: lvl.level(9).unwrap().clone(),
        items,
        loadout: BASE_LOADOUT,
        seed: 7,
        assist: true,
        initial_difficulty: None,
        second_player_loadout: None,
        carry_over: None,
        modes: Default::default(),
        endless_armor_mult: None,
        patch: Default::default(),
        processor_type: 4,
    };
    let mut a = Sim::new(setup);
    let mut b = Sim::new(plain);
    for t in 0..800 {
        a.step(inputs(t));
        b.step(inputs(t));
    }
    assert_eq!(a.state_hash(), b.state_hash());
}

/// Plays level 1 of episode 1 with the retired example mod; returns
/// (drone spawns, drone shots, lateral movements observed, fingerprint).
fn play_example(ticks: u64) -> (u32, u32, u32, u64, tyrian_mods::ResolvedMods) {
    let mut set = ModSet::load_from(&data(), &retired_mods());
    // Disabled in its mod.json, like the C-version mods.
    assert!(set.set_enabled("example_behaviors", true));
    let (setup, res) = setup_with(&set, 1, 9);
    let drone = res.new_enemies["zigzag_drone"];
    let drone_weapon = res.enemy_weapons["drone_spread"];
    let sprite = res.tables.as_ref().unwrap().weapon(drone_weapon as usize).sg[0];
    let mut sim = Sim::new(setup);
    let mut spawns = 0;
    let mut shots = 0;
    let mut turns = 0;
    let mut prev_alive = [false; 100];
    let mut prev_exc = [0i8; 100];
    let mut prev_shot_used = vec![false; sim.world().enemy_shots.len()];
    for t in 0..ticks {
        sim.step(inputs(t));
        let w = sim.world();
        for i in 0..100 {
            let alive = w.enemy_avail[i] != 1 && w.enemy[i].enemytype == drone;
            if alive && !prev_alive[i] {
                spawns += 1;
            }
            if alive && prev_alive[i] && w.enemy[i].exc.signum() != prev_exc[i].signum() && prev_exc[i] != 0 {
                turns += 1;
            }
            prev_alive[i] = alive;
            prev_exc[i] = w.enemy[i].exc;
        }
        for (b, s) in w.enemy_shots.iter().enumerate() {
            let used = !w.enemy_shot_avail[b];
            if used && !prev_shot_used[b] && s.sgr == sprite {
                shots += 1;
            }
            prev_shot_used[b] = used;
        }
    }
    (spawns, shots, turns, sim.state_hash(), res)
}

#[test]
fn example_mod_new_weapon_and_enemy_run() {
    if !have_data() {
        return;
    }
    let (spawns, shots, turns, hash, res) = play_example(1200);
    println!("drone: {spawns} spawns, {shots} shots, {turns} U-turns, hash {hash:016x}");
    assert!(res.diagnostics.items.iter().all(|d| d.severity != Severity::Error), "{:?}", res.diagnostics.items);
    // New player weapon, equipped at the start.
    let t = res.tables.as_ref().unwrap();
    let helix = res.starter.front_weapon as usize;
    assert_eq!(t.ports[helix].name.text(), "Helix Driver");
    let w1 = t.weapon(t.ports[helix].op[0][0] as usize);
    assert_eq!((w1.multi, w1.attack[0], w1.sx[0], w1.sx[1]), (2, 3, -2, 2));
    // New enemy: 5 events added, all spawned.
    assert_eq!(spawns, 5, "drone spawns");
    assert!(turns >= 3, "the zigzag must reverse the horizontal speed ({turns})");
    assert!(shots >= 3, "the drone turret must fire ({shots})");
    // Determinism: two identical games give the same fingerprint.
    let (_, _, _, hash2, _) = play_example(1200);
    assert_eq!(hash, hash2);
}

#[test]
fn hostile_inputs_never_panic() {
    // Booby-trapped JSON: depth, size, numbers, strings.
    let deep = "[".repeat(10_000);
    assert!(json::parse(&deep).is_err());
    assert!(json::parse("{\"a\": 1e999999}").is_ok() || json::parse("{\"a\": 1e999999}").is_err());
    assert!(json::parse("{\"a\": \"\\ud800\"}").is_ok() || json::parse("{\"a\": \"\\ud800\"}").is_err());
    assert!(json::parse("{\"a\": 1,, }").is_err());
    assert!(json::parse("/* not closed").is_err());
    let huge = format!("\"{}\"", "x".repeat(json::MAX_STRING_BYTES + 1));
    assert!(json::parse(&huge).is_err());
    // Comments and trailing commas (dialect of the C mods).
    let v = json::parse("// c\n{ \"a\": [1, 2,], /* b */ \"b\": true, }").unwrap();
    assert_eq!(v.get("a").unwrap().as_array().unwrap().len(), 2);

    // Hostile mods folder written to disk.
    let dir = std::env::temp_dir().join(format!("tyrian-mods-hostile-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let w = |rel: &str, content: &[u8]| {
        let p = dir.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, content).unwrap();
    };
    w("a_bad_json/mod.json", b"{ \"name\": ");
    w("b_bad_png/mod.json", b"{\"format_version\":1,\"name\":\"b\",\"enabled\":true}");
    w("b_bad_png/sprites/shots/x.png", b"\x89PNG\r\n\x1a\nshort");
    w("b_bad_png/sprites/ships/huge.png", &{
        let mut v = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        v.extend_from_slice(&100_000u32.to_be_bytes());
        v.extend_from_slice(&100_000u32.to_be_bytes());
        v.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
        v
    });
    w(
        "c_out_of_range/mod.json",
        br#"{"format_version": 1, "name": "c", "enabled": true,
            "weapons": [{"name": "W", "front": [{"projectile_count": 99999, "max_on_screen": 300,
                "projectiles": [{"damage": -5, "speed_y": 1000, "sprite": "nope"}]}]}],
            "shields": [{"name": "S", "shield_power": 9999, "max_power": -3}],
            "enemies": [{"original": "absent", "armor": 5}, {"original": -1}, {"original": 99999},
                        {"name": "e", "base": "absent"}, {"name": "f", "behavior": {"version": 9}},
                        {"name": "g", "behavior": {"path": [{"ticks": 0, "vx": 500}]}},
                        {"name": "h", "behavior": {"turrets": [{"weapon": "absent", "every": 5}]}}],
            "starter": {"ship": 9999, "front_weapon": "inconnue"}}"#,
    );
    w(
        "c_out_of_range/levels/ep1_09.json",
        br#"{"episode": 1, "lvl_file_num": 9, "events": [{"time": 70000, "spawn": "tech_corridor_32"}]}"#,
    );
    w("c_out_of_range/levels/ep9_99.json", br#"{"episode": 9, "lvl_file_num": 9, "events": []}"#);
    w("d_readme/mod.json", b"{\"format_version\":1,\"name\":\"d\"}");
    w("d_readme/README.md", &vec![b'r'; 1 << 20]);
    w("e_not_utf8/mod.json", b"{\"format_version\":1,\"name\": \"\xff\xfe\"}");

    if have_data() {
        let mut set = ModSet::load_from(&data(), &dir);
        let names: Vec<String> = set.folders.iter().map(|f| f.folder.clone()).collect();
        for n in &names {
            set.set_enabled(n, true);
        }
        let (_, items) = episode_items(&data(), 1).unwrap();
        let res = set.resolve(1, &items);
        let errors = res.diagnostics.count(Severity::Error);
        for d in &res.diagnostics.items {
            println!("{:?} {} : {}", d.severity, d.folder, d.message);
        }
        assert!(errors >= 8, "expected errors, got {errors}: {:#?}", res.diagnostics.items);
        // Values clamped as in the C version.
        let t = res.tables.as_ref().unwrap();
        let s = (12..=20).find(|&i| t.shields[i].name.text() == "S").unwrap();
        assert_eq!((t.shields[s].tpwr, t.shields[s].mpwr), (28, 0));
        let wport = (61..=80).find(|&i| t.ports[i].name.text() == "W").unwrap();
        assert!(t.weapon(t.ports[wport].op[0][0] as usize).max <= 8);
        // No half-valid behavior was attached.
        assert!(res.patch.behaviors.is_empty());
        // And a game still starts.
        let (setup, _) = setup_with(&set, 1, 9);
        let mut sim = Sim::new(setup);
        for t in 0..200 {
            sim.step(inputs(t));
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Writes a mod `name` (mod.json + files) into a temporary folder and
/// returns the error diagnostics of its resolution on episode 1.
fn errors_for(files: &[(&str, &str)]) -> Vec<String> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let dir = std::env::temp_dir().join(format!(
        "tyrian-mods-format-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let _ = std::fs::remove_dir_all(&dir);
    for (rel, content) in files {
        let p = dir.join("m").join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        if rel.ends_with(".png") {
            // Minimal valid PNG header (1×1): only the file name matters here.
            let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
            png.extend_from_slice(&1u32.to_be_bytes());
            png.extend_from_slice(&1u32.to_be_bytes());
            png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
            std::fs::write(p, png).unwrap();
        } else {
            std::fs::write(p, content).unwrap();
        }
    }
    let mut set = ModSet::load_from(&data(), &dir);
    set.set_enabled("m", true);
    let (_, items) = episode_items(&data(), 1).unwrap();
    let res = set.resolve(1, &items);
    let _ = std::fs::remove_dir_all(&dir);
    res.diagnostics.items.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect()
}

#[test]
fn c_format_mods_are_rejected() {
    if !have_data() {
        return;
    }
    // The mod.json of the three C-version mods, as they were: rejected, nothing is applied.
    let mut set = ModSet::load_from(&data(), &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/c-mods"));
    for n in ["alien_pack", "alien_ships", "example_nova_weapons"] {
        assert!(set.set_enabled(n, true), "{n}");
    }
    let (_, items) = episode_items(&data(), 1).unwrap();
    let res = set.resolve(1, &items);
    assert!(res.patch.is_empty() && res.sprites.is_empty());
    assert_eq!(res.diagnostics.count(Severity::Error), 3, "{:?}", res.diagnostics.items);
    assert!(res.reports.iter().all(|r| r.error.contains("format_version")));
    // Unknown version: rejected too.
    let e = errors_for(&[("mod.json", r#"{"format_version": 2, "name": "m"}"#)]);
    assert!(e.iter().any(|m| m.contains("unknown format_version 2")), "{e:?}");
}

#[test]
fn legacy_numbering_is_rejected() {
    if !have_data() {
        return;
    }
    let cases: [(&str, &[(&str, &str)]); 8] = [
        // projectile identifier imposed by the author
        (
            "_weapon_id",
            &[(
                "mod.json",
                r#"{"format_version":1,"name":"m","weapons":[{"name":"W","front":[{"_weapon_id":1900,"projectiles":[{"sprite":"orb_teal_1"}]}]}]}"#,
            )],
        ),
        // sprite by number
        (
            "by name",
            &[(
                "mod.json",
                r#"{"format_version":1,"name":"m","weapons":[{"name":"W","front":[{"projectiles":[{"sprite":12}]}]}]}"#,
            )],
        ),
        (
            "sprite_12",
            &[(
                "mod.json",
                r#"{"format_version":1,"name":"m","weapons":[{"name":"W","front":[{"projectiles":[{"sprite":"sprite_12"}]}]}]}"#,
            )],
        ),
        // PNG named by a number
        ("never a number", &[("mod.json", r#"{"format_version":1,"name":"m"}"#), ("sprites/shots/801.png", "")]),
        // starting equipment by number
        ("item name expected", &[("mod.json", r#"{"format_version":1,"name":"m","starter":{"ship":3}}"#)]),
        // enemy by "id" (old C format)
        ("'id'", &[("mod.json", r#"{"format_version":1,"name":"m","enemies":[{"id":5,"armor":3}]}"#)]),
        // old sections and keys of format 2
        ("'new_enemies'", &[("mod.json", r#"{"format_version":1,"name":"m","new_enemies":[]}"#)]),
        (
            "'format'",
            &[
                ("mod.json", r#"{"format_version":1,"name":"m"}"#),
                ("levels/ep1_09.json", r#"{"format":1,"episode":1,"lvl_file_num":9,"events":[]}"#),
            ],
        ),
    ];
    for (needle, files) in cases {
        let e = errors_for(files);
        assert!(e.iter().any(|m| m.contains(needle)), "expected '{needle}': {e:?}");
    }
}

#[test]
fn turret_weapon_of_mod_is_cited_by_name_only() {
    if !have_data() {
        return;
    }
    // The number of a weapon assigned to a mod (1800+) cannot be cited.
    let e = errors_for(&[(
        "mod.json",
        r#"{"format_version":1,"name":"m","enemies":[{"name":"d","behavior":{"turrets":[{"weapon":1800,"every":5}]}}]}"#,
    )]);
    assert!(e.iter().any(|m| m.contains("is referenced by name")), "{e:?}");
    // A new enemy name is accepted in elaunchtype; an unknown name is an error.
    let ok = errors_for(&[(
        "mod.json",
        r#"{"format_version":1,"name":"m","enemies":[{"name":"a","base":"tech_corridor_32"},{"name":"b","base":"tech_corridor_32","elaunchtype":"a"}]}"#,
    )]);
    assert!(ok.is_empty(), "{ok:?}");
    let ko = errors_for(&[(
        "mod.json",
        r#"{"format_version":1,"name":"m","enemies":[{"name":"b","base":"tech_corridor_32","eenemydie":"absent"}]}"#,
    )]);
    assert!(ko.iter().any(|m| m.contains("unknown enemy 'absent'")), "{ko:?}");
}

/// Point 13: no mandatory number any more — original content (enemies,
/// enemy weapons) and special damage are cited by name; a
/// number is rejected with a message that says what to write.
#[test]
fn original_content_and_damage_codes_are_cited_by_name() {
    if !have_data() {
        return;
    }
    let mods = |body: &str| format!(r#"{{"format_version":1,"name":"m",{body}}}"#);
    let ok = errors_for(&[(
        "mod.json",
        &mods(
            r#""enemies":[{"original":"tech_corridor_32","armor":9,"tur":["ring_green_leftdown_01",null,null],"eenemydie":"tech_corridor_01"}],
            "weapons":[{"name":"W","front":[{"projectiles":[{"damage":"freeze","sprite":"orb_teal_1"},{"damage":{"pierce":3},"sprite":"orb_teal_1"},{"damage":{"chain":"ring_green_leftdown_01"},"sprite":"orb_teal_1"}]}]}]"#,
        ),
    )]);
    assert!(ok.is_empty(), "{ok:?}");
    let refused: [(&str, String); 6] = [
        ("is referenced by name", mods(r#""enemies":[{"original":10,"armor":3}]"#)),
        ("is referenced by name", mods(r#""enemies":[{"name":"d","base":10}]"#)),
        ("is referenced by name", mods(r#""enemies":[{"original":"tech_corridor_32","tur":[59,0,0]}]"#)),
        ("is referenced by name", mods(r#""enemies":[{"name":"d","behavior":{"turrets":[{"weapon":59,"every":5}]}}]"#)),
        (
            "special effects are written by name",
            mods(r#""weapons":[{"name":"W","front":[{"projectiles":[{"damage":255,"sprite":"orb_teal_1"}]}]}]"#),
        ),
        ("unknown", mods(r#""enemies":[{"original":"no_such_enemy"}]"#)),
    ];
    for (needle, json) in refused {
        let e = errors_for(&[("mod.json", &json)]);
        assert!(e.iter().any(|m| m.contains(needle)), "expected '{needle}' for {json}: {e:?}");
    }
}

/// Point 12: levels by named events. Each original level (70)
/// exported in the named format then read back gives the same game (fingerprint at 3000
/// ticks); a type without a name or a number is a clear error.
#[test]
fn named_level_events_round_trip_every_original_level() {
    use tyrian_mods::levels::EnemyCited;
    use tyrian_mods::Diagnostics;
    if !have_data() {
        return;
    }
    let set = ModSet::load_dirs(&data(), &[]);
    let mut levels = 0;
    for ep in 1..=5u8 {
        let (lvl, items) = episode_items(&data(), ep).unwrap();
        for (i, l) in lvl.levels.iter().enumerate() {
            let num = i + 1;
            let list: Vec<_> = l.events.events.iter().map(|e| (*e, None::<EnemyCited>)).collect();
            let random: Vec<EnemyCited> = l.events.enemies.iter().map(|&r| EnemyCited::Slot(r)).collect();
            let text = set
                .export_level(&items, ep, num as u16, &list, &random)
                .unwrap_or_else(|e| panic!("ep{ep}_{num:02}: {e}"));
            assert!(!text.contains("eventtype") && !text.contains("eventdat"), "ep{ep}_{num:02}: numbers exported");
            let mut diag = Diagnostics::default();
            let back = set
                .read_level(&items, ep, &text, &mut diag)
                .unwrap_or_else(|| panic!("ep{ep}_{num:02}: {:?}", diag.items));
            assert_eq!(back.events.len(), l.events.events.len());
            assert_eq!(back.random_enemies.clone().unwrap_or_default(), l.events.enemies);
            let play = |events: Option<&tyrian_mods::levels::LevelOverride>| {
                let mut level = l.clone();
                if let Some(o) = events {
                    o.apply(&mut level.events);
                }
                let setup = LevelSetup {
                    episode: ep,
                    section: 0,
                    difficulty: 2,
                    level,
                    items: items.clone(),
                    loadout: BASE_LOADOUT,
                    seed: 7,
                    assist: true,
                    initial_difficulty: None,
                    second_player_loadout: None,
                    carry_over: None,
                    modes: Default::default(),
                    endless_armor_mult: None,
                    patch: Default::default(),
                    processor_type: 4,
                };
                let mut sim = Sim::new(setup);
                for t in 0..3000 {
                    sim.step(inputs(t));
                }
                sim.state_hash()
            };
            assert_eq!(play(None), play(Some(&back)), "ep{ep}_{num:02}: different game after round trip");
            levels += 1;
        }
    }
    assert_eq!(levels, 70);
    let level = |events: &str| {
        errors_for(&[
            ("mod.json", r#"{"format_version":1,"name":"m"}"#),
            ("levels/ep1_09.json", &format!(r#"{{"episode":1,"lvl_file_num":9,"add_events":[{events}]}}"#)),
        ])
    };
    for (needle, events) in [
        ("missing or unknown event type", r#"{"time": 5, "eventtype": 15, "eventdat": 10}"#),
        ("missing or unknown event type", r#"{"time": 5, "teleport": true}"#),
        ("an enemy is referenced by name", r#"{"time": 5, "spawn": 10}"#),
        ("unknown parameter 'eventdat2'", r#"{"time": 5, "spawn": "tech_corridor_32", "eventdat2": 130}"#),
        ("write \"random\"", r#"{"time": 5, "spawn": "tech_corridor_32", "x": -200}"#),
        ("'time' (number) required", r#"{"spawn": "tech_corridor_32"}"#),
    ] {
        let e = level(events);
        assert!(e.iter().any(|m| m.contains(needle)), "expected '{needle}' for {events}: {e:?}");
    }
    assert!(level(
        r#"{"time": 5, "spawn": "tech_corridor_32", "x": "random", "link": 3}, {"time": 9, "stars_off": true}"#
    )
    .is_empty());
}

/// Point 15: the `$schema` key (pointer to rewrite/mods/mod.schema.json
/// for text editors) is explicitly accepted; other unknown
/// keys remain errors.
#[test]
fn schema_key_is_accepted() {
    if !have_data() {
        return;
    }
    let ok = errors_for(&[("mod.json", r#"{"$schema": "../mod.schema.json", "format_version": 1, "name": "m"}"#)]);
    assert!(ok.is_empty(), "{ok:?}");
    let ko = errors_for(&[("mod.json", r#"{"$schemas": "x", "format_version": 1, "name": "m"}"#)]);
    assert!(ko.iter().any(|m| m.contains("unknown key '$schemas'")), "{ko:?}");
}

/// Point 19: an enemy PNG is named like the original enemy it
/// replaces (sprites/enemies/<name>.png); it targets the first image of that
/// enemy in its bank. Old <bank>/<image>.png convention rejected.
#[test]
fn enemy_sprites_are_named_after_the_enemy() {
    if !have_data() {
        return;
    }
    let resolve = |files: &[(&str, &str)]| {
        let dir = std::env::temp_dir().join(format!("tyrian-mods-enemy-png-{}-{}", std::process::id(), files.len()));
        let _ = std::fs::remove_dir_all(&dir);
        for (rel, content) in files {
            let p = dir.join("m").join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            if rel.ends_with(".png") {
                let mut png = vec![0x89, b'P', b'N', b'G', 13, 10, 26, 10, 0, 0, 0, 13, b'I', b'H', b'D', b'R'];
                png.extend_from_slice(&12u32.to_be_bytes());
                png.extend_from_slice(&14u32.to_be_bytes());
                png.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
                std::fs::write(p, png).unwrap();
            } else {
                std::fs::write(p, content).unwrap();
            }
        }
        let mut set = ModSet::load_from(&data(), &dir);
        set.set_enabled("m", true);
        let (_, items) = episode_items(&data(), 1).unwrap();
        let res = set.resolve(1, &items);
        let _ = std::fs::remove_dir_all(&dir);
        res
    };
    let mod_json = ("mod.json", r#"{"format_version":1,"name":"m"}"#);
    let res = resolve(&[mod_json, ("sprites/enemies/tech_corridor_32.png", "")]);
    let e = &ModSet::load_dirs(&data(), &[]).originals.enemies["tech_corridor_32"];
    let s: Vec<_> = res.sprites.iter().filter(|s| s.table == tyrian_mods::SpriteTable::Enemies).collect();
    assert_eq!(s.len(), 1, "{:?}", res.diagnostics.items);
    assert_eq!((s[0].bank, s[0].index), (e.bank as i32, e.record.egraphic[0] as i32));
    for (needle, rel) in [
        ("without a subfolder", "sprites/enemies/1/191.png"),
        ("unknown", "sprites/enemies/no_such_enemy.png"),
        ("unknown", "sprites/enemies/191.png"),
    ] {
        let res = resolve(&[mod_json, (rel, "")]);
        let errors: Vec<_> =
            res.diagnostics.items.iter().filter(|d| d.severity == Severity::Error).map(|d| d.message.clone()).collect();
        assert!(errors.iter().any(|m| m.contains(needle)), "expected '{needle}' for {rel}: {errors:?}");
    }
}

/// The published catalog (`rewrite/mods/original-names.json`) is the one
/// computed by the loader from the data: `tyrian-mods catalog` regenerates it.
#[test]
fn published_original_names_match_the_data() {
    if !have_data() {
        return;
    }
    let set = ModSet::load_from(&data(), &example_mods());
    let published = std::fs::read_to_string(example_mods().join("original-names.json")).unwrap();
    assert!(published == set.originals.to_json(), "rewrite/mods/original-names.json is out of date: rerun `cargo run -p tyrian-mods -- catalog --data ../data --sheets ../docs/original-enemies`");
}
