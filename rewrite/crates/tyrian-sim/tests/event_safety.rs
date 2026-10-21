//! Real data and initialization; only the event stream is replaced.
use std::path::PathBuf;
use tyrian_formats::lvl::Event;
use tyrian_sim::{setup::load_level, world::World, Loadout};
use tyrian_sim::{GeneratorId, ShieldId, ShipId, SidekickId, SpecialId, WeaponId};

fn event(time: u16, kind: u8, data: i16) -> Event {
    Event {
        eventtime: time,
        eventtype: kind,
        eventdat: data,
        eventdat2: 0,
        eventdat3: 0,
        eventdat4: 0,
        eventdat5: 0,
        eventdat6: 0,
    }
}

fn world(events: Vec<Event>) -> World {
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../data");
    let loadout = Loadout {
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
        cash: 10000,
    };
    let mut setup = load_level(&data, 1, 2, 9, 2, loadout, 42).expect("real level fixtures required");
    setup.level.events.events = events;
    World::start_level(&setup)
}

#[test]
fn same_tick_self_jump_stops_script() {
    let mut w = world(vec![event(0, 54, 0)]);
    w.loop_top();
    assert!(w.ending.really_end_level);
    assert!(w.script.event_loc > w.script.max_event);
}

#[test]
fn same_tick_two_event_cycle_stops_script() {
    let mut w = world(vec![event(0, 54, 1), event(1, 54, 0)]);
    w.loop_top();
    assert!(w.ending.really_end_level);
    assert!(w.script.event_loc > w.script.max_event);
}

#[test]
fn finite_same_tick_revisit_is_not_a_cycle() {
    let mut branch = event(0, 70, 10);
    branch.eventdat2 = 1;
    let mut spawn = event(10, 6, 1);
    spawn.eventdat4 = 1;
    let mut w = world(vec![branch, event(1, 0, 0), spawn, event(10, 54, 0)]);
    w.loop_top();
    assert!(!w.ending.really_end_level);
    assert_eq!(w.script.event_loc, 2);
    assert!(w.search_for(1).is_some());
}

#[test]
fn missing_high_jump_stops_script() {
    let mut w = world(vec![event(0, 0, 0)]);
    for target in 65501..=65534 {
        w.ending.really_end_level = false;
        w.event_jump(target);
        assert!(w.ending.really_end_level, "missing target {target}");
        assert!(w.script.event_loc > w.script.max_event);
    }
}

#[test]
fn missing_return_target_stops_script() {
    let mut w = world(vec![event(0, 0, 0)]);
    for target in 65501..=65535 {
        w.ending.really_end_level = false;
        w.script.return_loc = target;
        w.event_jump(65535);
        assert!(w.ending.really_end_level, "missing return target {target}");
        assert!(w.script.event_loc > w.script.max_event);
    }
}

#[test]
fn high_target_present_is_valid() {
    let mut w = world(vec![event(0, 0, 0), event(65534, 0, 0)]);
    w.event_jump(65534);
    assert!(!w.ending.really_end_level);
    assert_eq!(w.script.event_loc, 1); // event_system increments after the jump.
    assert_eq!(w.script.cur_loc, 65534);
    assert_eq!(w.script.return_loc, 1);
}

#[test]
fn maximum_straight_line_script_finishes_in_one_tick() {
    let count = tyrian_sim::world::EVENT_MAXIMUM - 1;
    let mut events = vec![event(0, 0, 0); count];
    events[count - 1] = event(0, 99, 1);
    let mut w = world(events);
    w.loop_top();
    assert!(!w.ending.really_end_level);
    assert_eq!(w.script.event_loc as usize, count + 1);
    assert!(w.script.random_explosions);
}

#[test]
fn missing_return_via_dispatch_stays_past_end() {
    let mut w = world(vec![event(0, 54, -1)]);
    w.script.return_loc = 65535;
    let mut rng = w.rng.clone();
    w.event_system();
    assert!(w.ending.really_end_level);
    assert!(w.script.event_loc > w.script.max_event);
    assert_eq!(w.rng.next_u32(), rng.next_u32());
}

#[test]
fn high_return_target_present_is_valid() {
    let mut w = world(vec![event(0, 0, 0), event(65535, 0, 0)]);
    let mut rng = w.rng.clone();
    w.script.return_loc = 65535;
    w.event_jump(65535);
    assert!(!w.ending.really_end_level);
    assert_eq!(w.script.cur_loc, 65535);
    assert_eq!(w.script.event_loc, 1);
    assert_eq!(w.rng.next_u32(), rng.next_u32());
}

#[test]
fn return_and_sentinel_keep_original_index_convention() {
    let mut w = world(vec![event(0, 0, 0), event(12, 0, 0)]);
    w.script.return_loc = 12;
    w.event_jump(65535);
    assert_eq!(w.script.cur_loc, 12);
    assert_eq!(w.script.return_loc, 12);
    assert_eq!(w.script.event_loc, 1);
    w.event_jump(65500);
    assert_eq!(w.script.event_loc, w.script.max_event);
    assert!(!w.ending.really_end_level);
}
