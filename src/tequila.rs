//! Native `FEQT` (`cTequilaEffectResource` / `ParticleSystemEd_File_(C)2004_Kuju_Ltd`) decoder.

use crate::{
    Result,
    archive::{chunks, named_payload},
};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub struct NumberCurve {
    pub value: f32,
    pub function: String,
    pub keyframes: Vec<(u32, f32)>,
}

impl Default for NumberCurve {
    fn default() -> Self {
        Self {
            value: 0.0,
            function: "NONE".into(),
            keyframes: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ColourRange {
    pub start_rgba: [f32; 4],
    pub transition_rgba: [f32; 4],
    pub end_rgba: [f32; 4],
    pub use_end: bool,
    pub use_transition: bool,
    pub transition_point: f32,
}

impl Default for ColourRange {
    fn default() -> Self {
        Self {
            start_rgba: [1.0, 1.0, 1.0, 1.0],
            transition_rgba: [1.0, 1.0, 1.0, 1.0],
            end_rgba: [1.0, 1.0, 1.0, 0.0],
            use_end: true,
            use_transition: false,
            transition_point: 0.5,
        }
    }
}

impl ColourRange {
    pub fn sample(&self, t: f32) -> [f32; 4] {
        let u = t.clamp(0.0, 1.0);
        if !self.use_end {
            return self.start_rgba;
        }
        let lerp4 = |a: [f32; 4], b: [f32; 4], f: f32| -> [f32; 4] {
            let f = f.clamp(0.0, 1.0);
            [
                a[0] + (b[0] - a[0]) * f,
                a[1] + (b[1] - a[1]) * f,
                a[2] + (b[2] - a[2]) * f,
                a[3] + (b[3] - a[3]) * f,
            ]
        };
        if self.use_transition && self.transition_point > 0.001 && self.transition_point < 0.999 {
            if u <= self.transition_point {
                lerp4(
                    self.start_rgba,
                    self.transition_rgba,
                    u / self.transition_point,
                )
            } else {
                lerp4(
                    self.transition_rgba,
                    self.end_rgba,
                    (u - self.transition_point) / (1.0 - self.transition_point),
                )
            }
        } else {
            lerp4(self.start_rgba, self.end_rgba, u)
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParticleSpec {
    pub radius: f32,
    pub final_radius: f32,
    pub life_ticks: i32,
    pub gravity_scalar: f32,
    pub blend_mode: u32,
    pub texture: String,
    pub colour_range: String,
    pub velocity_damp: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EmitterSpec {
    pub emit_per_turn: f32,
    pub life_ticks: i32,
    pub particle_descriptor: String,
    pub initial_velocity: [f32; 3],
    pub velocity_randomness: f32,
    pub outward_velocity: f32,
    pub shape: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct SequenceEntry {
    pub descriptor: String,
    pub time_ticks: i32,
    pub transmit_for: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DescriptorKind {
    Particle(ParticleSpec),
    Emitter(EmitterSpec),
    Colour(ColourRange),
    Sequence {
        entries: Vec<SequenceEntry>,
        sorted: bool,
        render_order: i32,
    },
    Other {
        type_id: u32,
        fields: BTreeMap<String, String>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Descriptor {
    pub type_id: u32,
    pub name: String,
    pub curves: BTreeMap<String, NumberCurve>,
    pub fields: BTreeMap<String, String>,
    pub kind: DescriptorKind,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EffectVisualProfile {
    pub primary_texture: String,
    pub start_radius: f32,
    pub end_radius: f32,
    pub duration_seconds: f32,
    pub emit_burst: f32,
    pub start_rgba: [f32; 4],
    pub end_rgba: [f32; 4],
    pub additive: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TequilaEffect {
    pub name: String,
    pub file_version: f32,
    pub descriptors: Vec<Descriptor>,
}

impl TequilaEffect {
    pub fn from_archive(archive: &[u8]) -> Result<Vec<Self>> {
        chunks(archive)?
            .into_iter()
            .filter(|c| &c.tag == b"FEQT")
            .map(|c| Self::parse_named_chunk(c.payload))
            .collect()
    }

    pub fn parse_named_chunk(payload: &[u8]) -> Result<Self> {
        let (name, body) = named_payload(payload)?;
        let text = std::str::from_utf8(body)
            .map(str::to_string)
            .unwrap_or_else(|_| body.iter().map(|&b| b as char).collect());
        Self::parse_text(name, &text)
    }

    pub fn parse_text(name: &str, text: &str) -> Result<Self> {
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .peekable();
        let Some(header) = lines.next() else {
            return Err("Empty FEQT payload".into());
        };
        if !header.contains("ParticleSystemEd_File_") {
            return Err(format!("Invalid FEQT header in {name}: {header}"));
        }
        let mut file_version = 2.0f32;
        let mut expected_count = 0usize;
        while let Some(&line) = lines.peek() {
            if line.starts_with("Particle_Descriptor_Type ") || line.starts_with("****") {
                break;
            }
            let line = lines.next().unwrap();
            if let Some(rest) = line.strip_prefix("File_Version ") {
                file_version = rest
                    .trim()
                    .parse::<f32>()
                    .map_err(|_| "Invalid FEQT File_Version")?;
            } else if let Some(rest) = line.strip_prefix("Num_Particle_Descriptors ") {
                expected_count = rest
                    .trim()
                    .parse::<usize>()
                    .map_err(|_| "Invalid FEQT Num_Particle_Descriptors")?;
            }
        }

        let mut descriptors = Vec::with_capacity(expected_count);
        while lines.peek().is_some() {
            while let Some(&line) = lines.peek() {
                if line.chars().all(|c| c == '*') {
                    lines.next();
                } else {
                    break;
                }
            }
            if lines.peek().is_none() {
                break;
            }
            let type_line = lines.next().ok_or("Missing Particle_Descriptor_Type")?;
            let type_id = type_line
                .strip_prefix("Particle_Descriptor_Type ")
                .ok_or("Expected Particle_Descriptor_Type")?
                .trim()
                .parse::<u32>()
                .map_err(|_| "Invalid Particle_Descriptor_Type")?;
            let name_line = lines.next().ok_or("Missing Particle_Descriptor_Name")?;
            let desc_name = name_line
                .strip_prefix("Particle_Descriptor_Name ")
                .ok_or("Expected Particle_Descriptor_Name")?
                .trim()
                .to_string();

            let mut curves = BTreeMap::new();
            let mut fields = BTreeMap::new();
            let mut seq_entries = Vec::new();
            let mut pending_seq_desc: Option<String> = None;
            let mut pending_seq_time: i32 = 0;

            while let Some(&line) = lines.peek() {
                if line.chars().all(|c| c == '*') || line.starts_with("Particle_Descriptor_Type ") {
                    break;
                }
                let line = lines.next().unwrap();
                if let Some(key) = line.strip_suffix(" NUMBER_VERSION_2") {
                    let key = key.trim().to_string();
                    let line1 = lines.next().ok_or("Missing NUMBER_VERSION_2 line 1")?;
                    let line2 = lines.next().ok_or("Missing NUMBER_VERSION_2 line 2")?;
                    let l1 = line1.strip_prefix("****1:").unwrap_or(line1).trim();
                    let mut parts1 = l1.splitn(2, char::is_whitespace);
                    let val = parts1
                        .next()
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0);
                    let func = parts1.next().unwrap_or("NONE").trim().to_string();
                    let l2 = line2.strip_prefix("****2:").unwrap_or(line2).trim();
                    let nums: Vec<f32> = l2
                        .split_whitespace()
                        .filter_map(|s| s.parse::<f32>().ok())
                        .collect();
                    let mut keyframes = Vec::new();
                    for pair in nums.chunks(2) {
                        if let [flag, v] = pair {
                            keyframes.push((*flag as u32, *v));
                        }
                    }
                    curves.insert(
                        key,
                        NumberCurve {
                            value: val,
                            function: func,
                            keyframes,
                        },
                    );
                } else if let Some((k, v)) = line.split_once(char::is_whitespace) {
                    let k = k.trim();
                    let v = v.trim();
                    if type_id == 6 {
                        match k {
                            "Particle_Descriptor" => {
                                pending_seq_desc = Some(v.to_string());
                            }
                            "Time" => {
                                pending_seq_time = v.parse::<i32>().unwrap_or(0);
                            }
                            "Transmit_FoR" => {
                                let tf = v.parse::<i32>().unwrap_or(0) != 0;
                                if let Some(d) = pending_seq_desc.take() {
                                    seq_entries.push(SequenceEntry {
                                        descriptor: d,
                                        time_ticks: pending_seq_time,
                                        transmit_for: tf,
                                    });
                                }
                            }
                            _ => {
                                fields.insert(k.to_string(), v.to_string());
                            }
                        }
                    } else {
                        fields.insert(k.to_string(), v.to_string());
                    }
                }
            }

            let curve_val =
                |k: &str, default: f32| -> f32 { curves.get(k).map_or(default, |c| c.value) };
            let field_f32 = |k: &str, default: f32| -> f32 {
                fields
                    .get(k)
                    .and_then(|s| s.parse::<f32>().ok())
                    .unwrap_or(default)
            };
            let field_i32 = |k: &str, default: i32| -> i32 {
                fields
                    .get(k)
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(default)
            };

            let kind = match type_id {
                1 => DescriptorKind::Particle(ParticleSpec {
                    radius: curve_val("Radius", 1.0),
                    final_radius: field_f32("Final_Radius", 1.0),
                    life_ticks: field_i32("Life", 30),
                    gravity_scalar: field_f32("GravityScalar", 0.0),
                    blend_mode: field_i32("Blend_Mode", 0).max(0) as u32,
                    texture: fields.get("Texture").cloned().unwrap_or_default(),
                    colour_range: fields.get("Colour_Range").cloned().unwrap_or_default(),
                    velocity_damp: field_f32("Velocity_Damp", 1.0),
                }),
                2 => DescriptorKind::Emitter(EmitterSpec {
                    emit_per_turn: curve_val("Emit_Per_Turn", 1.0),
                    life_ticks: field_i32("Life", 1),
                    particle_descriptor: fields
                        .get("Particle_Descriptor")
                        .cloned()
                        .unwrap_or_default(),
                    initial_velocity: [
                        curve_val("Initial_Velocity_X", 0.0),
                        curve_val("Initial_Velocity_Y", 0.0),
                        curve_val("Initial_Velocity_Z", 0.0),
                    ],
                    velocity_randomness: curve_val("Velocity_Randomness", 0.0),
                    outward_velocity: curve_val("Outward_Velocity", 0.0),
                    shape: fields.get("Shape").cloned().unwrap_or_default(),
                }),
                5 => DescriptorKind::Colour(ColourRange {
                    start_rgba: [
                        curve_val("Start_Red", 1.0),
                        curve_val("Start_Green", 1.0),
                        curve_val("Start_Blue", 1.0),
                        curve_val("Start_Alpha", 1.0),
                    ],
                    transition_rgba: [
                        curve_val("Transition_Red", 1.0),
                        curve_val("Transition_Green", 1.0),
                        curve_val("Transition_Blue", 1.0),
                        curve_val("Transition_Alpha", 1.0),
                    ],
                    end_rgba: [
                        curve_val("End_Red", 1.0),
                        curve_val("End_Green", 1.0),
                        curve_val("End_Blue", 1.0),
                        curve_val("End_Alpha", 0.0),
                    ],
                    use_end: field_i32("Use_End", 1) != 0,
                    use_transition: field_i32("Use_Transition", 0) != 0,
                    transition_point: field_f32("Transition_Point", 0.5),
                }),
                6 => DescriptorKind::Sequence {
                    entries: seq_entries,
                    sorted: field_i32("Sorted", 0) != 0,
                    render_order: field_i32("Render_Order", 0),
                },
                _ => DescriptorKind::Other {
                    type_id,
                    fields: fields.clone(),
                },
            };

            descriptors.push(Descriptor {
                type_id,
                name: desc_name,
                curves,
                fields,
                kind,
            });
        }

        if expected_count > 0 && descriptors.len() != expected_count {
            return Err(format!(
                "FEQT {name} descriptor count mismatch: header={} parsed={}",
                expected_count,
                descriptors.len()
            ));
        }

        Ok(Self {
            name: name.to_string(),
            file_version,
            descriptors,
        })
    }

    pub fn summary_profile(&self) -> EffectVisualProfile {
        let mut colours = BTreeMap::new();
        let mut particles = BTreeMap::new();
        let mut emitters = Vec::new();
        for d in &self.descriptors {
            match &d.kind {
                DescriptorKind::Colour(c) => {
                    colours.insert(d.name.as_str(), c);
                }
                DescriptorKind::Particle(p) => {
                    particles.insert(d.name.as_str(), p);
                }
                DescriptorKind::Emitter(e) => {
                    emitters.push(e);
                }
                _ => {}
            }
        }
        let primary_particle = emitters
            .iter()
            .find_map(|e| particles.get(e.particle_descriptor.as_str()).copied())
            .or_else(|| particles.values().next().copied());
        let total_emit: f32 = emitters
            .iter()
            .map(|e| e.emit_per_turn.max(1.0))
            .sum::<f32>()
            .max(1.0);
        if let Some(p) = primary_particle {
            let col = colours
                .get(p.colour_range.as_str())
                .copied()
                .cloned()
                .unwrap_or_default();
            let tex_stem = p
                .texture
                .strip_suffix(".ace")
                .or_else(|| p.texture.strip_suffix(".ACE"))
                .unwrap_or(&p.texture)
                .to_ascii_lowercase();
            let duration = (p.life_ticks.max(6) as f32 / 30.0).clamp(0.15, 6.0);
            EffectVisualProfile {
                primary_texture: tex_stem,
                start_radius: p.radius.max(0.25),
                end_radius: (p.radius * p.final_radius).max(0.15),
                duration_seconds: duration,
                emit_burst: total_emit,
                start_rgba: col.sample(0.0),
                end_rgba: col.sample(1.0),
                additive: p.blend_mode == 0,
            }
        } else {
            EffectVisualProfile {
                primary_texture: String::new(),
                start_radius: 1.0,
                end_radius: 1.5,
                duration_seconds: 0.65,
                emit_burst: total_emit,
                start_rgba: [1.0, 0.85, 0.45, 1.0],
                end_rgba: [1.0, 0.25, 0.05, 0.0],
                additive: true,
            }
        }
    }

    pub fn spawn_burst(
        &self,
        origin: [f32; 3],
        scale: f32,
        seed: u32,
        max_particles: usize,
    ) -> Vec<LiveTequilaParticle> {
        let mut colours = BTreeMap::new();
        let mut particles = BTreeMap::new();
        let mut emitters = Vec::new();
        for d in &self.descriptors {
            match &d.kind {
                DescriptorKind::Colour(c) => {
                    colours.insert(d.name.as_str(), c);
                }
                DescriptorKind::Particle(p) => {
                    particles.insert(d.name.as_str(), p);
                }
                DescriptorKind::Emitter(e) => {
                    emitters.push(e);
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        let cap = max_particles.clamp(1, 64);
        let mut idx = 0u32;
        for e in &emitters {
            let Some(&p) = particles
                .get(e.particle_descriptor.as_str())
                .or_else(|| particles.values().next())
            else {
                continue;
            };
            let col = colours
                .get(p.colour_range.as_str())
                .copied()
                .cloned()
                .unwrap_or_default();
            let count = (e.emit_per_turn.round() as usize).clamp(2, 8);
            let max_age = (p.life_ticks.max(8) as f32 / 30.0).clamp(0.22, 2.4);
            let start_r = (p.radius * 0.22 * scale).clamp(0.35, 3.8);
            let end_r = (start_r * p.final_radius.clamp(0.25, 2.5)).clamp(0.20, 5.2);
            for i in 0..count {
                if out.len() >= cap {
                    break;
                }
                let h = seed
                    .wrapping_mul(1664525)
                    .wrapping_add(idx.wrapping_mul(1013904223))
                    .wrapping_add(i as u32 * 374761393);
                idx = idx.wrapping_add(1);
                let angle = ((h & 0xFFFF) as f32 / 65535.0) * std::f32::consts::TAU;
                let elev = (((h >> 16) & 0xFF) as f32 / 255.0) * 0.65 + 0.35;
                let outward = (e.outward_velocity.abs() + e.velocity_randomness.abs())
                    .clamp(3.5, 16.0)
                    * scale;
                let vx = e.initial_velocity[0] * 0.25 + angle.cos() * outward * (1.0 - elev * 0.35);
                let vy =
                    (e.initial_velocity[1].abs() * 0.35 + outward * elev + 3.0).clamp(2.5, 18.0);
                let vz = e.initial_velocity[2] * 0.25 + angle.sin() * outward * (1.0 - elev * 0.35);
                out.push(LiveTequilaParticle {
                    pos: [
                        origin[0] + angle.cos() * 0.35 * scale,
                        origin[1] + 0.45 * scale,
                        origin[2] + angle.sin() * 0.35 * scale,
                    ],
                    vel: [vx, vy, vz],
                    age: 0.0,
                    max_age,
                    start_radius: start_r,
                    end_radius: end_r,
                    gravity_scalar: p.gravity_scalar.clamp(-2.0, 2.5),
                    velocity_damp: p.velocity_damp.clamp(0.75, 1.0),
                    colour_range: col.clone(),
                    spin: angle,
                });
            }
        }
        out
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct LiveTequilaParticle {
    pub pos: [f32; 3],
    pub vel: [f32; 3],
    pub age: f32,
    pub max_age: f32,
    pub start_radius: f32,
    pub end_radius: f32,
    pub gravity_scalar: f32,
    pub velocity_damp: f32,
    pub colour_range: ColourRange,
    pub spin: f32,
}

impl LiveTequilaParticle {
    pub fn step(&mut self, dt: f32) -> bool {
        let dt = dt.clamp(0.0, 0.1);
        self.age += dt;
        if self.age >= self.max_age {
            return false;
        }
        self.vel[1] -= 9.81 * self.gravity_scalar * dt;
        let damp = self.velocity_damp.clamp(0.5, 1.0).powf(dt * 30.0);
        for (p, v) in self.pos.iter_mut().zip(self.vel.iter_mut()) {
            *v *= damp;
            *p += *v * dt;
        }
        self.spin += dt * 1.8;
        true
    }

    pub fn normalized_age(&self) -> f32 {
        (self.age / self.max_age.max(0.05)).clamp(0.0, 1.0)
    }

    pub fn current_radius(&self) -> f32 {
        let t = self.normalized_age();
        self.start_radius + (self.end_radius - self.start_radius) * t
    }

    pub fn current_rgba(&self) -> [f32; 4] {
        self.colour_range.sample(self.normalized_age())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_feqt_descriptor_and_samples_colour_transition() {
        let sample = concat!(
            "ParticleSystemEd_File_(C)2004_Kuju_Ltd\r\n",
            "File_Version 2.000000\r\n",
            "Num_Particle_Descriptors 3\r\n",
            "Particle_Descriptor_Type 5\r\n",
            "Particle_Descriptor_Name 1099_(colour) Gold\r\n",
            "Start_Red NUMBER_VERSION_2\r\n",
            "****1: 1.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Start_Green NUMBER_VERSION_2\r\n",
            "****1: 0.800000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Start_Blue NUMBER_VERSION_2\r\n",
            "****1: 0.200000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Start_Alpha NUMBER_VERSION_2\r\n",
            "****1: 1.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "End_Red NUMBER_VERSION_2\r\n",
            "****1: 1.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "End_Green NUMBER_VERSION_2\r\n",
            "****1: 0.200000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "End_Blue NUMBER_VERSION_2\r\n",
            "****1: 0.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "End_Alpha NUMBER_VERSION_2\r\n",
            "****1: 0.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Use_End 1\r\n",
            "Use_Transition 0\r\n",
            "Transition_Point 0.200000\r\n",
            "*****************************************************************\r\n",
            "Particle_Descriptor_Type 1\r\n",
            "Particle_Descriptor_Name 1099_(particle) blob\r\n",
            "Radius NUMBER_VERSION_2\r\n",
            "****1: 6.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Final_Radius 0.500000\r\n",
            "Life 60\r\n",
            "GravityScalar 0.100000\r\n",
            "Blend_Mode 0\r\n",
            "Texture SFX_Glow.ace\r\n",
            "Colour_Range 1099_(colour) Gold\r\n",
            "Velocity_Damp 1.000000\r\n",
            "*****************************************************************\r\n",
            "Particle_Descriptor_Type 2\r\n",
            "Particle_Descriptor_Name 1099_(emitter) blobs explode\r\n",
            "Emit_Per_Turn NUMBER_VERSION_2\r\n",
            "****1: 12.000000 NONE\r\n",
            "****2: 0 100.000000 0 1.000000 0 0.000000\r\n",
            "Life -2\r\n",
            "Particle_Descriptor 1099_(particle) blob\r\n",
            "Shape NONE\r\n",
            "*****************************************************************\r\n",
        );
        let effect = TequilaEffect::parse_text("1099_FrontendSparks", sample).unwrap();
        assert_eq!(effect.descriptors.len(), 3);
        let profile = effect.summary_profile();
        assert_eq!(profile.primary_texture, "sfx_glow");
        assert_eq!(profile.start_radius, 6.0);
        assert_eq!(profile.end_radius, 3.0);
        assert_eq!(profile.duration_seconds, 2.0);
        assert_eq!(profile.emit_burst, 12.0);
        assert!(profile.additive);

        let mut burst = effect.spawn_burst([10.0, 2.0, -5.0], 1.2, 42, 16);
        assert_eq!(burst.len(), 8);
        let r0 = burst[0].current_radius();
        let c0 = burst[0].current_rgba();
        assert!(burst[0].step(0.5));
        assert!(burst[0].current_radius() < r0);
        assert!(burst[0].current_rgba()[3] < c0[3]);
    }

    #[test]
    fn decodes_all_original_feqt_chunks_in_frontend_and_campaign_archives() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let fe_res = root.join("local/extracted/files/Data/CompoundFiles/Frontend2_Level.res");
        if !fe_res.exists() {
            return;
        }
        let fe_bytes = std::fs::read(&fe_res).unwrap();
        let fe_effects = TequilaEffect::from_archive(&fe_bytes).unwrap();
        assert_eq!(fe_effects.len(), 5);
        for eff in &fe_effects {
            let p = eff.summary_profile();
            assert!(!p.primary_texture.is_empty());
            assert!(p.start_radius > 0.0);
        }
    }
}
