//! RBWE01's precomputed floating-point random stream, not a new PRNG policy.
//! Startup builds 1024 entries; the current cursor must be explicitly supplied.
use crate::{Result, lua50::Value};
use std::sync::Arc;

pub const TABLE_LEN: usize = 1024;
#[derive(Clone, Debug, PartialEq)]
pub struct Stream {
    table: Arc<[f32; TABLE_LEN]>,
    cursor: usize,
}
impl Stream {
    /// Zero is the fresh engine cursor, NOT a recovered post-menu/level-load cursor.
    pub fn at_cursor(cursor: usize) -> Result<Self> {
        if cursor >= TABLE_LEN {
            return Err("Random cursor outside native table".into());
        }
        // 0x8003f654; caller 0x8003d048 requests 0x400 entries. The
        // descending fill order and bits 4..18 are both significant.
        let mut seed = 0xad78_9fe6_u32;
        let mut table = [0.0; TABLE_LEN];
        for value in table.iter_mut().rev() {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            *value = ((seed >> 4) & 0x7fff) as f32 / 32768.0;
        }
        Ok(Self {
            table: Arc::new(table),
            cursor,
        })
    }
    pub fn cursor(&self) -> usize {
        self.cursor
    }
    /// GetRandom's zero/one/two/three-number signatures. The supported
    /// subset requires ordered finite f32 bounds and a positive finite step.
    /// Quantization is toward zero, relative to zero (not the lower bound).
    pub fn call(&mut self, args: &[Value]) -> Result<Vec<Value>> {
        fn number(value: &Value) -> Result<f32> {
            let Value::Number(n) = value else {
                return Err("Random argument must be numeric".into());
            };
            let n = *n as f32;
            if !n.is_finite() {
                return Err("Non-finite native random argument".into());
            }
            Ok(n)
        }
        let (low, high, step) = match args {
            [] => (0.0, 1.0, None),
            [high] => (0.0, number(high)?, None),
            [low, high] => (number(low)?, number(high)?, None),
            [low, high, step] => (number(low)?, number(high)?, Some(number(step)?)),
            _ => return Err("Unsupported GetRandom signature".into()),
        };
        let span = high - low;
        if low > high || !span.is_finite() || step.is_some_and(|n| n <= 0.0) {
            return Err("Unsupported native random range/step".into());
        }
        // Native advances BEFORE indexing (0x8003f744). Keep multiplication,
        // addition, remainder conversion and subtraction separate as in PPC.
        let next = (self.cursor + 1) % TABLE_LEN;
        let scaled = span * self.table[next];
        let mut value = if args.len() < 2 { scaled } else { low + scaled };
        if let Some(step) = step {
            // 0x800d5124 -> 0x8026128c -> double fmod at 0x8025db68.
            let remainder = (f64::from(value) % f64::from(step)) as f32;
            value -= remainder;
        }
        if !value.is_finite() {
            return Err("Non-finite native random result".into());
        }
        self.cursor = next;
        Ok(vec![Value::Number(f64::from(value))])
    }
}

/// Native integer draws use the same table but a DIFFERENT cursor (r13-0x3778).
/// Never substitute the Lua floating stream's cursor for this state.
#[derive(Clone, Debug, PartialEq)]
pub struct IntegerStream(Stream);
impl IntegerStream {
    pub fn at_cursor(cursor: usize) -> Result<Self> {
        Ok(Self(Stream::at_cursor(cursor)?))
    }
    pub fn cursor(&self) -> usize {
        self.0.cursor
    }
    /// 0x8003f834: advance, unsigned high -> f32, multiply, truncate to u32.
    /// A zero bound still consumes a draw. This does not recover session continuity.
    pub fn draw(&mut self, high: u32) -> u32 {
        let next = (self.0.cursor + 1) % TABLE_LEN;
        let value = high as f32 * self.0.table[next];
        self.0.cursor = next;
        value as u32
    }
}

/// Native C-runtime LCG (`0x80258a60`, `r13 - 0x4ac0` = `0x805fc6a0`, initial seed `1`).
/// Used by `cWaypoint::GetNextWaypoint` (`0x8003dfcc`), separate from the 1024-entry table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrtRand {
    state: u32,
}
impl Default for CrtRand {
    fn default() -> Self {
        Self::new(1)
    }
}
impl CrtRand {
    pub const fn new(seed: u32) -> Self {
        Self { state: seed }
    }
    pub const fn state(&self) -> u32 {
        self.state
    }
    /// `0x80258a60`: `state = state * 1103515245 + 12345; (state >> 16) & 0x7fff`.
    pub fn next_u15(&mut self) -> u32 {
        self.state = self.state.wrapping_mul(1_103_515_245).wrapping_add(12_345);
        (self.state >> 16) & 0x7fff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn numbers(values: &[f64]) -> Vec<Value> {
        values.iter().copied().map(Value::Number).collect()
    }
    #[test]
    fn native_table_order_cursor_and_wrap() {
        let mut s = Stream::at_cursor(0).unwrap();
        assert_eq!(s.call(&[]).unwrap(), numbers(&[0.74212646484375]));
        assert_eq!(s.call(&[]).unwrap(), numbers(&[0.711700439453125]));
        let mut last = Stream::at_cursor(1023).unwrap();
        assert_eq!(last.call(&[]).unwrap(), numbers(&[0.44720458984375]));
        assert_eq!(last.cursor(), 0);
        assert!(Stream::at_cursor(1024).is_err());
    }
    #[test]
    fn source_music_quantization_and_signed_ranges() {
        for (args, result) in [
            (&[0., 90., 10.][..], 60.),
            (&[5., 9., 10.][..], 0.),
            (&[-9., -5., 10.][..], 0.),
            (&[3., 3.][..], 3.),
        ] {
            let mut s = Stream::at_cursor(0).unwrap();
            assert_eq!(s.call(&numbers(args)).unwrap(), numbers(&[result]));
            assert_eq!(s.cursor(), 1); // Even a zero-width interval consumes a draw.
        }
        assert_eq!(
            Stream::at_cursor(0).unwrap().call(&numbers(&[2.])).unwrap(),
            numbers(&[1.4842529296875])
        );
    }
    #[test]
    fn integer_cursor_is_independent_and_zero_bounds_consume_draws() {
        let mut floating = Stream::at_cursor(0).unwrap();
        let mut integer = IntegerStream::at_cursor(0).unwrap();
        assert_eq!(integer.draw(100), 74);
        assert_eq!(integer.draw(100), 71);
        assert_eq!(floating.cursor(), 0);
        floating.call(&[]).unwrap();
        assert_eq!(integer.cursor(), 2);
        assert_eq!(integer.draw(0), 0);
        assert_eq!(integer.cursor(), 3);
        assert!(IntegerStream::at_cursor(TABLE_LEN).is_err());
    }
    #[test]
    fn integer_wrap_and_unsigned_bound_use_native_float_product() {
        let mut integer = IntegerStream::at_cursor(TABLE_LEN - 1).unwrap();
        assert_eq!(integer.draw(100), 44);
        assert_eq!(integer.cursor(), 0);
        let mut integer = IntegerStream::at_cursor(0).unwrap();
        assert_eq!(integer.draw(u32::MAX), 3_187_408_896);
        let mut copy = integer.clone();
        copy.draw(100);
        assert_eq!(integer.cursor(), 1);
        assert_eq!(copy.cursor(), 2);
    }
    #[test]
    fn invalid_calls_leave_cursor_unchanged() {
        let mut s = Stream::at_cursor(42).unwrap();
        for args in [
            numbers(&[2., 1.]),
            numbers(&[0., 1., 0.]),
            numbers(&[0., 1., -1.]),
            numbers(&[f64::MAX]),
            numbers(&[-f64::from(f32::MAX), f64::from(f32::MAX)]),
            numbers(&[f64::NAN]),
            numbers(&[1., 2., 3., 4.]),
            vec![Value::Boolean(true)],
        ] {
            assert!(s.call(&args).is_err());
            assert_eq!(s.cursor(), 42);
        }
    }
    #[test]
    fn crt_rand_matches_ppc_lcg_sequence() {
        let mut rng = CrtRand::default();
        assert_eq!(rng.state(), 1);
        assert_eq!(rng.next_u15(), 16_838);
        assert_eq!(rng.state(), 1_103_527_590);
        assert_eq!(rng.next_u15(), 5_758);
        assert_eq!(rng.next_u15(), 10_113);
    }
}
