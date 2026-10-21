//! Pseudo-random generators of the original game, reproduced bit for bit.
//!
//! - [`MersenneTwister`]: `src/mtrand.c` (MT19937), used by gameplay.
//! - [`Xoshiro256`]: `src/rng.c` (xoshiro256** seeded by SplitMix64), used
//!   by effects and Endless mode.

const N: usize = 624;
const M: usize = 397;

/// `mtrand.c`: MT19937, 32-bit output.
#[derive(Clone)]
pub struct MersenneTwister {
    x: [u32; N],
    p0: usize,
    p1: usize,
    pm: usize,
}

impl MersenneTwister {
    /// `mt_srand`.
    pub fn new(seed: u32) -> Self {
        let mut x = [0u32; N];
        x[0] = seed;
        for i in 1..N {
            x[i] = 1_812_433_253u32.wrapping_mul(x[i - 1] ^ (x[i - 1] >> 30)).wrapping_add(i as u32);
        }
        Self { x, p0: 0, p1: 1, pm: M }
    }

    /// `mt_rand`.
    pub fn next_u32(&mut self) -> u32 {
        let (p0, p1) = (self.x[self.p0], self.x[self.p1]);
        let mag = if p1 & 1 != 0 { 0x9908_b0df } else { 0 };
        let y = self.x[self.pm] ^ (((p0 & 0x8000_0000) | (p1 & 0x7fff_ffff)) >> 1) ^ mag;
        self.x[self.p0] = y;
        self.p0 = self.p1;
        self.p1 = (self.p1 + 1) % N;
        self.pm = (self.pm + 1) % N;
        let mut y = y;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^= y >> 18;
        y
    }

    /// Hash identical to `mtrand.c:mt_state_hash` (golden master).
    pub fn state_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &v in &self.x {
            h = (h ^ v as u64).wrapping_mul(0x0100_0000_01b3);
        }
        (h ^ self.p0 as u64).wrapping_mul(0x0100_0000_01b3)
    }
}

/// `rng.c`: xoshiro256**.
#[derive(Clone)]
pub struct Xoshiro256 {
    s: [u64; 4],
}

fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

impl Xoshiro256 {
    /// `rng_seed` (non-zero seed: the C code replaces 0 with the current time).
    pub fn new(seed: u64) -> Self {
        let mut sm = seed;
        Self { s: [splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm), splitmix64(&mut sm)] }
    }

    fn next_u64(&mut self) -> u64 {
        let s = &mut self.s;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// `rng_next`.
    pub fn next_u32(&mut self) -> u32 {
        (self.next_u64() >> 32) as u32
    }

    /// `rng.c:rng_range`: `min + rng_next() % (max - min + 1)`, with no draw
    /// when `min >= max`.
    pub fn range(&mut self, min: i32, max: i32) -> i32 {
        if min >= max {
            return min;
        }
        let span = (max - min) as u32 + 1;
        min + (self.next_u32() % span) as i32
    }

    /// `rng.c:rng_next_float`: top 24 bits, as an integer
    /// (real value = result / 2^24). A single xoshiro draw is consumed.
    pub fn next_float_bits(&mut self) -> u32 {
        (self.next_u64() >> 40) as u32
    }

    /// Hash identical to `rng.c:rng_state_hash` (golden master).
    pub fn state_hash(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for &v in &self.s {
            h = (h ^ v).wrapping_mul(0x0100_0000_01b3);
        }
        h
    }

    /// Raw internal state (snapshots).
    pub fn state(&self) -> [u64; 4] {
        self.s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seed 1: hashes taken from rewrite/golden/*.gm at tick 0
    /// (columns h_mt and h_rng, before any consumption).
    #[test]
    fn seeded_state_matches_c_golden_master() {
        assert_eq!(MersenneTwister::new(1).state_hash(), 0x61be_fcea_d2f4_5e71);
        assert_eq!(Xoshiro256::new(1).state_hash(), 0xda45_a0c9_0d73_c762);
    }

    /// Public reference value of MT19937 (seed 5489 → 3499211612).
    #[test]
    fn mt19937_reference_value() {
        assert_eq!(MersenneTwister::new(5489).next_u32(), 3_499_211_612);
    }
}
