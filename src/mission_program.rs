//! Authored script bindings and transactional level initialization.
//! Binding discovery is data recovery; resume ordering/activation remain separate policies.
use crate::{
    ObjectGraph, ObjectId, Result,
    archive::{chunks, named_payload},
    lua50::{self, Bootstrap, Prototype, Value},
    mission_vm::{Host, Status, Thread},
    mission_world::World,
    original_scene::field,
};
use serde::Serialize;
use std::collections::BTreeSet;
/// Interface-name candidates from docs/evidence/first-mission-native-bindings.json.
/// Declaring them does not grant an implementation: unsupported operations still fail closed.
const ENGINE_NAMES: &[&str] = &[
    "AttackTarget",
    "AllPlayerUnitsDead",
    "ClearMessageQueue",
    "Despawn",
    "EnableControllerItem",
    "EnableWeapon",
    "EndFrame",
    "FollowUnit",
    "FollowWaypoint",
    "FreezePlayer",
    "GetActivity",
    "GetHealthPercent",
    "GetFriendlyHealthTotals",
    "GetEnemyHealthTotals",
    "GetNumItemsInMessageQueue",
    "GetObjectXPosition",
    "GetObjectZPosition",
    "GetPlayerUnit",
    "GetRandom",
    "GetTime",
    "GoToArea",
    "IsDead",
    "IsInArea",
    "IsInRange",
    "IsUnderPlayerCommand",
    "Kill",
    "Measure",
    "ListScoringUnits",
    "OutOfBoundsMessageInfo",
    "PhoneMessage",
    "PhoneMessageWithObjective",
    "StoreSummaryMessage",
    "PlayEffect",
    "PlayerAngleToTarget",
    "ReadControllerState",
    "RegisterReflectionId",
    "SetActive",
    "SetHealthPercent",
    "SetMissionComplete",
    "SetMissionRatings",
    "SetMissionMinMaxTime",
    "SetMissionTime",
    "SetMissionPowerObjectives",
    "SetMissionTechObjectives",
    "SetArmyAllegianceForScoring",
    "StartMusic",
    "StopMusic",
    "SetMusicVolume",
    "SetObjectiveData",
    "Spawn",
    "StopAndGuard",
    "Teleport",
    "UnfreezePlayer",
    "WaitFor",
    "WaitForNoPause",
    "SetCamera",
    "CameraFade",
    "CameraSetTarget",
    "CameraSetFOV",
    "CameraSetWaypoint",
    "SetHudState",
    "SetTutorialItemSolid",
    "ActivateTutorialWarning",
    "ResetCommandBar",
    "DeclareVictory",
    "DestroyablePlayAnim",
    "ForceUnitTransfer",
    "SetUnitInvulnerable",
    "SetFunctionIndicatorState",
    "SetFunctionIndicatorMode",
    "SetFunctionIndicatorAction",
    "SetFunctionIndicatorPosition",
    "SetupPauseTutorialStack",
    "SetViewDistanceAdjustment",
    "SetWaterRenderEnable",
    "ToggleBlankPauseScreen",
    "GetCurrentMissionAttempted",
    "SetCurrentMissionCompleted",
    "GetCurrentMissionCompleted",
    "rint",
    "IsNetworkHost",
    "IsNetworkPlayer",
    "DebugOut",
    "GetTutorialMode",
    "AddToGroup",
    "BeachWaterUnit",
    "CameraShake",
    "CheckCapturePoint",
    "CheckSkipCutscene",
    "CombineFlags",
    "CountRemainingPlayerTroops",
    "EnterVehicle",
    "ExitVehicle",
    "GetActionState",
    "GetGroupHealthPercent",
    "GetGroupMember",
    "GetInstallationOwner",
    "GetMovementState",
    "GetNumMatchingInGroup",
    "GetNumSeatsFree",
    "GetPlayerTransferred",
    "GetPrimaryActionState",
    "GetSpawnMode",
    "GetState",
    "GetTimeLeft",
    "IsActive",
    "IsAttacking",
    "IsPassengerInUnit",
    "IsReadyToRevive",
    "LandAirUnit",
    "NumItemsInArea",
    "NumPassengersInUnit",
    "PrepareSkipCutscene",
    "PutUnitInVehicle",
    "ResetHealthAndScoreTotals",
    "ResetPathBlocking",
    "ReviveDeadUnit",
    "ReviveFullUnitCarrier",
    "SetCapturePointHeight",
    "SetDamageZoneDamage",
    "SetEnableGuardMode",
    "SetInstallationOwner",
    "SetInvulnerable",
    "SetRenderHUDComponent",
    "SetTickScriptWhenDead",
    "SetTickScriptWhenDespawned",
    "SetTimer",
    "SetUnitAI",
    "SetUnitShadowDistance",
    "ShowTimer",
    "StopCapturePointSounds",
    "Vanish",
    "VictoryDance",
    "CleanupSkipCutscene",
    "CoopSetup",
    "CreateGroup",
    "DefeatLament",
    "FindDeadUnit",
    "GetAttackTarget",
    "GetGroupHealth",
    "GetMissionEnded",
    "GetMissionWon",
    "GetMissionLost",
    "GetRange",
    "GetScoreTotals",
    "GetFriendlyScoreTotals",
    "GetEnemyScoreTotals",
    "GetHealthTotals",
    "AddStoredHealthAndScoreTotals",
    "GetSubmarineAirpercentage",
    "GetTargetedObject",
    "GetUnitInSeat",
    "IsSubmarineSubmerged",
    "NetworkGameOver",
    "RenderOptionalHUDComponent",
    "RepositionSkirmishScoreBoxes",
    "RollAirUnit",
    "SetCameraEffectFadeIn",
    "SetCameraEffectFadeOut",
    "SetCameraEffectIntensity",
    "SetCharacterAnimUpdateLOD",
    "SetDistortionEffect",
    "SetHUDMultiplePlayerArmies",
    "SetNetworkMissionComplete",
    "SetNogoZone",
    "SetObjectivesResult",
    "GetObjectivesResult",
    "SetPathReblocking",
    "SetReflectionRenderObjects",
    "SetRenderParamTintBlendFactor",
    "SpawnUnitInCarrier",
    "SpawnUnit",
    "StoreHealthAndScoreTotals",
    "SubmarineDive",
    "SubmarineSurface",
    "TestFlags",
    "WillReviveInGuard",
    "mod",
    "RemoveFromGroup",
    "ExtractFromGroup",
    "CopyGroup",
    "GetContainerOf",
    "GetPrimaryActionTarget",
    "GetKiller",
    "IsInRectangle",
    "IsOnFire",
    "TurnUnitLightsOn",
    "PlaySound",
    "DestroyableGetCurrentAnim",
    "SetUnitAllegiance",
    "SetIdle",
    "ActivateArmyEffect",
    "DectivateArmyEffect",
    "SetTutorialBButtonState",
    "SetFogColour",
    "SetSunAmbientColour",
    "SetSunDirectionalColour",
    "SetAntiSunDirectionalColour",
    "SetSunDirection",
    "SetAntiSunDirection",
    "Brace",
    "Cower",
    "SetSkirmishScoreLocked",
    "GetDistanceBetweenPlayers",
];
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Scope {
    Initializer,
    Global,
    Unit,
}
#[derive(Debug, Clone, Serialize)]
pub struct Binding {
    pub entity: ObjectId,
    pub kind: String,
    pub script: String,
    pub scope: Scope,
    pub update_when_paused: bool,
}
#[derive(Debug, Serialize)]
pub struct Probe {
    pub entity: ObjectId,
    pub script: String,
    pub outcome: String,
    pub instructions: u64,
    pub unresolved_global_reads: Vec<String>,
}
pub struct Program {
    pub vm: Bootstrap,
    pub host: Host,
    pub bindings: Vec<Binding>,
    pub initialized: bool,
}
impl Program {
    pub fn load(graph: &ObjectGraph, archive: &[u8]) -> Result<Self> {
        let mut vm = Bootstrap::new();
        let mut names = BTreeSet::new();
        for chunk in chunks(archive)? {
            if chunk.tag == *b"PRCS" {
                let (name, bytes) = named_payload(chunk.payload)?;
                if !names.insert(name.to_owned()) {
                    return Err(format!("Duplicate script resource {name}"));
                }
                vm.execute(name, &lua50::parse(bytes)?, graph, 100000)?;
            }
        }
        let host = Host::from_graph(graph)?;
        host.install(&mut vm);
        World::install(&mut vm)?;
        // Names are declarations, not implementations. Host::call rejects unsupported calls.
        // Keep captured engine functions callable so adding a real backend cannot leave stale nil captures.
        for &name in ENGINE_NAMES {
            vm.globals
                .entry(name.into())
                .or_insert_with(|| Value::Native(name.into()));
        }
        for name in [
            "CurrentPlayer1WaterSpawnX",
            "CurrentPlayer1WaterSpawnZ",
            "CurrentPlayer2WaterSpawnX",
            "CurrentPlayer2WaterSpawnZ",
        ] {
            vm.globals.entry(name.into()).or_insert(Value::Number(0.0));
        }
        let mut bindings = Vec::new();
        for object in graph.objects.values() {
            let Some(raw) = field(object, "mpScript") else {
                continue;
            };
            let id = ObjectId(raw.parse::<u64>().map_err(|_| "Invalid script reference")?);
            if id.0 == 0 {
                continue;
            }
            let resource = graph.objects.get(&id).ok_or("Unresolved script resource")?;
            if resource.kind != "cGameScriptResource" {
                return Err("Script reference has wrong resource type".into());
            }
            let name = field(resource, "mName").ok_or("Script resource missing name")?;
            if !names.contains(name) {
                return Err(format!("Script resource not in archive: {name}"));
            }
            let scope = match object.kind.as_str() {
                "cInitialisationScriptEntity" => Scope::Initializer,
                "cGlobalScriptEntity" => Scope::Global,
                "cTroop" | "cGroundVehicle" | "cAirVehicle" | "cWaterVehicle" | "cBuilding"
                | "cMorphingBuilding" | "cDestroyableObject" => Scope::Unit,
                _ => return Err(format!("Unsupported script owner {}", object.kind)),
            };
            let paused = match field(object, "m_UpdateWhenPaused") {
                Some("eTrue") => true,
                Some("eFalse") | None => false,
                Some(_) => return Err("Invalid script pause flag".into()),
            };
            bindings.push(Binding {
                entity: object.id,
                kind: object.kind.clone(),
                script: name.into(),
                scope,
                update_when_paused: paused,
            });
        }
        let program = Self {
            vm,
            host,
            bindings,
            initialized: false,
        };
        for binding in &program.bindings {
            program.thread(binding)?;
        }
        Ok(program)
    }
    /// Shared-state global execution, unlike isolated capability probes. Unit-script
    /// activation/lifecycle must be added separately, not inferred from army membership.
    pub fn global_scheduler(&self) -> Result<crate::mission_vm::Scheduler> {
        if !self.initialized {
            return Err("Initialize authored mission before starting globals".into());
        }
        let mut scheduler = crate::mission_vm::Scheduler::default();
        for binding in self.bindings.iter().filter(|b| b.scope == Scope::Global) {
            if scheduler
                .threads
                .insert(binding.entity.0, self.thread(binding)?)
                .is_some()
            {
                return Err("Duplicate global script owner".into());
            }
            if binding.update_when_paused {
                scheduler.update_when_paused.insert(binding.entity.0);
            }
        }
        Ok(scheduler)
    }
    /// Rust mission policy: bind all authored unit coroutines as well as globals.
    /// Scripts own their guards; unspawned actors can initialize invulnerability and wait.
    pub fn mission_scheduler(&self) -> Result<crate::mission_vm::Scheduler> {
        let mut scheduler = self.global_scheduler()?;
        for binding in self.bindings.iter().filter(|b| b.scope == Scope::Unit) {
            if scheduler
                .threads
                .insert(binding.entity.0, self.thread(binding)?)
                .is_some()
            {
                return Err("Duplicate mission script owner".into());
            }
            if binding.update_when_paused {
                scheduler.update_when_paused.insert(binding.entity.0);
            }
        }
        Ok(scheduler)
    }
    pub fn thread(&self, binding: &Binding) -> Result<Thread> {
        Self::bound_thread(&self.vm, binding)
    }
    fn bound_thread(vm: &Bootstrap, binding: &Binding) -> Result<Thread> {
        let value = vm
            .globals
            .get(&binding.script)
            .ok_or("Missing authored script entry point")?;
        let Value::Function { script, child } = value else {
            return Err("Authored script entry point must be a registered root function".into());
        };
        let p = vm
            .functions
            .get(&(script.clone(), *child))
            .ok_or("Missing script prototype")?;
        Thread::new(&binding.script, p.clone(), &[Value::Object(binding.entity)])
    }
    /// All six SP_0.1 initializers are selected from XML, never a hand-picked list.
    /// Stable object-ID order is a replacement policy, not recovered engine ordering.
    pub fn initialize(&mut self) -> Result<usize> {
        if self.initialized {
            return Err("Mission already initialized".into());
        }
        let mut vm = self.vm.clone();
        let mut host = self.host.clone();
        let mut count = 0;
        for binding in self
            .bindings
            .iter()
            .filter(|b| b.scope == Scope::Initializer)
        {
            let mut thread = Self::bound_thread(&vm, binding)?;
            if !matches!(
                thread.resume(&mut vm, &mut host, 100000)?,
                Status::Complete(_)
            ) {
                return Err(format!("Initializer yielded: {}", binding.script));
            }
            count += 1;
        }
        self.vm = vm;
        self.host = host;
        self.initialized = true;
        Ok(count)
    }
    /// Independent first-resume probes. Never represented as a running mission.
    /// Clones isolate effects, including shared captured cells, between each probe.
    pub fn probe_globals(&self) -> Result<Vec<Probe>> {
        if !self.initialized {
            return Err("Initialize before probing globals".into());
        }
        let mut report = Vec::new();
        for binding in self.bindings.iter().filter(|b| b.scope == Scope::Global) {
            let mut thread = self.thread(binding)?;
            let mut vm = self.vm.clone();
            let mut host = self.host.clone();
            let outcome = match thread.resume(&mut vm, &mut host, 100000) {
                Ok(Status::Complete(_)) => "complete".into(),
                Ok(Status::Sleeping(_)) => "yielded".into(),
                Ok(Status::Runnable) => "runnable".into(),
                Err(e) => format!("blocked: {e}"),
            };
            let Value::Function { script, child } = &self.vm.globals[&binding.script] else {
                return Err("Global probe entry point was replaced".into());
            };
            let mut reads = BTreeSet::new();
            global_reads(&self.vm.functions[&(script.clone(), *child)], &mut reads);
            report.push(Probe {
                entity: binding.entity,
                script: binding.script.clone(),
                outcome,
                instructions: thread.instructions,
                unresolved_global_reads: reads
                    .into_iter()
                    .filter(|name| !self.vm.globals.contains_key(name))
                    .collect(),
            });
        }
        Ok(report)
    }
}
fn global_reads(p: &Prototype, out: &mut BTreeSet<String>) {
    for &word in &p.code {
        if word & 63 == 5 {
            let index = ((word >> 6) & 262143) as usize;
            if let Some(lua50::Constant::String(name)) = p.constants.get(index) {
                out.insert(name.clone());
            }
        }
    }
    for child in &p.children {
        global_reads(child, out);
    }
}
