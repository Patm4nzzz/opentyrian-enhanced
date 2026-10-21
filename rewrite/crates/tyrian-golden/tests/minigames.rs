//! Campaign mini-games (`]e[` ENGAGE + `]g[` GALAGA): `** ALE **`
//! (episode 1, section 41) and `SQUADRON` (episode 4, section 45).
//!
//! The game mode is deduced from the episode script by
//! `tyrian_sim::campaign::mini_game_modes` — exactly what the Godot
//! campaign does — and must be the one the C code writes in the header of its
//! golden masters (`mode-galaga.gm`, `mode-squadron.gm`, recorded by
//! `c-final:utils/golden/record.sh`). Replayed with these inputs, the level must end
//! as in the C code: with the game over (neither has a level-end
//! event), which brings the campaign back to the `]b[` save.

use std::path::{Path, PathBuf};
use tyrian_formats::episode::EpisodeScript;
use tyrian_golden::GoldenMaster;
use tyrian_sim::campaign::{
    engage_loadout, mini_game_exit, mini_game_modes, run_script, MiniGameExit, ScriptContext, ScriptStop,
};
use tyrian_sim::{PlayerInput, Sim};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn check(golden: &str, episode: u8, section: u16, name: &str) {
    let data = root().join("data");
    let gm_path = root().join("rewrite/golden").join(golden);
    let Ok(script_bytes) = std::fs::read(data.join(format!("levels{episode}.dat"))) else {
        eprintln!("data missing: test skipped");
        return;
    };
    let gm = GoldenMaster::load(&gm_path).expect("golden master");
    let script = EpisodeScript::parse(&script_bytes).unwrap();
    let ctx = ScriptContext { initial_difficulty: 2, all_players_alive: true, ship: 1, ..Default::default() };
    let (stop, fx) = run_script(&script, episode, section, &ctx, [true; 5]);
    let ScriptStop::Level { directive, .. } = stop else { panic!("{golden}: {stop:?}") };
    assert_eq!(directive.level_name.trim(), name);
    let modes = mini_game_modes(&fx, 2).expect("]e[ expected");
    let h = &gm.header;
    assert_eq!((h.episode, h.section as u16, h.lvl_file), (episode, section, directive.lvl_file_num as usize));
    assert_eq!(Some(modes), h.modes, "{golden}: mode inferred from the script ≠ C mode");
    assert_eq!(Loadout11::from(engage_loadout()), Loadout11::from(h.loadout), "{golden}: ]e[ equipment");

    // Same construction as the Godot level scene in campaign.
    let mut setup = tyrian_sim::setup::load_level(
        &data,
        episode,
        section as u8,
        directive.lvl_file_num as usize,
        h.difficulty,
        engage_loadout(),
        h.seed,
    )
    .unwrap();
    setup.initial_difficulty = Some(2);
    setup.modes = modes;
    let mut sim = Sim::new(setup);
    for exp in &gm.ticks {
        if sim.state().level_ended {
            break;
        }
        sim.step([PlayerInput(exp.mask), PlayerInput(0)]);
        let got = sim.frame_record();
        assert_eq!(
            (got.px, got.py, got.cur_loc, got.h_rng),
            (exp.px, exp.py, exp.cur_loc, exp.h_rng),
            "{golden}: tick {}",
            exp.tick
        );
    }
    assert_eq!(gm.end_reason, "level", "{golden}: the C version ends with the game over");
    assert!(sim.state().level_ended, "{golden}: level end expected");
    let w = sim.world();
    assert!(w.all_players_dead(), "{golden}: the only way out is death");
    assert_eq!(mini_game_exit(w.all_players_dead(), false), MiniGameExit::ReturnToBackup);
}

/// Comparable equipment (the `Loadout` fields without cash, which the
/// "items" line of the golden master does not carry).
#[derive(Debug, PartialEq, Eq)]
struct Loadout11([u8; 10]);

impl From<tyrian_sim::Loadout> for Loadout11 {
    fn from(l: tyrian_sim::Loadout) -> Self {
        Loadout11([
            l.ship.0,
            l.front_weapon.0,
            l.front_power,
            l.rear_weapon.0,
            l.rear_power,
            l.shield.0,
            l.generator.0,
            l.left_sidekick.0,
            l.right_sidekick.0,
            l.special.0,
        ])
    }
}

#[test]
fn ale_from_campaign_script_matches_c() {
    check("mode-galaga.gm", 1, 41, "** ALE **");
}

#[test]
fn squadron_from_campaign_script_matches_c() {
    check("mode-squadron.gm", 4, 45, "SQUADRON");
}
