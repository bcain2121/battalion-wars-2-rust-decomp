//! Explicit Rust-owned scenery/effect runtime, NOT native engine parity.
//! Cosmetic light chains run at 60 fixed steps/second. Geometry is retired by World.
//! Mission-owned blast damage is opt-in; classified presentation gaps may be warnings.
//! Unknown gameplay channels and malformed data remain errors.
use crate::mission_effect_chain::{Event, Tail};
use crate::mission_effect_light::{Light, Spec};
use crate::mission_random::IntegerStream;
use crate::{Object, ObjectGraph, ObjectId, Result};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

#[derive(Clone, Debug)]
pub struct Effect {
    pub owner: ObjectId,
    pub position: [f32; 3],
    pub light: Light,
    tail: Tail,
}
#[derive(Clone)]
pub struct Runtime {
    objects: Arc<BTreeMap<ObjectId, Object>>,
    pub retired: BTreeSet<ObjectId>,
    effects: Vec<Effect>,
    lightning: Vec<crate::mission_lightning::Lightning>,
    random: IntegerStream,
    accumulator: f64,
    allow_cosmetic_gaps: bool,
    damage_enabled: bool,
    pub activity_score: f64,
    blasts: Vec<crate::mission_damage::Blast>,
    warnings: BTreeSet<String>,
    pub requested_view_distance: Option<(f64, f64)>,
    pub requested_water_render: Option<bool>,
    pub command_bar_revision: u64,
    pub rejected_commands: BTreeSet<String>,
    pub placement_corrections: BTreeMap<ObjectId, ([f32; 3], [f32; 3])>,
}
fn scalar<'a>(o: &'a Object, name: &str) -> Result<&'a str> {
    let fields: Vec<_> = o
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|n| n == name))
        .collect();
    let [field] = fields.as_slice() else {
        return Err(format!("Rewrite missing/ambiguous {}.{name}", o.id.0));
    };
    let [value] = field.items.as_slice() else {
        return Err(format!("Rewrite invalid scalar {}.{name}", o.id.0));
    };
    Ok(value)
}
fn reference(o: &Object, name: &str) -> Result<Option<ObjectId>> {
    let id = scalar(o, name)?
        .parse::<u64>()
        .map_err(|_| format!("Invalid {name}"))?;
    Ok((id != 0).then_some(ObjectId(id)))
}
impl Runtime {
    /// A new Rust session owns its effect RNG; this is not a captured Wii RNG cursor.
    pub fn new(graph: &ObjectGraph) -> Self {
        Self {
            objects: Arc::new(graph.objects.clone()),
            retired: BTreeSet::new(),
            effects: Vec::new(),
            lightning: Vec::new(),
            random: IntegerStream::at_cursor(0).expect("valid cursor"),
            accumulator: 0.0,
            allow_cosmetic_gaps: false,
            damage_enabled: false,
            activity_score: 0.0,
            blasts: Vec::new(),
            warnings: BTreeSet::new(),
            requested_view_distance: None,
            requested_water_render: None,
            command_bar_revision: 0,
            rejected_commands: BTreeSet::new(),
            placement_corrections: BTreeMap::new(),
        }
    }
    /// Opt-in playable policy. Never suppresses gameplay errors or malformed data.
    pub fn allow_cosmetic_gaps(&mut self) {
        self.allow_cosmetic_gaps = true;
    }
    pub(crate) fn allow_faded_transfer_presentation(&mut self) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Transfer presentation backend not configured".into());
        }
        self.warnings.insert("Cosmetic omission: frozen fully-faded default transfer retargets the camera without visible interpolation; native HUD0 transfer sound is not integrated".into());
        Ok(())
    }
    pub(crate) fn allow_missile_presentation(&mut self) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Missile presentation backend not configured".into());
        }
        self.warnings.insert("Cosmetic omission: guided missile procedural proxies; muzzle/impact effects, smoke trails, firing animation and hardware sound are not integrated".into());
        Ok(())
    }
    pub(crate) fn allow_bomber_presentation(&mut self) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Bomber projectile presentation backend not configured".into());
        }
        self.warnings.insert("Cosmetic omission: bomber projectiles use procedural proxies; dedicated muzzle/impact presentation and hardware sound are not integrated".into());
        Ok(())
    }
    pub fn request_view_distance(&mut self, adjustment: f64, frames: f64) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("View-distance presentation backend not configured".into());
        }
        if !adjustment.is_finite() || !frames.is_finite() || frames < 0.0 || frames.fract() != 0.0 {
            return Err("Invalid view-distance request".into());
        }
        self.requested_view_distance = Some((adjustment, frames));
        self.warnings.insert(
            "Cosmetic omission: scripted view-distance adjustment is recorded but not applied"
                .into(),
        );
        Ok(())
    }
    pub fn reset_command_bar(&mut self) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Command bar presentation backend not configured".into());
        }
        self.command_bar_revision = self
            .command_bar_revision
            .checked_add(1)
            .ok_or("Command bar revision overflow")?;
        self.warnings.insert(
            "Cosmetic omission: command selection reset; original command-bar graphics pending"
                .into(),
        );
        Ok(())
    }
    pub fn request_water_render(&mut self, enabled: bool) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Water presentation backend not configured".into());
        }
        self.requested_water_render = Some(enabled);
        self.warnings.insert(
            "Cosmetic omission: scripted water visibility is recorded but not applied".into(),
        );
        Ok(())
    }
    pub fn warnings(&self) -> &BTreeSet<String> {
        &self.warnings
    }
    pub(crate) fn enable_damage(&mut self) {
        self.damage_enabled = true;
    }
    pub(crate) fn take_blasts(&mut self) -> Vec<crate::mission_damage::Blast> {
        std::mem::take(&mut self.blasts)
    }
    pub(crate) fn objects(&self) -> Arc<BTreeMap<ObjectId, Object>> {
        self.objects.clone()
    }
    fn resolve_object_base(&self, object: &Object) -> Result<&Object> {
        let key = if object.kind == "cMorphingBuilding" {
            match crate::original_scene::field(object, "mAllegiance") {
                Some("eSolarEmpire") => "mBaseSE",
                Some("eAngloIsles") => "mBaseAG",
                Some("eWesternFrontier") => "mBaseWF",
                Some("eTundranTerritories") | Some("eTundran") => "mBaseTT",
                Some("eXylvania") | Some("eXylvanian") => "mBaseXY",
                Some("eUnderWorld") | Some("eUnderworld") => "mBaseUW",
                _ => "mBaseNeutral",
            }
        } else {
            "mBase"
        };
        let mut base_id = reference(object, key)?;
        if base_id.is_none() && object.kind == "cMorphingBuilding" {
            for fallback in [
                "mBaseNeutral",
                "mBaseSE",
                "mBaseAG",
                "mBaseWF",
                "mBaseTT",
                "mBaseXY",
                "mBaseUW",
            ] {
                if let Ok(Some(id)) = reference(object, fallback) {
                    base_id = Some(id);
                    break;
                }
            }
        }
        self.object(base_id.ok_or("Missing damage/destruction base")?)
    }
    pub(crate) fn damage_for(
        &self,
        blast: &crate::mission_damage::Blast,
        id: ObjectId,
        from_player: bool,
    ) -> Result<f64> {
        let object = self.object(id)?;
        let base = self.resolve_object_base(object)?;
        if let Some(flag) = crate::original_scene::field(base, "mPlayerDamageOnly") {
            match flag {
                "eTrue" if !from_player => return Ok(0.0),
                "eTrue" | "eFalse" => {}
                _ => return Err("Invalid player-only damage flag".into()),
            }
        }
        if !from_player
            && base.kind == "sTroopBase"
            && blast
                .channels
                .iter()
                .any(|(k, v)| k == "DAMAGE_IMPACT" && *v > 0.0)
            && blast
                .channels
                .iter()
                .any(|(k, v)| k == "DAMAGE_EXPLOSIVE" && *v > 0.0)
        {
            let mut filtered = blast.clone();
            filtered.channels = blast
                .channels
                .iter()
                .filter_map(|(k, v)| {
                    if k == "DAMAGE_IMPACT" || k == "DAMAGE_CUSTOM_1" {
                        None
                    } else if k == "DAMAGE_EXPLOSIVE" {
                        Some((k.clone(), *v * 0.5))
                    } else {
                        Some((k.clone(), *v))
                    }
                })
                .collect();
            return filtered.damage_to(base, object.kind == "cDestroyableObject");
        }
        blast.damage_to(base, object.kind == "cDestroyableObject")
    }
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }
    pub fn lightning(&self) -> &[crate::mission_lightning::Lightning] {
        &self.lightning
    }
    /// Five-argument PlayEffect route; named camera-anchored Rust LightningX visual.
    pub fn play_effect(&mut self, id: ObjectId, flags: u32, offset: [f32; 3]) -> Result<()> {
        if !(matches!(flags, 1 | 3) || (self.allow_cosmetic_gaps && flags == 2)) {
            return Err("Unsupported rewrite PlayEffect channel flags".into());
        }
        let object = self.object(id)?;
        if object.kind != "cScriptedEffectBase" {
            return Err("PlayEffect requires cScriptedEffectBase".into());
        }
        let has_sound = flags & 2 != 0 && reference(object, "mSoundEffect")?.is_some();
        if has_sound && !self.allow_cosmetic_gaps {
            return Err("Scripted effect sound backend not configured".into());
        }
        let anchor = scalar(object, "meScriptedEffectAnchorType")?.to_string();
        let timeout = scalar(object, "mTequilaTimeOut")?
            .parse::<f64>()
            .map_err(|_| "Invalid effect timeout")?;
        let Some(visual_id) = reference(object, "mTequilaEffect")? else {
            if self.allow_cosmetic_gaps && timeout.is_finite() && timeout >= 0.0 {
                if has_sound {
                    self.warnings.insert(format!(
                        "Cosmetic omission: scripted effect {} sound channel",
                        id.0
                    ));
                }
                return Ok(());
            }
            return Err("Missing effect visual".into());
        };
        let visual = self.object(visual_id)?;
        if visual.kind != "cTequilaEffectResource" {
            return Err("Unmapped Rust scripted visual".into());
        }
        let visual_name = scalar(visual, "mName")?.to_string();
        if has_sound {
            self.warnings.insert(format!(
                "Cosmetic omission: scripted effect {} sound channel",
                id.0
            ));
        }
        if anchor == "eEFFECT_ANCHOR_TO_CAMERA_POS" && visual_name == "001_Null" {
            self.lightning.clear();
            return Ok(());
        }
        if anchor != "eEFFECT_ANCHOR_TO_CAMERA_POS_DIR" || visual_name != "1010_X Lightning" {
            if self.allow_cosmetic_gaps && timeout.is_finite() && timeout >= 0.0 {
                self.warnings.insert(format!(
                    "Cosmetic omission: scripted effect {} anchor {} visual {}",
                    id.0, anchor, visual_name
                ));
                return Ok(());
            }
            if anchor != "eEFFECT_ANCHOR_TO_CAMERA_POS_DIR" {
                return Err("Unsupported scripted effect anchor".into());
            }
            return Err("Unmapped Rust scripted visual".into());
        }
        if self.lightning.len() >= 64 {
            return Err("Scripted effect capacity exceeded".into());
        }
        let effect = crate::mission_lightning::Lightning::new(id, offset, timeout)?;
        self.lightning.push(effect);
        Ok(())
    }
    pub(crate) fn omit_scripted_visual(&mut self, id: ObjectId) -> Result<()> {
        if !self.allow_cosmetic_gaps {
            return Err("Unit-anchored scripted visual backend not configured".into());
        }
        let object = self.object(id)?;
        if object.kind != "cScriptedEffectBase" {
            return Err("Invalid scripted visual resource".into());
        }
        let visual = reference(object, "mTequilaEffect")?;
        let sound = reference(object, "mSoundEffect")?;
        if let Some(visual) = visual
            && self.object(visual)?.kind != "cTequilaEffectResource"
        {
            return Err("Invalid scripted visual reference".into());
        }
        if let Some(sound) = sound
            && self.object(sound)?.kind != "cSoundBase"
        {
            return Err("Invalid scripted sound reference".into());
        }
        if visual.is_none() && sound.is_none() {
            return Err("Empty scripted effect".into());
        }
        let timeout = scalar(object, "mTequilaTimeOut")?
            .parse::<f64>()
            .map_err(|_| "Invalid scripted visual timeout")?;
        if !timeout.is_finite() || timeout < 0. {
            return Err("Invalid scripted visual timeout".into());
        }
        self.warnings.insert(format!(
            "Cosmetic omission: unit-targeted scripted effect {} visual {:?} sound {:?}",
            id.0, visual, sound
        ));
        Ok(())
    }
    fn object(&self, id: ObjectId) -> Result<&Object> {
        self.objects
            .get(&id)
            .ok_or_else(|| format!("Rewrite missing object {}", id.0))
    }
    fn delay(&self, id: ObjectId) -> Result<f32> {
        let delay = scalar(self.object(id)?, "mNextExplosionDelay")?
            .parse::<f32>()
            .map_err(|_| "Invalid chain delay")?;
        if !delay.is_finite() || delay <= 0.0 {
            return Err("Rewrite requires positive child delay".into());
        }
        Ok(delay)
    }
    pub(crate) fn spawn(
        &mut self,
        owner: ObjectId,
        position: [f32; 3],
        id: ObjectId,
    ) -> Result<()> {
        if self.effects.len() >= 4096 {
            return Err("Rewrite effect capacity exceeded".into());
        }
        let object = self.object(id)?;
        let mut warnings = Vec::new();
        let mut blast = None;
        for name in [
            "mBoomSoundBase",
            "mTequilaEffect",
            "mScorchTexturePtr",
            "mGroundGlowTexturePtr",
        ] {
            if let Some(resource) = reference(object, name)? {
                self.object(resource)?;
                if !self.allow_cosmetic_gaps {
                    return Err(format!("Rewrite effect {name} backend not configured"));
                }
                warnings.push(format!(
                    "Cosmetic omission: effect {} channel {name} resource {}",
                    id.0, resource.0
                ));
            }
        }
        for name in [
            "mDamageAmount",
            "mNumFragmentTrails",
            "mNumRubbleBits",
            "mNumExtraExplosions",
            "mTaggedEffectBase",
        ] {
            let fields: Vec<_> = object
                .fields
                .iter()
                .filter(|f| f.attributes.get("name").is_some_and(|n| n == name))
                .collect();
            let [f] = fields.as_slice() else {
                return Err(format!("Missing/ambiguous effect {name}"));
            };
            if f.items.is_empty() {
                return Err(format!("Invalid empty effect {name}"));
            }
            let values = f
                .items
                .iter()
                .map(|v| {
                    v.parse::<i64>()
                        .map_err(|_| format!("Invalid effect {name}"))
                })
                .collect::<Result<Vec<_>>>()?;
            if values.iter().any(|v| *v != 0) {
                // Trail-only incidental fragments may be omitted, but burst explosions may not.
                // Damage requires the world consumer; rubble/extra/tagged effects are not bypassed.
                if self.damage_enabled && name == "mDamageAmount" {
                    blast = Some(crate::mission_damage::Blast::from_explosion(
                        object, owner, position,
                    )?);
                } else if self.allow_cosmetic_gaps
                    && name == "mNumFragmentTrails"
                    && values.len() == 1
                    && values[0] > 0
                    && match reference(object, "mFragmentType")? {
                        None => true,
                        Some(id) => {
                            let fragment = self.object(id)?;
                            fragment.kind == "cIncidentalBase"
                                && reference(fragment, "BurstExplodeType")?.is_none()
                        }
                    }
                {
                    warnings.push(format!(
                        "Cosmetic omission: effect {} fragment trails (no burst explosion)",
                        id.0
                    ));
                } else {
                    return Err(format!("Rewrite effect {name} backend not configured"));
                }
            }
        }
        let light = Light::new(Spec::from_explosion(object)?);
        let first = reference(object, "mNextExplosionType1")?;
        let second = reference(object, "mNextExplosionType2")?;
        let probability = scalar(object, "mNextExplosion1Probability")?
            .parse::<i8>()
            .map_err(|_| "Invalid chain probability")?;
        let mut tail = Tail::new(0.0, first, second, probability)?;
        let mut random = self.random.clone();
        let mut immediate_child = None;
        tail.initialize_delay(&mut random, |child| match self.delay(child) {
            Ok(d) => Ok(d),
            Err(e) => {
                if self.allow_cosmetic_gaps
                    && scalar(self.object(child)?, "mNextExplosionDelay")
                        .ok()
                        .and_then(|s| s.parse::<f32>().ok())
                        == Some(0.0)
                {
                    immediate_child = Some(child);
                    Ok(f32::MIN_POSITIVE)
                } else {
                    Err(e)
                }
            }
        })?;
        if immediate_child.is_some() {
            tail = Tail::new(-1.0, first, second, probability)?;
        }
        self.random = random;
        self.warnings.extend(warnings);
        if let Some(blast) = blast {
            self.blasts.push(blast);
        }
        self.effects.push(Effect {
            owner,
            position,
            light,
            tail,
        });
        if let Some(child) = immediate_child {
            self.spawn(owner, position, child)?;
        }
        Ok(())
    }
    /// Retirement plus effect construction commit together. World encloses geometry/state.
    pub fn retire(&mut self, id: ObjectId, position: [f32; 3]) -> Result<()> {
        if self.retired.contains(&id) {
            return Ok(());
        }
        if position.iter().any(|v| !v.is_finite()) {
            return Err("Invalid rewrite effect position".into());
        }
        let object = self.object(id)?;
        let (base_kind, dead_model, explosion_field) = match object.kind.as_str() {
            "cDestroyableObject" => ("sDestroyBase", Some("DeadModel"), "ExplodeType"),
            "cBuilding" | "cMorphingBuilding" => {
                ("cBuildingImpBase", Some("mpDeadModel"), "mExplodeType")
            }
            "cTroop" => ("sTroopBase", None, "mExplodeType"),
            "cGroundVehicle" => ("cGroundVehicleBase", None, "mExplodeType"),
            "cAirVehicle" => ("cAirVehicleBase", None, "mExplodeType"),
            "cWaterVehicle" => ("sWaterVehicleBase", None, "mExplodeType"),
            _ => return Err("Unsupported rewrite Kill entity type".into()),
        };
        let base = self.resolve_object_base(object)?;
        if base.kind != base_kind
            && !matches!(
                (object.kind.as_str(), base.kind.as_str()),
                ("cGroundVehicle", "sGroundVehicleBase")
                    | ("cAirVehicle", "sAirVehicleBase")
                    | ("cWaterVehicle", "cWaterVehicleBase")
            )
        {
            return Err("Invalid rewrite destruction base type".into());
        }
        let dead = dead_model
            .map(|key| reference(base, key))
            .transpose()?
            .flatten();
        if let Some(model) = dead {
            self.object(model)?;
            if !self.allow_cosmetic_gaps {
                return Err("Rewrite dead-model renderer not configured".into());
            }
        }
        let explosion =
            reference(base, explosion_field).or_else(|_| reference(base, "explodeType"))?;
        let mut staged = self.clone();
        if let Some(model) = dead {
            staged.warnings.insert(format!(
                "Cosmetic omission: dead model {} for entity {}",
                model.0, id.0
            ));
        }
        if let Some(explosion) = explosion {
            staged.spawn(id, position, explosion)?;
        }
        staged.retired.insert(id);
        *self = staged;
        Ok(())
    }
    pub fn advance(&mut self, dt: f64) -> Result<()> {
        if !dt.is_finite() || !(0.0..=1.0).contains(&dt) {
            return Err("Invalid rewrite effect delta".into());
        }
        let mut staged = self.clone();
        staged.accumulator += dt;
        staged.activity_score *= (-dt / 3.0).exp();
        let step = 1.0 / 60.0;
        while staged.accumulator + 1e-12 >= step {
            staged.accumulator = (staged.accumulator - step).max(0.0);
            for lightning in &mut staged.lightning {
                lightning.advance_tick()?;
            }
            staged.lightning.retain(|effect| !effect.finished());
            let mut children = Vec::new();
            for effect in &mut staged.effects {
                effect.light.native_update();
                if let Event::Selected(Some(child)) =
                    effect.tail.advance(step as f32, &mut staged.random)?
                {
                    children.push((effect.owner, effect.position, child));
                }
            }
            staged
                .effects
                .retain(|e| e.light.snapshot().active || e.tail.remaining() > 0.0);
            for (owner, position, child) in children {
                staged.spawn(owner, position, child)?;
            }
        }
        *self = staged;
        Ok(())
    }
}
