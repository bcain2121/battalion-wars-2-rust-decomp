//! Rust-authored LightningX replacement, not a Tequila bytecode decoder.
//! Camera-local ribbons, one soft flash every four seconds; zero timeout loops.
use crate::{ObjectId, Result};
#[derive(Clone, Debug, PartialEq)]
pub struct Lightning {
    pub source: ObjectId,
    pub offset: [f32; 3],
    ticks: u64,
    timeout: Option<u64>,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Triangle list in camera-local coordinates (camera forward is -Z).
    pub triangles: Vec<[[f32; 3]; 3]>,
    pub colour: [f32; 4],
}
impl Lightning {
    pub fn new(source: ObjectId, offset: [f32; 3], seconds: f64) -> Result<Self> {
        if offset.iter().any(|x| !x.is_finite())
            || !seconds.is_finite()
            || !(0.0..=86400.0).contains(&seconds)
        {
            return Err("Invalid Rust lightning transform/timeout".into());
        }
        Ok(Self {
            source,
            offset,
            ticks: 0,
            timeout: (seconds > 0.0).then(|| (seconds * 60.0).ceil() as u64),
        })
    }
    pub fn advance_tick(&mut self) -> Result<()> {
        self.ticks = self
            .ticks
            .checked_add(1)
            .ok_or("Lightning clock overflow")?;
        Ok(())
    }
    pub fn finished(&self) -> bool {
        self.timeout.is_some_and(|end| self.ticks >= end)
    }
    pub fn frame(&self) -> Frame {
        let phase = self.ticks % 240;
        let alpha = if !self.finished() && phase < 12 {
            (1.0 - phase as f32 / 12.0).powi(2)
        } else {
            0.0
        };
        let mut frame = Frame {
            triangles: Vec::new(),
            colour: [0.7, 0.85, 1.0, alpha],
        };
        if alpha == 0.0 {
            return frame;
        }
        // Deterministic alternating branches; no gameplay/effect random stream consumption.
        let flip = if (self.ticks / 240).is_multiple_of(2) {
            1.0
        } else {
            -1.0
        };
        let trunk = [
            [-25., 55.],
            [-31., 42.],
            [-25., 32.],
            [-34., 19.],
            [-30., 10.],
        ];
        let branch = [[-25., 32.], [-16., 26.], [-19., 20.]];
        for (points, width) in [(&trunk[..], 0.55), (&branch[..], 0.28)] {
            for pair in points.windows(2) {
                let [a, b] = [pair[0], pair[1]];
                let dx: f32 = b[0] - a[0];
                let dy: f32 = b[1] - a[1];
                let len = dx.hypot(dy);
                let nx = -dy / len * width;
                let ny = dx / len * width;
                let point = |p: [f32; 2], s: f32| {
                    [
                        (p[0] + nx * s) * flip + self.offset[0],
                        p[1] + ny * s + self.offset[1],
                        -100.0 + self.offset[2],
                    ]
                };
                let [a0, a1, b0, b1] = [point(a, -1.), point(a, 1.), point(b, -1.), point(b, 1.)];
                frame.triangles.extend([[a0, b0, b1], [a0, b1, a1]]);
            }
        }
        frame
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flash_has_real_geometry_fades_and_repeats_without_randomness() {
        let mut effect = Lightning::new(ObjectId(1), [0.; 3], 0.).unwrap();
        let first = effect.frame();
        assert_eq!(first.triangles.len(), 12);
        for _ in 0..6 {
            effect.advance_tick().unwrap();
        }
        assert!(effect.frame().colour[3] < first.colour[3]);
        for _ in 6..12 {
            effect.advance_tick().unwrap();
        }
        assert!(effect.frame().triangles.is_empty());
        for _ in 12..240 {
            effect.advance_tick().unwrap();
        }
        assert_eq!(effect.frame().triangles.len(), 12);
        assert_ne!(effect.frame().triangles, first.triangles);
        assert!(!effect.finished());
    }
    #[test]
    fn finite_lifetime_and_invalid_configuration() {
        let mut effect = Lightning::new(ObjectId(1), [1., 2., 3.], 0.1).unwrap();
        for _ in 0..6 {
            effect.advance_tick().unwrap();
        }
        assert!(effect.finished());
        assert!(effect.frame().triangles.is_empty());
        assert!(Lightning::new(ObjectId(1), [f32::NAN, 0., 0.], 0.).is_err());
        assert!(Lightning::new(ObjectId(1), [0.; 3], -1.).is_err());
    }
}
