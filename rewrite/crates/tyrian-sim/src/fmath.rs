//! Complete, bit-for-bit reproduction of the few C floating-point computations that
//! influence game state (ADR 0003: no floating point in the simulation).
//!
//! The C code computes in single-precision IEEE-754 `float` (round to nearest,
//! ties to even): each operation here is therefore an exact rounding
//! of a rational to a 24-bit mantissa. The values handled stay
//! modest (|x| < 2^20), subnormals never appear.

/// Non-negative single-precision float, represented exactly by
/// `mant * 2^exp` with `mant < 2^24`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct F32 {
    pub neg: bool,
    pub mant: u64,
    pub exp: i32,
}

fn bits(v: u128) -> i32 {
    128 - v.leading_zeros() as i32
}

/// Rounds the rational `num / den` (den > 0) to the nearest `float`.
pub fn round_rational(num: i128, den: i128) -> F32 {
    assert!(den > 0);
    let neg = num < 0;
    let n = num.unsigned_abs();
    let d = den as u128;
    if n == 0 {
        return F32 { neg: false, mant: 0, exp: 0 };
    }
    // Choice of the exponent e such that 2^23 <= n / (d * 2^e) < 2^24.
    let mut e = bits(n) - bits(d) - 24;
    loop {
        let (nn, dd) = scale(n, d, e);
        let q = nn / dd;
        if q < (1 << 23) {
            e -= 1;
        } else if q >= (1 << 24) {
            e += 1;
        } else {
            break;
        }
    }
    let (nn, dd) = scale(n, d, e);
    let mut q = nn / dd;
    let r = nn % dd;
    // Round to nearest, ties to even.
    let twice = 2 * r;
    if twice > dd || (twice == dd && q & 1 == 1) {
        q += 1;
    }
    if q == 1 << 24 {
        q = 1 << 23;
        e += 1;
    }
    F32 { neg, mant: q as u64, exp: e }
}

fn scale(n: u128, d: u128, e: i32) -> (u128, u128) {
    if e >= 0 {
        (n, d << e)
    } else {
        (n << (-e), d)
    }
}

#[allow(clippy::should_implement_trait)] // C operations named like the C ones, explicit rounding
impl F32 {
    /// Exact value as a rational (numerator, denominator).
    fn as_rational(self) -> (i128, i128) {
        let m = self.mant as i128 * if self.neg { -1 } else { 1 };
        if self.exp >= 0 {
            (m << self.exp, 1)
        } else {
            (m, 1i128 << (-self.exp))
        }
    }

    /// Float product `self * k` (integer k converted exactly to float).
    pub fn mul_int(self, k: i64) -> F32 {
        let (n, d) = self.as_rational();
        round_rational(n * k as i128, d)
    }

    /// Float product `self * other` (a single rounding, like the C).
    pub fn mul(self, other: F32) -> F32 {
        let (n1, d1) = self.as_rational();
        let (n2, d2) = other.as_rational();
        round_rational(n1 * n2, d1 * d2)
    }

    /// `roundf`: round to integer, half away from zero.
    pub fn roundf(self) -> i64 {
        let (n, d) = self.as_rational();
        let a = n.unsigned_abs();
        let d = d as u128;
        let mut q = a / d;
        if 2 * (a % d) >= d {
            q += 1;
        }
        let q = q as i64;
        if n < 0 {
            -q
        } else {
            q
        }
    }

    /// `floorf`.
    pub fn floorf(self) -> i64 {
        let (n, d) = self.as_rational();
        n.div_euclid(d) as i64
    }
}

/// `roundf((float)a / b * c)` with integer a, b, c (aimed enemy shots:
/// `enemy_render.c:JE_drawEnemy`).
pub fn round_div_mul(a: i64, b: i64, c: i64) -> i64 {
    let q = if b < 0 { round_rational(-(a as i128), -(b as i128)) } else { round_rational(a as i128, b as i128) };
    q.mul_int(c).roundf()
}

/// `floorf((float)a / b * c - 1)` (parallax scrolling,
/// `mainint.c:JE_mainGamePlayerFunctions`). The subtraction of 1 is exact
/// for the values reached (|x| >= 1, 24-bit mantissa), hence
/// `floor(x) - 1`.
pub fn floor_div_mul_minus1(a: i64, b: i64, c: i64) -> i64 {
    round_rational(a as i128, b as i128).mul_int(c).floorf() - 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_hardware_float_on_samples() {
        // Comparison with the hardware f32 (reserved for tests).
        for a in -300i64..300 {
            for b in [1i64, 3, 7, 13, 50, 99, 224, 255] {
                for c in [1i64, 2, 3, 5, 7, 9, 72] {
                    let hw = ((a as f32) / (b as f32) * (c as f32)).round() as i64;
                    assert_eq!(round_div_mul(a, b, c), hw, "{a}/{b}*{c}");
                    let hwf = ((a as f32) / (b as f32) * (c as f32) - 1.0).floor() as i64;
                    assert_eq!(floor_div_mul_minus1(a, b, c), hwf, "floor {a}/{b}*{c}");
                }
            }
        }
    }
}
