//! Temporary playable skirmish scaffolding, NOT recovered BW2 combat/AI semantics.
use crate::{Result, terrain::Terrain};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug)]
struct DenseGrid {
    min_x: i32,
    min_z: i32,
    stride: usize,
    rows: usize,
    heights: Vec<f32>,
    active: Vec<bool>,
}
impl DenseGrid {
    fn build(
        heights_map: &BTreeMap<(i32, i32), f32>,
        cells_set: &BTreeSet<(i32, i32)>,
    ) -> Option<Self> {
        let (&(min_x, _), &(max_x, _)) = (
            heights_map.first_key_value()?.0,
            heights_map.last_key_value()?.0,
        );
        let min_z = heights_map.keys().map(|&(_, z)| z).min()?;
        let max_z = heights_map.keys().map(|&(_, z)| z).max()?;
        let stride = usize::try_from(max_x.checked_sub(min_x)?.checked_add(2)?).ok()?;
        let rows = usize::try_from(max_z.checked_sub(min_z)?.checked_add(2)?).ok()?;
        let len = stride.checked_mul(rows)?;
        if len > 4_000_000 {
            return None;
        }
        let mut heights = vec![f32::NAN; len];
        let mut active = vec![false; len];
        for (&(x, z), &h) in heights_map {
            let ix = (x - min_x) as usize;
            let iz = (z - min_z) as usize;
            heights[iz * stride + ix] = h;
        }
        for &(x, z) in cells_set {
            if x >= min_x && z >= min_z {
                let ix = (x - min_x) as usize;
                let iz = (z - min_z) as usize;
                if ix + 1 < stride && iz + 1 < rows {
                    let i00 = iz * stride + ix;
                    let i10 = i00 + 1;
                    let i01 = i00 + stride;
                    let i11 = i01 + 1;
                    if heights[i00].is_finite()
                        && heights[i10].is_finite()
                        && heights[i01].is_finite()
                        && heights[i11].is_finite()
                    {
                        active[i00] = true;
                    }
                }
            }
        }
        Some(Self {
            min_x,
            min_z,
            stride,
            rows,
            heights,
            active,
        })
    }
    #[inline]
    fn height(&self, x: f32, z: f32) -> Option<f32> {
        let ix = x.floor() as i32;
        let iz = z.floor() as i32;
        let gx = usize::try_from(ix.checked_sub(self.min_x)?).ok()?;
        let gz = usize::try_from(iz.checked_sub(self.min_z)?).ok()?;
        if gx + 1 >= self.stride || gz + 1 >= self.rows {
            return None;
        }
        let i00 = gz * self.stride + gx;
        if !self.active[i00] {
            return None;
        }
        let a = self.heights[i00];
        let b = self.heights[i00 + 1];
        let c = self.heights[i00 + self.stride];
        let d = self.heights[i00 + self.stride + 1];
        let u = x - ix as f32;
        let v = z - iz as f32;
        Some(if u + v <= 1.0 {
            a + (b - a) * u + (c - a) * v
        } else {
            d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
        })
    }
}

#[derive(Debug)]
pub struct Ground {
    heights: BTreeMap<(i32, i32), f32>,
    cells: BTreeSet<(i32, i32)>,
    grid: Option<DenseGrid>,
}
impl Ground {
    pub fn from_terrain(t: &Terrain) -> Self {
        let heights: BTreeMap<(i32, i32), f32> = t
            .positions
            .iter()
            .map(|p| ((p[0] as i32, p[2] as i32), p[1]))
            .collect();
        let cells: BTreeSet<(i32, i32)> = t
            .indices
            .chunks_exact(6)
            .map(|i| {
                let p = t.positions[i[0] as usize];
                (p[0] as i32, p[2] as i32)
            })
            .collect();
        let grid = DenseGrid::build(&heights, &cells);
        Self {
            heights,
            cells,
            grid,
        }
    }
    #[inline]
    pub fn height(&self, x: f32, z: f32) -> Option<f32> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }
        if let Some(grid) = &self.grid {
            return grid.height(x, z);
        }
        let ix = x.floor() as i32;
        let iz = z.floor() as i32;
        if !self.cells.contains(&(ix, iz)) {
            return None;
        }
        let a = *self.heights.get(&(ix, iz))?;
        let b = *self.heights.get(&(ix + 1, iz))?;
        let c = *self.heights.get(&(ix, iz + 1))?;
        let d = *self.heights.get(&(ix + 1, iz + 1))?;
        let u = x - ix as f32;
        let v = z - iz as f32;
        Some(if u + v <= 1.0 {
            a + (b - a) * u + (c - a) * v
        } else {
            d + (c - d) * (1.0 - u) + (b - d) * (1.0 - v)
        })
    }
    pub fn arena(&self) -> Result<[f32; 2]> {
        let mut best = None;
        for &(x, z) in &self.cells {
            if x % 12 != 0 || z % 12 != 0 {
                continue;
            }
            let mut min = f32::INFINITY;
            let mut max = f32::NEG_INFINITY;
            let mut valid = true;
            for dz in (-20..=20).step_by(2) {
                for dx in (-20..=20).step_by(2) {
                    if let Some(h) = self.height((x + dx) as f32 + 0.25, (z + dz) as f32 + 0.25) {
                        min = min.min(h);
                        max = max.max(h);
                    } else {
                        valid = false;
                    }
                }
            }
            if valid && best.is_none_or(|(_, score)| max - min < score) {
                best = Some(([x as f32 + 0.25, z as f32 + 0.25], max - min));
            }
        }
        best.map(|(p, _)| p)
            .ok_or("No sufficiently contiguous terrain area found".into())
    }
    fn visible(&self, a: [f32; 2], b: [f32; 2]) -> bool {
        let Some(ha) = self.height(a[0], a[1]) else {
            return false;
        };
        let Some(hb) = self.height(b[0], b[1]) else {
            return false;
        };
        let n = (distance(a, b) * 2.0).ceil().max(1.0) as usize;
        for i in 1..n {
            let t = i as f32 / n as f32;
            let x = a[0] + (b[0] - a[0]) * t;
            let z = a[1] + (b[1] - a[1]) * t;
            if self
                .height(x, z)
                .is_none_or(|h| h > ha + 1.0 + (hb - ha) * t)
            {
                return false;
            }
        }
        true
    }
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2)).sqrt()
}
#[derive(Debug, Clone)]
pub struct Enemy {
    pub position: [f32; 2],
    pub health: f32,
    cooldown: f32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Playing,
    Won,
    Lost,
}
#[derive(Debug, Default, Clone, Copy)]
pub struct Input {
    pub forward: f32,
    pub strafe: f32,
    pub turn: f32,
    pub fire: bool,
    pub sprint: bool,
    pub cycle: bool,
    pub restart: bool,
}
#[derive(Debug)]
pub struct Game {
    pub center: [f32; 2],
    pub player: [f32; 2],
    pub yaw: f32,
    pub health: f32,
    pub enemies: Vec<Enemy>,
    pub outcome: Outcome,
    pub target: Option<usize>,
    pub shots: u32,
    pub hits: u32,
    pub last_shot: Option<([f32; 2], [f32; 2])>,
    pub flash: f32,
    pub elapsed: f32,
    cooldown: f32,
}
impl Game {
    pub fn new(center: [f32; 2]) -> Self {
        let enemies = [
            [-12.0, -8.0],
            [-6.0, -12.0],
            [0.0, -14.0],
            [6.0, -12.0],
            [12.0, -8.0],
        ]
        .into_iter()
        .enumerate()
        .map(|(i, p)| Enemy {
            position: [center[0] + p[0], center[1] + p[1]],
            health: 60.0,
            cooldown: 2.5 + i as f32 * 0.4,
        })
        .collect();
        Self {
            center,
            player: [center[0], center[1] + 9.0],
            yaw: 0.0,
            health: 100.0,
            enemies,
            outcome: Outcome::Playing,
            target: None,
            shots: 0,
            hits: 0,
            last_shot: None,
            flash: 0.0,
            elapsed: 0.0,
            cooldown: 0.0,
        }
    }
    pub fn alive(&self) -> usize {
        self.enemies.iter().filter(|e| e.health > 0.0).count()
    }
    pub fn tick(&mut self, ground: &Ground, input: Input, dt: f32) {
        if input.restart {
            *self = Self::new(self.center);
            return;
        }
        if self.outcome != Outcome::Playing || !dt.is_finite() || dt <= 0.0 {
            return;
        }
        let dt = dt.min(0.05);
        self.elapsed += dt;
        self.flash = (self.flash - dt).max(0.0);
        self.cooldown = (self.cooldown - dt).max(0.0);
        self.yaw += input.turn.clamp(-1.0, 1.0) * dt * 1.8;
        let mut dx = input.strafe * self.yaw.cos() + input.forward * self.yaw.sin();
        let mut dz = input.strafe * self.yaw.sin() - input.forward * self.yaw.cos();
        let len = (dx * dx + dz * dz).sqrt().max(1.0);
        dx /= len;
        dz /= len;
        let speed = if input.sprint { 7.0 } else { 4.5 };
        self.player = walk(
            ground,
            self.player,
            [dx * speed * dt, dz * speed * dt],
            self.center,
        );
        let candidates: Vec<_> = self
            .enemies
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                e.health > 0.0
                    && distance(self.player, e.position) < 32.0
                    && ground.visible(self.player, e.position)
            })
            .map(|(i, _)| i)
            .collect();
        if self.target.is_none_or(|i| !candidates.contains(&i)) {
            self.target = candidates.first().copied();
        }
        if input.cycle && !candidates.is_empty() {
            let index = self
                .target
                .and_then(|i| candidates.iter().position(|c| *c == i))
                .unwrap_or(0);
            self.target = Some(candidates[(index + 1) % candidates.len()]);
        }
        if input.fire && self.cooldown == 0.0 {
            self.shots += 1;
            self.cooldown = 0.3;
            self.flash = 0.1;
            let end = if let Some(i) = self.target {
                self.enemies[i].health = (self.enemies[i].health - 20.0).max(0.0);
                self.hits += 1;
                self.enemies[i].position
            } else {
                [
                    self.player[0] + self.yaw.sin() * 20.0,
                    self.player[1] - self.yaw.cos() * 20.0,
                ]
            };
            self.last_shot = Some((self.player, end));
        }
        if self.alive() == 0 {
            self.outcome = Outcome::Won;
            return;
        }
        for enemy in &mut self.enemies {
            if enemy.health <= 0.0 {
                continue;
            }
            let dist = distance(enemy.position, self.player);
            if dist > 8.0 {
                let delta = [
                    (self.player[0] - enemy.position[0]) / dist * dt * 1.4,
                    (self.player[1] - enemy.position[1]) / dist * dt * 1.4,
                ];
                enemy.position = walk(ground, enemy.position, delta, self.center);
            }
            enemy.cooldown -= dt;
            if enemy.cooldown <= 0.0 && dist < 27.0 && ground.visible(enemy.position, self.player) {
                self.health = (self.health - 3.0).max(0.0);
                enemy.cooldown = 1.3;
            }
        }
        if self.health <= 0.0 {
            self.outcome = Outcome::Lost;
        }
    }
}
fn walk(ground: &Ground, start: [f32; 2], delta: [f32; 2], center: [f32; 2]) -> [f32; 2] {
    let mut p = start;
    for axis in 0..2 {
        let mut candidate = p;
        candidate[axis] += delta[axis];
        if distance(candidate, center) > 23.0 {
            continue;
        }
        if let (Some(old), Some(new)) = (
            ground.height(p[0], p[1]),
            ground.height(candidate[0], candidate[1]),
        ) && (new - old).abs() <= delta[axis].abs() * 2.5 + 0.03
        {
            p = candidate;
        }
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    fn flat() -> Ground {
        Ground {
            heights: (-40..=40)
                .flat_map(|x| (-40..=40).map(move |z| ((x, z), 0.0)))
                .collect(),
            cells: (-40..40)
                .flat_map(|x| (-40..40).map(move |z| (x, z)))
                .collect(),
            grid: None,
        }
    }
    #[test]
    fn height_rejects_void() {
        let g = flat();
        assert_eq!(g.height(0.3, 0.2), Some(0.0));
        assert_eq!(g.height(41.0, 0.0), None);
    }
    #[test]
    fn movement_stays_on_ground_and_in_arena() {
        let g = flat();
        let mut s = Game::new([0.0, 0.0]);
        for _ in 0..1000 {
            s.tick(
                &g,
                Input {
                    forward: 1.0,
                    sprint: true,
                    ..Default::default()
                },
                0.016,
            );
        }
        assert!(distance(s.player, s.center) <= 23.0);
        assert!(s.player[1] < 9.0);
    }
    #[test]
    fn firing_wins_and_restart_resets() {
        let g = flat();
        let mut s = Game::new([0.0, 0.0]);
        for _ in 0..1000 {
            s.tick(
                &g,
                Input {
                    fire: true,
                    ..Default::default()
                },
                0.016,
            );
        }
        assert_eq!(s.outcome, Outcome::Won);
        assert_eq!(s.hits, 15);
        s.tick(
            &g,
            Input {
                restart: true,
                ..Default::default()
            },
            0.016,
        );
        assert_eq!(s.alive(), 5);
        assert_eq!(s.health, 100.0);
    }
    #[test]
    fn idle_can_lose() {
        let g = flat();
        let mut s = Game::new([0.0, 0.0]);
        for _ in 0..5000 {
            s.tick(&g, Input::default(), 0.016);
        }
        assert_eq!(s.outcome, Outcome::Lost);
    }
    #[test]
    fn cooldown_and_diagonal_normalization() {
        let g = flat();
        let mut s = Game::new([0.0, 0.0]);
        let start = s.player;
        s.tick(
            &g,
            Input {
                forward: 1.0,
                strafe: 1.0,
                fire: true,
                ..Default::default()
            },
            0.05,
        );
        s.tick(
            &g,
            Input {
                fire: true,
                ..Default::default()
            },
            0.05,
        );
        assert_eq!(s.shots, 1);
        assert!((distance(start, s.player) - 0.225).abs() < 0.0001);
    }
    #[test]
    fn terrain_blocks_line_of_sight() {
        let mut g = flat();
        for x in -3..=3 {
            g.heights.insert((x, 0), 8.0);
        }
        assert!(!g.visible([0.2, -3.0], [0.2, 3.0]));
        assert!(g.visible([8.0, -3.0], [8.0, 3.0]));
    }
    #[test]
    fn target_cycles_and_finished_game_stops() {
        let g = flat();
        let mut s = Game::new([0.0, 0.0]);
        s.tick(&g, Input::default(), 0.016);
        let first = s.target;
        s.tick(
            &g,
            Input {
                cycle: true,
                ..Default::default()
            },
            0.016,
        );
        assert_ne!(s.target, first);
        s.outcome = Outcome::Lost;
        let before = s.player;
        s.tick(
            &g,
            Input {
                forward: 1.0,
                fire: true,
                ..Default::default()
            },
            0.05,
        );
        assert_eq!(s.player, before);
        assert_eq!(s.shots, 0);
    }
    #[test]
    fn interpolation_follows_triangle() {
        let mut g = flat();
        g.heights.insert((1, 1), 4.0);
        assert_eq!(g.height(0.25, 0.25), Some(0.0));
        assert_eq!(g.height(0.75, 0.75), Some(2.0));
    }
}
