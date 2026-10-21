//! C `float` operations reproduced bit for bit without floating point
//! (ADR 0003), for the paths that handle an angle as a `float`:
//! the Dragonwing linked gun (`player_movement.c:JE_playerMovement`,
//! `linkGunDirec`) and the orientation of its shots
//! (`shots.c:player_shot_set_direction`).
//!
//! A `float` is carried by its 32 IEEE-754 bits ([`F32`]). All
//! `float` and `double` values are dyadic rationals `m × 2^e`:
//! additions, comparisons and conversions are therefore exact, then rounded
//! to nearest (ties to even) as the hardware does.
//!
//! `sinf`/`cosf`: the reference is glibc 2.39 of the machine on which the
//! golden masters were recorded (Ubuntu 24.04, x86-64 with FMA),
//! which selects `__sinf_fma`/`__cosf_fma`: algorithm
//! `sysdeps/ieee754/flt-32/s_sinf.c` (`reduce_fast` reduction, `sinf_poly`
//! polynomial) compiled with `-mfma`, where GCC fuses each `a + b * c` into
//! one FMA. This is not a correctly rounded result (max error 0.56 ulp): the
//! algorithm itself is therefore replayed, double operation by double operation, in
//! exact arithmetic followed by IEEE rounding (ADR 0050). The test
//! `sincos_match_hardware` checks it against the libm of the machine.

/// Single-precision IEEE-754 `float`, by its bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct F32(pub u32);

/// Dyadic rational `m × 2^e` (exact value of a float or a double).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Dy {
    m: i128,
    e: i32,
}

fn bit_len(v: u128) -> u32 {
    128 - v.leading_zeros()
}

/// `M_PI` (double): 0x1921FB54442D18 · 2^-51.
const M_PI: Dy = Dy { m: 0x1921FB54442D18, e: -51 };
/// `M_PI_2` (double).
const M_PI_2: Dy = Dy { m: 0x1921FB54442D18, e: -52 };

impl Dy {
    fn zero() -> Dy {
        Dy { m: 0, e: 0 }
    }

    fn neg(self) -> Dy {
        Dy { m: -self.m, e: self.e }
    }

    /// Exact sum, or rounded with a sticky bit when the exponents are
    /// too far apart for the i128: the low term is then shifted right
    /// keeping a sticky bit, 126 bits below the top of the high term —
    /// well beyond the 53 useful bits, so the rounding that follows is unchanged.
    /// The mantissas handled are at most 110 bits.
    fn add(self, o: Dy) -> Dy {
        if self.m == 0 {
            return o;
        }
        if o.m == 0 {
            return self;
        }
        let (hi, lo) = if self.e >= o.e { (self, o) } else { (o, self) };
        let room = 126 - bit_len(hi.m.unsigned_abs()) as i32;
        let e = lo.e.max(hi.e - room);
        let lo_m = if e > lo.e {
            let s = (e - lo.e) as u32;
            let a = lo.m.unsigned_abs();
            let (q, lost) = if s >= 128 { (0, a != 0) } else { (a >> s, a & ((1u128 << s) - 1) != 0) };
            let q = (q | lost as u128) as i128;
            if lo.m < 0 {
                -q
            } else {
                q
            }
        } else {
            lo.m
        };
        Dy { m: (hi.m << (hi.e - e)) + lo_m, e }
    }

    /// Exact product (mantissas ≤ 63 bits).
    fn mul(self, o: Dy) -> Dy {
        Dy { m: self.m * o.m, e: self.e + o.e }
    }

    /// `a * b` in double.
    fn dmul(self, o: Dy) -> Dy {
        self.mul(o).to_f64()
    }

    /// `fma(a, b, c)`: `c + a * b` rounded only once, in double.
    fn dfma(a: Dy, b: Dy, c: Dy) -> Dy {
        c.add(a.mul(b)).to_f64()
    }

    fn sub(self, o: Dy) -> Dy {
        self.add(o.neg())
    }

    fn cmp(self, o: Dy) -> std::cmp::Ordering {
        let d = self.sub(o);
        d.m.cmp(&0)
    }

    /// Round to nearest (ties to even) to `p` mantissa bits.
    fn round(self, p: u32) -> Dy {
        let a = self.m.unsigned_abs();
        let n = bit_len(a);
        if n <= p {
            return self;
        }
        let shift = n - p;
        let mut q = a >> shift;
        let r = a & ((1u128 << shift) - 1);
        let half = 1u128 << (shift - 1);
        let mut shift = shift;
        if r > half || (r == half && q & 1 == 1) {
            q += 1;
            if q == 1u128 << p {
                q >>= 1;
                shift += 1;
            }
        }
        let q = q as i128;
        Dy { m: if self.m < 0 { -q } else { q }, e: self.e + shift as i32 }
    }

    fn to_f32(self) -> F32 {
        let r = self.round(24);
        if r.m == 0 {
            return F32(0);
        }
        let sign = if r.m < 0 { 1u32 << 31 } else { 0 };
        let mut q = r.m.unsigned_abs();
        let mut e = r.e;
        while q < (1 << 23) {
            q <<= 1;
            e -= 1;
        }
        let field = e + 23 + 127;
        assert!((1..=254).contains(&field), "float outside the normal range");
        F32(sign | (field as u32) << 23 | (q as u32 & 0x7F_FFFF))
    }

    fn to_f64(self) -> Dy {
        self.round(53)
    }
}

#[allow(clippy::should_implement_trait)] // C operations named like the C ones, explicit rounding
impl F32 {
    pub const ZERO: F32 = F32(0);

    fn dy(self) -> Dy {
        let sign = self.0 >> 31;
        let field = ((self.0 >> 23) & 0xFF) as i32;
        let frac = (self.0 & 0x7F_FFFF) as i128;
        let (m, e) = if field == 0 { (frac, -149) } else { (frac | 1 << 23, field - 127 - 23) };
        Dy { m: if sign == 1 { -m } else { m }, e }
    }

    /// Conversion of an integer to float (exact below 2^24).
    pub fn from_int(v: i64) -> F32 {
        Dy { m: v as i128, e: 0 }.to_f32()
    }

    /// `float f = M_PI;` (double rounded to float).
    pub fn m_pi() -> F32 {
        M_PI.to_f32()
    }

    /// `a + b` in float.
    pub fn add(self, o: F32) -> F32 {
        self.dy().add(o.dy()).to_f32()
    }

    /// `a - b` in float.
    pub fn sub(self, o: F32) -> F32 {
        self.dy().sub(o.dy()).to_f32()
    }

    /// `a * b` in float (exact product, a single rounding).
    pub fn mul(self, o: F32) -> F32 {
        self.dy().mul(o.dy()).to_f32()
    }

    /// `(unsigned long)((double)base * (double)self)`, clamped to `u64::MAX`
    /// (`endless_math.c:endless_apply_score_mult`).
    pub fn mul_u64_trunc_via_double(self, base: u64) -> u64 {
        let p = Dy { m: base as i128, e: 0 }.round(53).mul(self.dy()).to_f64();
        if p.m <= 0 {
            return 0;
        }
        let a = p.m as u128;
        let v = if p.e >= 0 {
            if p.e >= 64 {
                u128::MAX
            } else {
                a << p.e
            }
        } else if -p.e >= 128 {
            0
        } else {
            a >> (-p.e)
        };
        v.min(u64::MAX as u128) as u64
    }

    /// `fabsf(a)`.
    pub fn abs(self) -> F32 {
        F32(self.0 & 0x7FFF_FFFF)
    }

    /// `a < b`.
    pub fn lt(self, o: F32) -> bool {
        self.dy().cmp(o.dy()).is_lt()
    }

    /// Comparison `a < b` where `b` is a `float` constant given by its bits.
    pub fn lt_bits(self, b: u32) -> bool {
        self.lt(F32(b))
    }

    /// Comparison `a > b` where `b` is a `float` constant given by its bits.
    pub fn gt_bits(self, b: u32) -> bool {
        F32(b).lt(self)
    }

    /// `roundf(self * k)` with integer k (product rounded to float).
    pub fn mul_int_roundf(self, k: i64) -> i64 {
        let d = self.dy();
        let p = Dy { m: d.m * k as i128, e: d.e }.to_f32().dy();
        roundf_dy(p)
    }

    /// `sinf` (glibc 2.39, FMA variant).
    pub fn sin(self) -> F32 {
        sincos(self, false)
    }

    /// `cosf` (glibc 2.39, FMA variant).
    pub fn cos(self) -> F32 {
        sincos(self, true)
    }
}

/// `roundf`: half away from zero.
fn roundf_dy(d: Dy) -> i64 {
    if d.e >= 0 {
        return (d.m << d.e) as i64;
    }
    let s = (-d.e) as u32;
    if s >= 120 {
        return 0;
    }
    let a = d.m.unsigned_abs();
    let mut q = a >> s;
    if (a >> (s - 1)) & 1 == 1 {
        q += 1;
    }
    let q = q as i64;
    if d.m < 0 {
        -q
    } else {
        q
    }
}

// --- Mixed float/double operations of the C code ---------------------------

/// `f += 0.2f` / `f -= 0.2f`: 0.2f = 0x3E4CCCCD.
pub const F_0_2: F32 = F32(0x3E4C_CCCD);
/// `0.3f`.
pub const F_0_3: u32 = 0x3E99_999A;
/// `-3.24f`.
pub const F_MINUS_3_24: u32 = 0xC04F_5C29;

/// `(float)(x OP double)`: computed in double (53-bit rounding) then
/// converted to float, like `linkGunDirec -= (2 * M_PI)`.
fn via_double(x: Dy) -> F32 {
    x.to_f64().to_f32()
}

/// `double` constants used by `player_movement.c` and `shots.c`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoubleConst {
    /// `M_PI_2`
    HalfPi,
    /// `M_PI + M_PI_2` (rounded double sum)
    ThreeHalfPi,
    /// `M_PI`
    Pi,
    /// `2 * M_PI`
    TwoPi,
    /// `0`
    Zero,
}

impl DoubleConst {
    fn dy(self) -> Dy {
        match self {
            DoubleConst::HalfPi => M_PI_2,
            DoubleConst::ThreeHalfPi => M_PI.add(M_PI_2).to_f64(),
            DoubleConst::Pi => M_PI,
            DoubleConst::TwoPi => Dy { m: M_PI.m, e: M_PI.e + 1 },
            DoubleConst::Zero => Dy::zero(),
        }
    }

    /// `float tempR = <constante double>;`
    pub fn as_f32(self) -> F32 {
        self.dy().to_f32()
    }
}

/// `f >= (double)c` in double (f promoted exactly).
pub fn ge_double(f: F32, c: DoubleConst) -> bool {
    !f.dy().cmp(c.dy()).is_lt()
}

/// `f < (double)c`.
pub fn lt_double(f: F32, c: DoubleConst) -> bool {
    f.dy().cmp(c.dy()).is_lt()
}

/// `f -= c` / `f += c` with a double c: double computation then float.
pub fn add_double(f: F32, c: DoubleConst, negate: bool) -> F32 {
    let c = if negate { c.dy().neg() } else { c.dy() };
    via_double(f.dy().add(c))
}

/// `(double)a - (double)b < M_PI`: `linkGunDirec - tempR < M_PI`, where
/// `linkGunDirec - tempR` is evaluated in float (two float operands) then
/// compared to the double.
pub fn float_diff_lt_double(a: F32, b: F32, c: DoubleConst) -> bool {
    lt_double(a.sub(b), c)
}

/// `roundf(direction * (16 / (2 * M_PI)))`: `16 / (2 * M_PI)` is a double,
/// the product is computed in double (direction promoted), then `roundf` receives
/// the double converted to float.
pub fn round_dir16(direction: F32) -> i64 {
    // 16 / (2π) rounded to double: 0x145F306DC9C883 · 2^-51 (8/π).
    const K: Dy = Dy { m: 0x145F306DC9C883, e: -51 };
    let d = direction.dy();
    let p = Dy { m: d.m * K.m, e: d.e + K.e }.to_f64().to_f32();
    roundf_dy(p.dy())
}

// --- Additional Destruct operations (destruct.c) -------------------------

#[allow(clippy::should_implement_trait)] // C operations named like the C ones, explicit rounding
impl F32 {
    /// Builds a float from its bits (constants `0.01f`, `0.05f`...).
    pub const fn from_bits(b: u32) -> F32 {
        F32(b)
    }

    /// `(float)u` for a 32-bit unsigned integer (round to nearest).
    pub fn from_u32(v: u32) -> F32 {
        Dy { m: v as i128, e: 0 }.to_f32()
    }

    /// `rng.c:rng_next_float`: `(float)(bits24) / (float)(1 << 24)`, exact.
    pub fn from_unit24(bits: u32) -> F32 {
        Dy { m: (bits & 0xFF_FFFF) as i128, e: -24 }.to_f32()
    }

    /// `-a`.
    pub fn neg(self) -> F32 {
        F32(self.0 ^ 0x8000_0000)
    }

    /// `a / b` in float (quotient rounded only once).
    pub fn div(self, o: F32) -> F32 {
        D64(self.dy()).div(D64(o.dy())).0.round(24).to_f32_exact()
    }

    /// `a > b`.
    pub fn gt(self, o: F32) -> bool {
        o.lt(self)
    }

    /// `a == b` (zeros of opposite signs are equal, like the C).
    pub fn eq_f(self, o: F32) -> bool {
        self.dy().cmp(o.dy()).is_eq()
    }

    /// `roundf(a)` (half away from zero), integer value.
    pub fn roundf(self) -> i64 {
        roundf_dy(self.dy())
    }

    /// `floorf(a)`, integer value.
    pub fn floorf(self) -> i64 {
        let d = self.dy();
        if d.e >= 0 {
            return (d.m << d.e) as i64;
        }
        let s = (-d.e) as u32;
        if s >= 120 {
            return if d.m < 0 { -1 } else { 0 };
        }
        // Euclidean division towards -inf.
        (d.m >> s) as i64
    }

    /// C conversion `(int)f` / `(unsigned)f`: truncation towards zero.
    pub fn trunc(self) -> i64 {
        let d = self.dy();
        if d.e >= 0 {
            return (d.m << d.e) as i64;
        }
        let s = (-d.e) as u32;
        if s >= 120 {
            return 0;
        }
        let q = (d.m.unsigned_abs() >> s) as i64;
        if d.m < 0 {
            -q
        } else {
            q
        }
    }
}

impl Dy {
    /// Value already rounded to 24 bits -> float (no further rounding).
    fn to_f32_exact(self) -> F32 {
        self.to_f32()
    }
}

/// IEEE-754 `double`, carried by its exact dyadic value (already rounded to
/// 53 bits): mixed float/double expressions of `destruct.c`
/// (`rng_next_float() * M_PI / 50 + 0.01f`, `angle * 9.99f / M_PI`...).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct D64(Dy);

#[allow(clippy::should_implement_trait)] // C operations named like the C ones, explicit rounding
impl D64 {
    /// `(double)f`: exact.
    pub fn from_f32(f: F32) -> D64 {
        D64(f.dy())
    }

    /// `(double)n`: exact below 2^53.
    pub fn from_int(v: i64) -> D64 {
        D64(Dy { m: v as i128, e: 0 }.to_f64())
    }

    /// `M_PI`.
    pub fn pi() -> D64 {
        D64(M_PI)
    }

    /// `M_PI_2`.
    pub fn half_pi() -> D64 {
        D64(M_PI_2)
    }

    /// `M_PI_4`.
    pub fn quarter_pi() -> D64 {
        D64(Dy { m: M_PI.m, e: M_PI.e - 2 })
    }

    /// `2 * M_PI` (exact: power of two).
    pub fn two_pi() -> D64 {
        D64(Dy { m: M_PI.m, e: M_PI.e + 1 })
    }

    pub fn add(self, o: D64) -> D64 {
        D64(self.0.add(o.0).to_f64())
    }

    pub fn sub(self, o: D64) -> D64 {
        D64(self.0.sub(o.0).to_f64())
    }

    pub fn mul(self, o: D64) -> D64 {
        D64(self.0.mul(o.0).to_f64())
    }

    /// `a / b` in double: quotient over more than 64 bits plus a sticky bit,
    /// then rounded to nearest (ties to even).
    pub fn div(self, o: D64) -> D64 {
        let (a, b) = (self.0, o.0);
        assert!(b.m != 0, "division by zero");
        if a.m == 0 {
            return D64(Dy::zero());
        }
        let neg = (a.m < 0) != (b.m < 0);
        let (am, bm) = (a.m.unsigned_abs(), b.m.unsigned_abs());
        // Numerator shifted so that the quotient has at least 66 bits.
        let shift = (66 + bit_len(bm) as i32 - bit_len(am) as i32).max(0) as u32;
        assert!(bit_len(am) + shift < 127, "division out of range");
        let num = am << shift;
        let q = num / bm;
        let r = num % bm;
        let q = (q << 1) | (r != 0) as u128;
        let m = q as i128;
        let d = Dy { m: if neg { -m } else { m }, e: a.e - b.e - shift as i32 - 1 };
        D64(d.to_f64())
    }

    /// `(float)d`: round to nearest.
    pub fn to_f32(self) -> F32 {
        self.0.to_f32()
    }

    pub fn lt(self, o: D64) -> bool {
        self.0.cmp(o.0).is_lt()
    }

    pub fn gt(self, o: D64) -> bool {
        o.0.cmp(self.0).is_lt()
    }
}

// --- sinf / cosf (glibc 2.39, FMA variant) ------------------------------

/// Builds a double from its bits.
fn dbits(b: u64) -> Dy {
    let sign = b >> 63;
    let field = ((b >> 52) & 0x7FF) as i32;
    let frac = (b & ((1u64 << 52) - 1)) as i128;
    let (m, e) = if field == 0 { (frac, -1074) } else { (frac | 1 << 52, field - 1023 - 52) };
    Dy { m: if sign == 1 { -m } else { m }, e }
}

/// `sincos_t` from `s_sincosf_data.c` (`__sincosf_table`), without `TOINT_INTRINSICS`.
struct SinCosTable {
    hpi_inv: u64,
    hpi: u64,
    c: [u64; 5],
    s: [u64; 3],
}

const TABLE: [SinCosTable; 2] = [
    SinCosTable {
        hpi_inv: 0x4164_5F30_6DC9_C883, // 0x1.45F306DC9C883p+23
        hpi: 0x3FF9_21FB_5444_2D18,     // 0x1.921FB54442D18p0
        c: [
            0x3FF0_0000_0000_0000, // 0x1p0
            0xBFDF_FFFF_FD0C_621C, // -0x1.ffffffd0c621cp-2
            0x3FA5_5553_E106_8F19, // 0x1.55553e1068f19p-5
            0xBF56_C087_E89A_359D, // -0x1.6c087e89a359dp-10
            0x3EF9_9343_027B_F8C3, // 0x1.99343027bf8c3p-16
        ],
        s: [
            0xBFC5_5554_5995_A603, // -0x1.555545995a603p-3
            0x3F81_1076_0523_0BC4, // 0x1.1107605230bc4p-7
            0xBF29_94EB_3774_CF24, // -0x1.994eb3774cf24p-13
        ],
    },
    SinCosTable {
        hpi_inv: 0x4164_5F30_6DC9_C883,
        hpi: 0x3FF9_21FB_5444_2D18,
        c: [
            0xBFF0_0000_0000_0000,
            0x3FDF_FFFF_FD0C_621C,
            0xBFA5_5553_E106_8F19,
            0x3F56_C087_E89A_359D,
            0xBEF9_9343_027B_F8C3,
        ],
        s: [0xBFC5_5554_5995_A603, 0x3F81_1076_0523_0BC4, 0xBF29_94EB_3774_CF24],
    },
];

/// `sincosf_poly.h:sinf_poly`, each `a + b * c` fused into an FMA.
fn sinf_poly(x: Dy, x2: Dy, p: &SinCosTable, n: i32) -> Dy {
    let c = |i: usize| dbits(p.c[i]);
    let s = |i: usize| dbits(p.s[i]);
    if n & 1 == 0 {
        let x3 = x.dmul(x2);
        let s1 = Dy::dfma(x2, s(2), s(1));
        let x7 = x3.dmul(x2);
        let sv = Dy::dfma(x3, s(0), x);
        Dy::dfma(x7, s1, sv)
    } else {
        let x4 = x2.dmul(x2);
        let c2 = Dy::dfma(x2, c(4), c(3));
        let c1 = Dy::dfma(x2, c(1), c(0));
        let x6 = x4.dmul(x2);
        let cv = Dy::dfma(x4, c(2), c1);
        Dy::dfma(x6, c2, cv)
    }
}

/// `s_sincosf.h:reduce_fast` (without `TOINT_INTRINSICS`): quadrant n and
/// `x - n * hpi` (fused into an FMA).
fn reduce_fast(x: Dy, p: &SinCosTable) -> (Dy, i32) {
    let r = x.dmul(dbits(p.hpi_inv));
    // (int32_t)r: truncation towards zero.
    let t = if r.e >= 0 { r.m << r.e } else { r.m / (1i128 << (-r.e).min(126)) };
    let n = ((t as i32).wrapping_add(0x80_0000)) >> 24;
    (Dy::dfma(Dy { m: -(n as i128), e: 0 }, dbits(p.hpi), x), n)
}

/// `s_sincosf_data.c:__inv_pio4`: 4/π on 192 bits, 8 new bits per entry.
const INV_PIO4: [u32; 24] = [
    0xa2, 0xa2f9, 0xa2f983, 0xa2f9836e, 0xf9836e4e, 0x836e4e44, 0x6e4e4415, 0x4e441529, 0x441529fc, 0x1529fc27,
    0x29fc2757, 0xfc2757d1, 0x2757d1f5, 0x57d1f534, 0xd1f534dd, 0xf534ddc0, 0x34ddc0db, 0xddc0db62, 0xc0db6295,
    0xdb629599, 0x6295993c, 0x95993c43, 0x993c4390, 0x3c439041,
];

/// `s_sincosf.h:reduce_large`: Payne-Hanek reduction in 64-bit integers
/// (exact), then `(double)(int64_t)res0 * pi63` (a single double rounding).
fn reduce_large(xi: u32) -> (Dy, i32) {
    let arr = &INV_PIO4[((xi >> 26) & 15) as usize..];
    let shift = (xi >> 23) & 7;
    let xi = ((xi & 0xff_ffff) | 0x80_0000) << shift;
    let res0 = xi.wrapping_mul(arr[0]) as u64;
    let res1 = xi as u64 * arr[4] as u64;
    let res2 = xi as u64 * arr[8] as u64;
    let mut res0 = (res2 >> 32) | (res0 << 32);
    res0 = res0.wrapping_add(res1);
    let n = res0.wrapping_add(1u64 << 61) >> 62;
    res0 = res0.wrapping_sub(n << 62);
    // (double)(int64_t)res0: rounded to 53 bits, then the product rounded.
    let x = Dy { m: res0 as i64 as i128, e: 0 }.to_f64();
    let pi63 = dbits(0x3C19_21FB_5444_2D18); // 0x1.921FB54442D18p-62
    (x.dmul(pi63), n as i32)
}

fn abstop12(f: F32) -> u32 {
    (f.0 >> 20) & 0x7FF
}

fn sincos(y: F32, cos: bool) -> F32 {
    const PIO4: F32 = F32(0x3F49_0FDB); // 0x1.921FB6p-1f
    const TINY: F32 = F32(0x3980_0000); // 0x1p-12f
    const LIMIT: F32 = F32(0x42F0_0000); // 120.0f
    let x = y.dy();
    let p = &TABLE[0];
    if abstop12(y) < abstop12(PIO4) {
        if abstop12(y) < abstop12(TINY) {
            return if cos { F32(0x3F80_0000) } else { y };
        }
        let x2 = x.dmul(x);
        return sinf_poly(x, x2, p, if cos { 1 } else { 0 }).to_f32();
    }
    if abstop12(y) >= abstop12(LIMIT) {
        // `optionSatelliteRotate` (orbiting sidekicks) grows without bound:
        // |y| >= 120 goes through `reduce_large` (s_sinf.c / s_cosf.c).
        assert!(abstop12(y) < 0x7F8, "sinf/cosf: infinity or NaN");
        let sign = (y.0 >> 31) as i32;
        let (xr, n) = reduce_large(y.0);
        let q = n.wrapping_add(sign);
        let s = if q & 3 == 1 || q & 3 == 2 { -1 } else { 1 };
        let p = if q & 2 != 0 { &TABLE[1] } else { p };
        let xs = Dy { m: xr.m * s, e: xr.e };
        let x2 = xr.dmul(xr);
        return sinf_poly(xs, x2, p, if cos { n ^ 1 } else { n }).to_f32();
    }
    let (xr, n) = reduce_fast(x, p);
    let sign = if n & 3 == 1 || n & 3 == 2 { -1 } else { 1 };
    let p = if n & 2 != 0 { &TABLE[1] } else { p };
    let xs = Dy { m: xr.m * sign, e: xr.e };
    let x2 = xr.dmul(xr);
    sinf_poly(xs, x2, p, if cos { n ^ 1 } else { n }).to_f32()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hw(f: F32) -> f32 {
        f32::from_bits(f.0)
    }

    #[test]
    fn arithmetic_matches_hardware() {
        let mut x = F32::m_pi();
        assert_eq!(hw(x), std::f64::consts::PI as f32);
        for i in 0..2000 {
            let up = i % 7 < 4;
            let y = if up { x.add(F_0_2) } else { x.sub(F_0_2) };
            let h = if up { hw(x) + 0.2f32 } else { hw(x) - 0.2f32 };
            assert_eq!(hw(y), h);
            x = y;
            let wrapped = add_double(x, DoubleConst::TwoPi, true);
            assert_eq!(hw(wrapped), (hw(x) as f64 - 2.0 * std::f64::consts::PI) as f32);
            let plus = add_double(x, DoubleConst::TwoPi, false);
            assert_eq!(hw(plus), (hw(x) as f64 + 2.0 * std::f64::consts::PI) as f32);
            assert_eq!(round_dir16(x), ((hw(x) as f64 * (16.0 / (2.0 * std::f64::consts::PI))) as f32).round() as i64);
        }
        let tr = DoubleConst::ThreeHalfPi.as_f32();
        assert_eq!(hw(tr), (std::f64::consts::PI + std::f64::consts::FRAC_PI_2) as f32);
        assert_eq!(f32::from_bits(F_0_3), 0.3f32);
        assert_eq!(f32::from_bits(F_MINUS_3_24), -3.24f32);
    }

    /// Compares `sinf`/`cosf` and `roundf(sinf(x) * k)` against the libm of the
    /// machine on a dense sample of [-8, 8].
    #[test]
    fn sincos_match_hardware() {
        let mut checked = 0;
        let mut bits = 0x3000_0000u32; // ~4.6e-10
        while bits < 0x4100_0000 {
            for s in [0u32, 1 << 31] {
                let f = F32(bits | s);
                let v = hw(f);
                assert_eq!(f.sin().0, v.sin().to_bits(), "sinf({v:e})");
                assert_eq!(f.cos().0, v.cos().to_bits(), "cosf({v:e})");
                checked += 1;
            }
            bits += 997;
        }
        assert!(checked > 100_000);
    }

    /// Large angles (`reduce_large`, |x| >= 120): the angle of orbiting
    /// sidekicks (`optionSatelliteRotate`) exceeds 1000 radians in one level.
    #[test]
    fn sincos_large_match_hardware() {
        let mut bits = 0x42F0_0000u32; // 120
        while bits < 0x4F00_0000 {
            for s in [0u32, 1 << 31] {
                let f = F32(bits | s);
                let v = hw(f);
                assert_eq!(f.sin().0, v.sin().to_bits(), "sinf({v:e})");
                assert_eq!(f.cos().0, v.cos().to_bits(), "cosf({v:e})");
            }
            bits += 211;
        }
    }

    /// Exhaustive sweep of [0.25, 8) (≈ 1.3·10^7 floats per sign):
    /// `cargo test --release -p tyrian-sim -- --ignored sincos_exhaustive`.
    #[test]
    #[ignore]
    fn sincos_exhaustive() {
        for bits in 0x3E80_0000u32..0x4100_0000 {
            for s in [0u32, 1 << 31] {
                let f = F32(bits | s);
                let v = hw(f);
                assert_eq!(f.sin().0, v.sin().to_bits(), "sinf({v:e})");
                assert_eq!(f.cos().0, v.cos().to_bits(), "cosf({v:e})");
            }
        }
    }

    #[test]
    fn mul_roundf() {
        for bits in (0x3000_0000u32..0x4100_0000).step_by(1009) {
            for s in [0u32, 1 << 31] {
                let f = F32(bits | s).sin();
                for k in [20i64, 26, 3, 7] {
                    assert_eq!(f.mul_int_roundf(k), (hw(f) * k as f32).round() as i64);
                }
            }
        }
    }

    #[test]
    fn destruct_float_ops_match_hardware() {
        let pi = std::f64::consts::PI;
        let mut seed = 12345u32;
        for _ in 0..20000 {
            seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let bits = seed >> 8;
            let f = F32::from_unit24(bits);
            let hf = bits as f32 / (1u32 << 24) as f32;
            assert_eq!(hw(f), hf);
            // rng_next_float() * M_PI / 50 + 0.01f
            let a = D64::from_f32(f)
                .mul(D64::pi())
                .div(D64::from_int(50))
                .add(D64::from_f32(F32(0.01f32.to_bits())))
                .to_f32();
            assert_eq!(hw(a), (hf as f64 * pi / 50.0 + 0.01f32 as f64) as f32);
            // rng_next_float() * (2 * M_PI)
            let r = D64::from_f32(f).mul(D64::two_pi()).to_f32();
            assert_eq!(hw(r), (hf as f64 * (2.0 * pi)) as f32);
            // angle * 9.99f / M_PI, floorf
            let k = D64::from_f32(f.mul(F32(9.99f32.to_bits()))).div(D64::pi()).to_f32();
            assert_eq!(hw(k), ((hf * 9.99f32) as f64 / pi) as f32);
            assert_eq!(k.floorf(), (((hf * 9.99f32) as f64 / pi) as f32).floor() as i64);
            let v = F32((seed & 0x3FFF_FFFF) | ((seed & 1) << 31));
            assert_eq!(v.roundf(), hw(v).round() as i64);
            assert_eq!(v.floorf(), hw(v).floor() as i64);
            assert_eq!(v.trunc(), hw(v).trunc() as i64);
            let w = F32::from_int((seed % 400) as i64 - 200).add(f);
            assert_eq!(hw(w.div(F32::from_int(3))), hw(w) / 3.0);
            assert_eq!(hw(w.neg()), -hw(w));
        }
        assert_eq!(
            hw(D64::half_pi().sub(D64::from_f32(F32(0.01f32.to_bits()))).to_f32()),
            (std::f64::consts::FRAC_PI_2 - 0.01f32 as f64) as f32
        );
        assert_eq!(hw(D64::quarter_pi().to_f32()), std::f64::consts::FRAC_PI_4 as f32);
        assert_eq!(hw(F32::from_u32(4_294_967_295)), 4_294_967_295u32 as f32);
    }
}
