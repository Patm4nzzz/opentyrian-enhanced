//! Anomalies and mutators in Endless against the C golden master
//! (`endless.c:endless_anomaly_golden_dump`, `c-final:utils/golden/record_endless.sh`,
//! files `rewrite/golden/endless/anomalies-seed*.agm`): `anomaly_pick_three`
//! draws, xorshift state, deck multiplier bit for bit,
//! `mutators_score_multiplier` for the 256 masks and
//! `endless_apply_score_mult`.

use std::path::Path;
use tyrian_sim::anomalies::{final_score, mutators_score_multiplier, Deck, DECK_MAX};
use tyrian_sim::f32ops::F32;

#[test]
fn anomalies_match_c() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../golden/endless");
    let mut files = 0;
    for seed in [1u32, 7, 42, 1_234_567] {
        let path = dir.join(format!("anomalies-seed{seed}.agm"));
        let text = std::fs::read_to_string(&path).expect("golden master anomalies");
        files += 1;
        let mut deck = Deck::new(seed);
        let mut lines = 0;
        for line in text.lines() {
            let t: Vec<&str> = line.split_whitespace().collect();
            match t[0] {
                "anomalies" => assert_eq!(t[4], "11", "registry size"),
                "pick" => {
                    let wave: u16 = t[1].parse().unwrap();
                    let n: usize = t[2].parse().unwrap();
                    let picks = deck.pick_three(wave);
                    let mut got: Vec<i64> = picks.iter().map(|&p| p as i64).collect();
                    got.resize(3, -1);
                    let exp: Vec<i64> = t[3..6].iter().map(|v| v.parse().unwrap()).collect();
                    assert_eq!((picks.len(), got), (n, exp), "seed {seed} wave {wave}");
                    assert_eq!(format!("{:08x}", deck.rng_state), t[7], "seed {seed} wave {wave}: xorshift");
                    if n > 0 && deck.active.len() < DECK_MAX {
                        deck.add(picks[wave as usize % n]);
                    }
                    assert_eq!(deck.active.len().to_string(), t[9]);
                    assert_eq!(
                        format!("{:08x}", deck.score_multiplier().0),
                        t[11],
                        "seed {seed} wave {wave}: multiplier"
                    );
                }
                "mutators" => {
                    let mask = u8::from_str_radix(t[1], 16).unwrap();
                    assert_eq!(format!("{:08x}", mutators_score_multiplier(mask).0), t[2], "mutators {mask:02x}");
                }
                "final" => {
                    let base: u64 = t[1].parse().unwrap();
                    let m = F32(u32::from_str_radix(t[2], 16).unwrap());
                    assert_eq!(m.mul_u64_trunc_via_double(base).to_string(), t[3], "final {base} × {}", t[2]);
                }
                "end" => {}
                other => panic!("unknown line {other}"),
            }
            lines += 1;
        }
        assert_eq!(lines, 306, "seed {seed}: lines read");
    }
    assert_eq!(files, 4);
    // Final score = base × deck × mutators (endless.c:endless_record_score).
    let mut d = Deck::new(1);
    d.add(0); // glass_hull, ×3
    assert_eq!(final_score(1400, &d, 0x80 | 0x01), 1400 * 3 * 5 / 4);
}
