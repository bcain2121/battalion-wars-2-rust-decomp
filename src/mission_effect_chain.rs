//! Bounded explosion setup selection and update's delayed-chain tail.
//! This does not create effects or install a Kill backend.
use crate::{ObjectId, Result, mission_random::IntegerStream};

#[derive(Clone, Debug, PartialEq)]
pub struct Tail {
    remaining: f32,
    first: Option<ObjectId>,
    second: Option<ObjectId>,
    probability: i8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    NotDue,
    /// A null selection still consumes the native integer random draw.
    Selected(Option<ObjectId>),
}

impl Tail {
    pub fn new(
        delay: f32,
        first: Option<ObjectId>,
        second: Option<ObjectId>,
        probability: i8,
    ) -> Result<Self> {
        if !delay.is_finite() {
            return Err("Non-finite effect chain delay".into());
        }
        if [first, second].into_iter().flatten().any(|id| id.0 == 0) {
            return Err("Null effect chain reference must be None".into());
        }
        Ok(Self {
            remaining: delay,
            first,
            second,
            probability,
        })
    }
    pub fn remaining(&self) -> f32 {
        self.remaining
    }

    /// Setup selection at 0x800bb498..0x800bb4f8. Only null/deferred children
    /// are supported; an immediate child requires a real creation backend.
    /// `child_delay` must be a read-only lookup of the SELECTED profile's +0xb4.
    /// This is not the parent's delay, and the selected child is not cached for expiry.
    /// Both the integer cursor and this timer commit only on success. A caller must
    /// still transactionally enclose the rest of explosion construction.
    pub fn initialize_delay(
        &mut self,
        random: &mut IntegerStream,
        child_delay: impl FnOnce(ObjectId) -> Result<f32>,
    ) -> Result<Option<ObjectId>> {
        let mut staged_random = random.clone();
        let selected = self.select(&mut staged_random);
        let mut remaining = self.remaining;
        if let Some(id) = selected {
            let delay = child_delay(id)?;
            if !delay.is_finite() {
                return Err("Non-finite selected effect chain delay".into());
            }
            // Native zero comparison includes negative zero. Creation happens before
            // storing -1; do not consume the draw or fake that creation here.
            if delay == 0.0 {
                return Err("Immediate effect chain creation backend not configured".into());
            }
            if remaining == 0.0 {
                remaining = delay;
            }
        }
        self.remaining = remaining;
        *random = staged_random;
        Ok(selected)
    }

    fn select(&self, random: &mut IntegerStream) -> Option<ObjectId> {
        // 0x800ba668..88: sign-extend the byte, then UNSIGNED compare.
        let threshold = i32::from(self.probability) as u32;
        if random.draw(100) <= threshold {
            self.first
        } else {
            self.second
        }
    }

    /// Call only when control flow reaches 0x800bcef4..0x800bcf98.
    /// One subtraction and at most one selection; no catch-up loop or child creation.
    pub fn advance(&mut self, delta: f32, random: &mut IntegerStream) -> Result<Event> {
        if !delta.is_finite() || delta < 0.0 {
            return Err("Unsupported effect chain delta".into());
        }
        if self.remaining <= 0.0 {
            return Ok(Event::NotDue);
        }
        self.remaining -= delta;
        if self.remaining > 0.0 {
            return Ok(Event::NotDue);
        }
        Ok(Event::Selected(self.select(random)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_selects_delay_then_expiry_selects_again() {
        let mut random = IntegerStream::at_cursor(0).unwrap(); // draws 74, then 71
        let mut tail = Tail::new(0.0, Some(ObjectId(1)), Some(ObjectId(2)), 72).unwrap();
        assert_eq!(
            tail.initialize_delay(&mut random, |id| {
                assert_eq!(id, ObjectId(2));
                Ok(0.1)
            })
            .unwrap(),
            Some(ObjectId(2))
        );
        assert_eq!(random.cursor(), 1);
        assert_eq!(tail.remaining(), 0.1);
        assert_eq!(
            tail.advance(0.1, &mut random).unwrap(),
            Event::Selected(Some(ObjectId(1)))
        );
        assert_eq!(random.cursor(), 2);
    }
    #[test]
    fn setup_null_and_existing_delay_preserve_native_order() {
        let mut random = IntegerStream::at_cursor(0).unwrap();
        let mut null = Tail::new(3.0, None, None, 50).unwrap();
        assert_eq!(
            null.initialize_delay(&mut random, |_| panic!("null profile read"))
                .unwrap(),
            None
        );
        assert_eq!(null.remaining(), 3.0);
        assert_eq!(random.cursor(), 1);
        for current in [3.0, -1.0] {
            let mut tail = Tail::new(current, Some(ObjectId(1)), Some(ObjectId(1)), 50).unwrap();
            assert_eq!(
                tail.initialize_delay(&mut random, |_| Ok(0.1)).unwrap(),
                Some(ObjectId(1))
            );
            assert_eq!(tail.remaining(), current);
        }
        let mut tail = Tail::new(-0.0, Some(ObjectId(1)), Some(ObjectId(1)), 50).unwrap();
        tail.initialize_delay(&mut random, |_| Ok(-2.0)).unwrap();
        assert_eq!(tail.remaining(), -2.0);
        assert_eq!(random.cursor(), 4);
    }
    #[test]
    fn setup_missing_invalid_and_immediate_child_roll_back() {
        for delay in [0.0, -0.0, f32::NAN, f32::INFINITY] {
            for current in [0.0, 5.0] {
                let mut random = IntegerStream::at_cursor(0).unwrap();
                let mut tail =
                    Tail::new(current, Some(ObjectId(1)), Some(ObjectId(1)), 50).unwrap();
                let before = tail.clone();
                assert!(tail.initialize_delay(&mut random, |_| Ok(delay)).is_err());
                assert_eq!(tail, before);
                assert_eq!(random.cursor(), 0);
            }
        }
        let mut random = IntegerStream::at_cursor(0).unwrap();
        let mut tail = Tail::new(0.0, Some(ObjectId(1)), Some(ObjectId(1)), 50).unwrap();
        assert!(
            tail.initialize_delay(&mut random, |_| Err("Unknown child profile".into()))
                .is_err()
        );
        assert_eq!(tail.remaining(), 0.0);
        assert_eq!(random.cursor(), 0);
    }
    #[test]
    fn chain_uses_inclusive_native_unsigned_probability() {
        for (probability, expected) in [
            (73, Some(ObjectId(2))),
            (74, Some(ObjectId(1))),
            (-1, Some(ObjectId(1))),
        ] {
            let mut random = IntegerStream::at_cursor(0).unwrap(); // first draw 74
            let mut tail =
                Tail::new(0.1, Some(ObjectId(1)), Some(ObjectId(2)), probability).unwrap();
            assert_eq!(
                tail.advance(0.1, &mut random).unwrap(),
                Event::Selected(expected)
            );
            assert_eq!(random.cursor(), 1);
        }
    }
    #[test]
    fn same_child_and_null_child_still_consume_draw_but_not_repeatedly() {
        for child in [Some(ObjectId(250009268)), None] {
            let mut random = IntegerStream::at_cursor(0).unwrap();
            let mut tail = Tail::new(0.1, child, child, 50).unwrap();
            assert_eq!(tail.advance(0.0, &mut random).unwrap(), Event::NotDue);
            assert_eq!(random.cursor(), 0);
            assert_eq!(
                tail.advance(1.0, &mut random).unwrap(),
                Event::Selected(child)
            );
            assert!(tail.remaining() < 0.0);
            assert_eq!(tail.advance(1.0, &mut random).unwrap(), Event::NotDue);
            assert_eq!(random.cursor(), 1);
        }
    }
    #[test]
    fn disabled_and_invalid_timing_do_not_consume_or_mutate() {
        let mut random = IntegerStream::at_cursor(0).unwrap();
        for delay in [0.0, -1.0] {
            let mut tail = Tail::new(delay, None, None, 50).unwrap();
            assert_eq!(tail.advance(1.0, &mut random).unwrap(), Event::NotDue);
            assert_eq!(tail.remaining(), delay);
        }
        let mut tail = Tail::new(0.1, None, None, 50).unwrap();
        let before = tail.clone();
        for delta in [f32::NAN, f32::INFINITY, -0.1] {
            assert!(tail.advance(delta, &mut random).is_err());
            assert_eq!(tail, before);
        }
        assert_eq!(random.cursor(), 0);
        assert!(Tail::new(f32::NAN, None, None, 50).is_err());
        assert!(Tail::new(1.0, Some(ObjectId(0)), None, 50).is_err());
    }
}
