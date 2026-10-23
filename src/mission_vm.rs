//! Capability-limited mission execution. Explicit subset, not complete Lua parity.
//! All host effects are in-memory and transactional; no OS capabilities or no-op stubs.
use crate::{
    ObjectGraph, ObjectId, Result,
    lua50::{Bootstrap, Constant, Prototype, Value},
    original_scene,
};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, PartialEq)]
pub struct BoundsMessage {
    pub message: f64,
    pub army: f64,
    pub duration: f64,
    pub sprite: ObjectId,
}
#[derive(Clone, Default)]
pub struct Host {
    pub frontend: Option<crate::frontend_environment::Environment>,
    pub frontend_sprites: Option<crate::frontend_sprites::Sprites>,
    pub world: crate::mission_world::World,
    pub positions: BTreeMap<ObjectId, [f32; 3]>,
    pub known_objects: BTreeSet<ObjectId>,
    pub bounds_message: Option<BoundsMessage>,
    pub debug: Vec<Vec<Value>>,
    pub time: f64,
    /// Simulation-supplied no-pause clock, never inferred from wall-clock reads.
    pub no_pause_time: f64,
    pub frame: u64,
}
impl Host {
    pub fn from_graph(graph: &ObjectGraph) -> Result<Self> {
        let mut h = Self {
            world: crate::mission_world::World::from_graph(graph)?,
            known_objects: graph.objects.keys().copied().collect(),
            ..Self::default()
        };
        for o in graph.objects.values() {
            if let Some(raw) = original_scene::field(o, "Mat") {
                let m = original_scene::matrix(raw)?;
                h.positions.insert(o.id, [m[12], m[13], m[14]]);
            }
        }
        Ok(h)
    }
    pub fn install(&self, vm: &mut Bootstrap) {
        for name in [
            "GetObjectXPosition",
            "GetObjectZPosition",
            "OutOfBoundsMessageInfo",
            "DebugOut",
            "GetTime",
            "EndFrame",
            "WaitFor",
            "WaitForNoPause",
            "ReadControllerState",
        ] {
            vm.globals.insert(name.into(), Value::Native(name.into()));
        }
    }
    fn call(
        &mut self,
        name: &str,
        args: &[Value],
        vm: &mut Bootstrap,
    ) -> Result<(Vec<Value>, Option<Wake>)> {
        match (name, args) {
            ("OpenSprite" | "CloseSprite", _) => {
                let sprites = self
                    .frontend_sprites
                    .as_mut()
                    .ok_or("Frontend sprite backend not configured")?;
                let values = if name == "OpenSprite" {
                    sprites.open(vm, args)?
                } else {
                    sprites.close(vm, args)?
                };
                Ok((values, None))
            }
            (
                "GetFramesPerSecond"
                | "GetNumOfGameType"
                | "EnableControllerErrorMessages"
                | "BuildType"
                | "PlayVideo"
                | "IsVideoFinished"
                | "GetTotalFrames"
                | "GetFrameNum"
                | "StartVO"
                | "StopVO"
                | "CheckVOFinished"
                | "MatrixMultiply"
                | "RotationToVector"
                | "GlobeLatLonToOrientationMatrix"
                | "GlobeCalcLatLonFromLocationMtx"
                | "GlobeCalcLatLonFromOrientationMtx"
                | "GlobeCalcClickedPos"
                | "GlobeSelectCalcTargetOrientation"
                | "CalcGlobeInterection"
                | "GlobePickRegionFromMap",
                _,
            ) => {
                let env = self
                    .frontend
                    .as_mut()
                    .ok_or("Frontend environment not configured")?;
                Ok((env.call_with_vm(name, args, vm)?, None))
            }
            (
                "sin"
                | "cos"
                | "tan"
                | "mod"
                | "sqrt"
                | "abs"
                | "ceil"
                | "floor"
                | "rint"
                | "SetCurrentMissionAttempted"
                | "GetCurrentMissionAttempted"
                | "SetCurrentMissionCompleted"
                | "GetCurrentMissionCompleted"
                | "SetCurrentFMVOutSeen"
                | "GetCurrentFMVOutSeen",
                _,
            ) if self.frontend.is_some() => {
                let env = self.frontend.as_mut().unwrap();
                Ok((env.call_with_vm(name, args, vm)?, None))
            }
            ("ReadControllerState", _) => {
                let input = self
                    .world
                    .input
                    .as_ref()
                    .ok_or("Controller input backend not configured")?;
                let controller = self
                    .world
                    .controller
                    .as_ref()
                    .ok_or("Controller mask backend not configured")?;
                Ok((input.read(args, controller, self.frame)?, None))
            }
            ("GetObjectXPosition" | "GetObjectZPosition", [Value::Object(id)]) => {
                let p = self
                    .positions
                    .get(id)
                    .ok_or_else(|| format!("Object {} has no world position", id.0))?;
                Ok((
                    vec![Value::Number(
                        p[if name == "GetObjectXPosition" { 0 } else { 2 }] as f64,
                    )],
                    None,
                ))
            }
            (
                "OutOfBoundsMessageInfo",
                [
                    Value::Number(message),
                    army_val,
                    Value::Number(duration),
                    Value::Object(sprite),
                    rest @ ..,
                ],
            ) if rest.is_empty() || matches!(rest, [Value::Enum(p)] if p == "PLAYER_ONE") => {
                let army = match army_val {
                    Value::Number(n) => *n,
                    Value::Enum(s) => match s.as_str() {
                        "ARMY_SOLAR" => 0.0,
                        "ARMY_WF" => 1.0,
                        "ARMY_ANGLO" => 2.0,
                        "ARMY_TUNDRAN" => 3.0,
                        "ARMY_XYLVANIAN" => 4.0,
                        "ARMY_UNDERWORLD" => 5.0,
                        "ARMY_NEUTRAL" => 6.0,
                        _ => return Err("Invalid bounds-message army".into()),
                    },
                    _ => return Err("Invalid bounds-message arguments".into()),
                };
                if [message, &army, duration].iter().any(|n| !n.is_finite()) || *duration < 0.0 {
                    return Err("Invalid bounds-message arguments".into());
                }
                if !self.known_objects.contains(sprite) {
                    return Err("Unknown bounds-message sprite object".into());
                }
                self.bounds_message = Some(BoundsMessage {
                    message: *message,
                    army,
                    duration: *duration,
                    sprite: *sprite,
                });
                Ok((vec![], None))
            }
            ("DebugOut", _) => {
                if self.debug.len() >= 4096 {
                    return Err("Debug-event limit".into());
                }
                self.debug.push(args.to_vec());
                Ok((vec![], None))
            }
            ("GetTime", []) => Ok((vec![Value::Number(self.time)], None)),
            (
                "CameraSetFOV",
                [
                    Value::Object(id),
                    Value::Number(target),
                    Value::Enum(mode),
                    rest @ ..,
                ],
            ) if rest.is_empty()
                || matches!(rest, [Value::Number(_)])
                || matches!(rest, [Value::Enum(w)] if matches!(w.as_str(), "WAIT" | "NO_WAIT"))
                || matches!(rest, [Value::Number(_), Value::Enum(w)] if matches!(w.as_str(), "WAIT" | "NO_WAIT")) =>
            {
                let (rate, wait) = match rest {
                    [] => (0.0, "NO_WAIT"),
                    [Value::Number(r)] => (*r, "NO_WAIT"),
                    [Value::Enum(w)] => (0.0, w.as_str()),
                    [Value::Number(r), Value::Enum(w)] => (*r, w.as_str()),
                    _ => unreachable!(),
                };
                let frame = self.frame.checked_add(1).ok_or("Frame overflow")?;
                let seconds = self
                    .world
                    .cameras
                    .as_mut()
                    .ok_or("Scripted camera backend not configured")?
                    .set_fov(*id, *target, mode, rate)?;
                let deadline = self.time + seconds;
                if !deadline.is_finite() {
                    return Err("Invalid FOV deadline".into());
                }
                Ok((
                    vec![Value::Boolean(false)],
                    (wait == "WAIT").then_some(Wake {
                        frame,
                        time: deadline,
                        clock: Clock::Game,
                    }),
                ))
            }
            ("EndFrame", []) => Ok((
                vec![],
                Some(Wake {
                    frame: self.frame.checked_add(1).ok_or("Frame overflow")?,
                    time: self.time,
                    clock: Clock::Game,
                }),
            )),
            (
                "CameraFade",
                [
                    Value::Enum(direction),
                    Value::Enum(wait),
                    Value::Number(seconds),
                ],
            ) if matches!(wait.as_str(), "WAIT" | "NO_WAIT") => {
                let deadline = self.time + seconds;
                let frame = self.frame.checked_add(1).ok_or("Frame overflow")?;
                if !deadline.is_finite() {
                    return Err("Invalid fade deadline".into());
                }
                self.world
                    .cameras
                    .as_mut()
                    .ok_or("Scripted camera backend not configured")?
                    .start_fade(direction, *seconds)?;
                Ok((
                    vec![],
                    (wait == "WAIT").then_some(Wake {
                        frame,
                        time: deadline,
                        clock: Clock::Game,
                    }),
                ))
            }
            ("WaitFor" | "WaitForNoPause", [Value::Number(seconds)]) => {
                let clock = if name == "WaitFor" {
                    Clock::Game
                } else {
                    Clock::NoPause
                };
                let now = match clock {
                    Clock::Game => self.time,
                    Clock::NoPause => self.no_pause_time,
                };
                let deadline = now + seconds;
                if !seconds.is_finite() || *seconds < 0.0 || !deadline.is_finite() {
                    return Err("Invalid wait duration".into());
                }
                Ok((
                    vec![],
                    Some(Wake {
                        frame: self.frame.checked_add(1).ok_or("Frame overflow")?,
                        time: deadline,
                        clock,
                    }),
                ))
            }
            (
                "Teleport",
                [
                    Value::Object(id),
                    Value::Number(x),
                    Value::Number(z),
                    Value::Number(heading),
                    rest @ ..,
                ],
            ) if self.world.rewrite.is_some()
                && (rest.is_empty()
                    || matches!(rest, [Value::Number(margin)] if margin.is_finite() && *margin >= 0.)) =>
            {
                let margin = match rest {
                    [Value::Number(m)] => *m,
                    _ => 0.0,
                };
                let applied = self.world.teleport_scripted_with_margin(
                    &mut self.positions,
                    *id,
                    *x,
                    *z,
                    *heading,
                    margin,
                )?;
                Ok((vec![Value::Boolean(applied)], None))
            }
            _ => {
                let values = self.world.call(name, args, &self.positions)?;
                for (id, pos) in std::mem::take(&mut self.world.pending_positions) {
                    self.positions.insert(id, pos);
                }
                Ok((values, None))
            }
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Clock {
    Game,
    NoPause,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Wake {
    pub frame: u64,
    pub time: f64,
    pub clock: Clock,
}
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    Runnable,
    Sleeping(Wake),
    Complete(Vec<Value>),
}
#[derive(Clone)]
struct Caller {
    prototype: Prototype,
    registers: Vec<Value>,
    pc: usize,
    top: Option<usize>,
    destination: usize,
    results: usize,
    upvalues: Vec<usize>,
    open: BTreeMap<usize, usize>,
}
#[derive(Clone)]
pub struct Thread {
    pub name: String,
    prototype: Prototype,
    registers: Vec<Value>,
    pub pc: usize,
    pub status: Status,
    pub instructions: u64,
    callers: Vec<Caller>,
    call_path: Vec<String>,
    top: Option<usize>,
    upvalues: Vec<usize>,
    open: BTreeMap<usize, usize>,
}
fn truth(v: &Value) -> bool {
    !matches!(v, Value::Nil | Value::Boolean(false))
}
fn constant(p: &Prototype, i: usize) -> Result<Value> {
    match p.constants.get(i) {
        Some(Constant::Nil) => Ok(Value::Nil),
        Some(Constant::Number(n)) => Ok(Value::Number(*n)),
        Some(Constant::String(s)) => Ok(Value::String(s.clone())),
        None => Err("Constant index out of range".into()),
    }
}
fn number(v: Value) -> Result<f64> {
    match v {
        Value::Number(n) if n.is_finite() => Ok(n),
        _ => Err("Finite numeric operand required; coercion not implemented".into()),
    }
}
/// Ordered-comparison operand. The original engine exposes world objects to Lua
/// as plain numbers holding the object ID (e.g. `GetPlayerUnit` at 0x800e8258
/// pushes `*(obj + 0xc)` with `lua_pushnumber`), so `0 < handle` is valid
/// original script code. Other non-numbers still fail like Lua 5.0's order error.
fn handle_number(v: Value) -> Result<f64> {
    match v {
        Value::Object(id) => Ok(id.0 as f64),
        other => number(other),
    }
}
impl Thread {
    pub fn debug_context(&self, window: usize) -> String {
        let start = self.pc.saturating_sub(window);
        let end = (self.pc + window).min(self.prototype.code.len());
        let mut out = Vec::new();
        for idx in start..end {
            let word = self.prototype.code[idx];
            let op = word & 63;
            let a = (word >> 24) as usize;
            let b = ((word >> 15) & 511) as usize;
            let c = ((word >> 6) & 511) as usize;
            let bx = ((word >> 6) & 262143) as usize;
            let k = |i: usize| {
                if i >= 250 {
                    format!("{:?}", self.prototype.constants.get(i - 250))
                } else {
                    format!("R{i}({:?})", self.registers.get(i))
                }
            };
            let desc = match op {
                0 => format!("MOVE R{a} <- R{b}"),
                1 => format!("LOADK R{a} <- {:?}", self.prototype.constants.get(bx)),
                2 => format!("LOADBOOL R{a} <- {b} (skip={c})"),
                5 => format!("GETGLOBAL R{a} <- {:?}", self.prototype.constants.get(bx)),
                6 => format!("GETTABLE R{a} <- R{b}[{}]", k(c)),
                7 => format!(
                    "SETGLOBAL {:?} <- R{a}({:?})",
                    self.prototype.constants.get(bx),
                    self.registers.get(a)
                ),
                20 => format!(
                    "JMP -> {}",
                    idx.saturating_add_signed(bx as isize - 131071) + 1
                ),
                21 => format!("EQ (a={a}) {} == {}", k(b), k(c)),
                22 => format!("LT (a={a}) {} < {}", k(b), k(c)),
                23 => format!("LE (a={a}) {} <= {}", k(b), k(c)),
                24 => format!("TEST R{a} <- R{b}({:?}) (c={c})", self.registers.get(b)),
                25 => format!("CALL R{a}({:?}) b={b} c={c}", self.registers.get(a)),
                _ => format!("OP_{op} a={a} b={b} c={c} bx={bx}"),
            };
            let mark = if idx == self.pc { ">>" } else { "  " };
            out.push(format!("{mark} [{idx}] {desc}"));
        }
        out.join(" | ")
    }
    pub fn new(name: &str, p: Prototype, args: &[Value]) -> Result<Self> {
        Self::captured(name, p, args, Vec::new())
    }
    pub fn from_callable(
        name: &str,
        value: &Value,
        args: &[Value],
        vm: &Bootstrap,
    ) -> Result<Self> {
        match value {
            Value::Function { script, child } => Self::new(
                name,
                vm.functions
                    .get(&(script.clone(), *child))
                    .ok_or("Missing Lua function")?
                    .clone(),
                args,
            ),
            Value::Closure(id) => {
                let closure = vm.closures.get(*id).ok_or("Missing Lua closure")?;
                Self::captured(
                    name,
                    closure.prototype.clone(),
                    args,
                    closure.upvalues.clone(),
                )
            }
            _ => Err("Lua thread requires a Lua callable".into()),
        }
    }
    fn captured(name: &str, p: Prototype, args: &[Value], upvalues: Vec<usize>) -> Result<Self> {
        if p.upvalues as usize != upvalues.len()
            || p.vararg
            || p.stack == 0
            || p.parameters > p.stack
        {
            return Err("Unsupported thread signature".into());
        }
        let mut registers = vec![Value::Nil; p.stack as usize];
        let count = args.len().min(p.parameters as usize);
        registers[..count].clone_from_slice(&args[..count]);
        Ok(Self {
            name: name.into(),
            prototype: p,
            registers,
            pc: 0,
            status: Status::Runnable,
            instructions: 0,
            callers: Vec::new(),
            call_path: vec![name.into()],
            top: None,
            upvalues,
            open: BTreeMap::new(),
        })
    }
    fn pull(&mut self, vm: &Bootstrap) -> Result<()> {
        for (&register, &cell) in &self.open {
            self.registers[register] = vm
                .upvalues
                .get(cell)
                .ok_or("Invalid open upvalue cell")?
                .clone();
        }
        Ok(())
    }
    fn push(&self, vm: &mut Bootstrap) -> Result<()> {
        for (&register, &cell) in &self.open {
            *vm.upvalues
                .get_mut(cell)
                .ok_or("Invalid open upvalue cell")? = self.registers[register].clone();
        }
        Ok(())
    }
    fn reg(&self, i: usize) -> Result<Value> {
        self.registers
            .get(i)
            .cloned()
            .ok_or("Register index out of range".into())
    }
    fn set(&mut self, i: usize, v: Value) -> Result<()> {
        *self
            .registers
            .get_mut(i)
            .ok_or("Register destination out of range")? = v;
        Ok(())
    }
    fn results(&mut self, a: usize, c: usize, values: &[Value]) -> Result<()> {
        let count = if c == 0 { values.len() } else { c - 1 };
        for i in 0..count {
            self.set(a + i, values.get(i).cloned().unwrap_or(Value::Nil))?;
        }
        self.top = if c == 0 { Some(a + count) } else { None };
        Ok(())
    }
    fn enter(
        &mut self,
        p: Prototype,
        args: &[Value],
        a: usize,
        c: usize,
        upvalues: Vec<usize>,
    ) -> Result<()> {
        if self.callers.len() >= 128 {
            return Err("Lua call depth limit".into());
        }
        let child = Thread::captured(&self.name, p, args, upvalues)?;
        self.callers.push(Caller {
            prototype: std::mem::replace(&mut self.prototype, child.prototype),
            registers: std::mem::replace(&mut self.registers, child.registers),
            pc: std::mem::replace(&mut self.pc, 0),
            top: self.top.take(),
            destination: a,
            results: c,
            upvalues: std::mem::replace(&mut self.upvalues, child.upvalues),
            open: std::mem::take(&mut self.open),
        });
        Ok(())
    }
    fn rk(&self, i: usize) -> Result<Value> {
        if i >= 250 {
            constant(&self.prototype, i - 250)
        } else {
            self.reg(i)
        }
    }
    /// Resume atomically to next yield/return. A failed slice changes neither thread,
    /// shared Lua state nor the capability host. Earlier successful yields stay committed.
    pub fn resume(&mut self, vm: &mut Bootstrap, host: &mut Host, budget: usize) -> Result<Status> {
        if !host.time.is_finite() || host.time < 0.0 {
            return Err("Invalid logical clock".into());
        }
        if matches!(self.status, Status::Complete(_)) {
            return Ok(self.status.clone());
        }
        if let Status::Sleeping(w) = self.status
            && (host.frame < w.frame
                || match w.clock {
                    Clock::Game => host.time,
                    Clock::NoPause => host.no_pause_time,
                } < w.time)
        {
            return Ok(self.status.clone());
        }
        let mut thread = self.clone();
        let mut state = vm.clone();
        let mut effects = host.clone();
        let outcome = thread.resume_staged(&mut state, &mut effects, budget)?;
        *self = thread;
        *vm = state;
        *host = effects;
        Ok(outcome)
    }
    // Caller MUST own the enclosing transaction. Scheduler stages the entire tick once;
    // cloning the whole world again for every thread was dominating frame time.
    fn resume_staged(
        &mut self,
        vm: &mut Bootstrap,
        host: &mut Host,
        budget: usize,
    ) -> Result<Status> {
        if !host.time.is_finite() || host.time < 0.0 {
            return Err("Invalid logical clock".into());
        }
        if matches!(self.status, Status::Complete(_)) {
            return Ok(self.status.clone());
        }
        if let Status::Sleeping(w) = self.status
            && (host.frame < w.frame
                || match w.clock {
                    Clock::Game => host.time,
                    Clock::NoPause => host.no_pause_time,
                } < w.time)
        {
            return Ok(self.status.clone());
        }
        self.status = Status::Runnable;
        self.run(vm, host, budget).map_err(|e| {
            format!(
                "{} pc {}: {e}",
                self.call_path.join(" -> "),
                self.pc.saturating_sub(1)
            )
        })
    }
    fn run(&mut self, vm: &mut Bootstrap, host: &mut Host, budget: usize) -> Result<Status> {
        for _ in 0..budget {
            self.pull(vm)?;
            let word = *self
                .prototype
                .code
                .get(self.pc)
                .ok_or("Instruction pointer out of range")?;
            self.pc += 1;
            self.instructions += 1;
            let op = word & 63;
            let a = (word >> 24) as usize;
            let b = ((word >> 15) & 511) as usize;
            let c = ((word >> 6) & 511) as usize;
            let bx = ((word >> 6) & 262143) as usize;
            match op {
                0 => self.set(a, self.reg(b)?)?,
                1 => self.set(a, constant(&self.prototype, bx)?)?,
                2 => {
                    if b > 1 || c > 1 {
                        return Err("Invalid LOADBOOL flag".into());
                    }
                    self.set(a, Value::Boolean(b != 0))?;
                    if c != 0 {
                        self.pc += 1;
                    }
                }
                3 => {
                    if b < a || b >= self.registers.len() {
                        return Err("Invalid LOADNIL range".into());
                    }
                    for i in a..=b {
                        self.set(i, Value::Nil)?;
                    }
                }
                4 => {
                    let cell = *self.upvalues.get(b).ok_or("GETUPVAL index out of range")?;
                    self.set(
                        a,
                        vm.upvalues
                            .get(cell)
                            .ok_or("GETUPVAL cell missing")?
                            .clone(),
                    )?;
                }
                8 => {
                    let cell = *self.upvalues.get(b).ok_or("SETUPVAL index out of range")?;
                    let value = self.reg(a)?;
                    *vm.upvalues.get_mut(cell).ok_or("SETUPVAL cell missing")? = value.clone();
                    for (&register, &open_cell) in &self.open {
                        if open_cell == cell {
                            self.registers[register] = value.clone();
                        }
                    }
                }
                5 => {
                    let Value::String(key) = constant(&self.prototype, bx)? else {
                        return Err("Global key must be a string".into());
                    };
                    self.set(a, vm.globals.get(&key).cloned().unwrap_or(Value::Nil))?;
                }
                6 => {
                    let Value::Table(table) = self.reg(b)? else {
                        return Err("GETTABLE requires a table".into());
                    };
                    self.set(a, vm.table_get(table, &self.rk(c)?)?)?;
                }
                7 => {
                    let Value::String(key) = constant(&self.prototype, bx)? else {
                        return Err("Global key must be a string".into());
                    };
                    if vm.globals.len() >= 10000 && !vm.globals.contains_key(&key) {
                        return Err("Global limit".into());
                    }
                    vm.globals.insert(key, self.reg(a)?);
                }
                9 => {
                    let Value::Table(table) = self.reg(a)? else {
                        return Err("SETTABLE requires a table".into());
                    };
                    vm.table_set(table, self.rk(b)?, self.rk(c)?)?;
                }
                10 => {
                    if vm.tables.len() >= 4096 {
                        return Err("Table limit".into());
                    }
                    self.set(a, Value::Table(vm.tables.len()))?;
                    vm.tables.push(BTreeMap::new());
                }
                12..=16 => {
                    let left = number(self.rk(b)?)?;
                    let right = number(self.rk(c)?)?;
                    let n = match op {
                        12 => left + right,
                        13 => left - right,
                        14 => left * right,
                        15 => left / right,
                        _ => left.powf(right),
                    };
                    if !n.is_finite() {
                        return Err("Nonfinite arithmetic result".into());
                    }
                    self.set(a, Value::Number(n))?;
                }
                17 => self.set(a, Value::Number(-number(self.reg(b)?)?))?,
                18 => self.set(a, Value::Boolean(!truth(&self.reg(b)?)))?,
                20 => {
                    self.pc = self
                        .pc
                        .checked_add_signed(bx as isize - 131071)
                        .ok_or("Invalid jump")?
                }
                21..=23 => {
                    if a > 1 {
                        return Err("Invalid comparison flag".into());
                    }
                    let left = self.rk(b)?;
                    let right = self.rk(c)?;
                    let yes = match op {
                        21 => match (&left, &right) {
                            (Value::Object(id), Value::Number(n))
                            | (Value::Number(n), Value::Object(id)) => *n == id.0 as f64,
                            (Value::Enum(s), Value::Number(n))
                            | (Value::Number(n), Value::Enum(s))
                                if s == "ID_NONE" =>
                            {
                                *n == 0.0
                            }
                            _ => left == right,
                        },
                        22 => handle_number(left)? < handle_number(right)?,
                        _ => handle_number(left)? <= handle_number(right)?,
                    };
                    if yes != (a != 0) {
                        self.pc += 1;
                    }
                }
                24 => {
                    if c > 1 {
                        return Err("Invalid TEST flag".into());
                    }
                    let value = self.reg(b)?;
                    if truth(&value) == (c != 0) {
                        self.set(a, value)?;
                    } else {
                        self.pc += 1;
                    }
                }
                25 => {
                    let end = if b == 0 {
                        self.top.ok_or("Open CALL without result top")?
                    } else {
                        a + b
                    };
                    if end < a + 1 {
                        return Err("Invalid open argument range".into());
                    }
                    let args = (a + 1..end)
                        .map(|i| self.reg(i))
                        .collect::<Result<Vec<_>>>()?;
                    let callable = self.reg(a)?;
                    match callable {
                        Value::Native(name) => {
                            let (values, wake) = host
                                .call(&name, &args, vm)
                                .map_err(|e| format!("{name}: {e}"))?;
                            self.results(a, c, &values)?;
                            if let Some(wake) = wake {
                                self.push(vm)?;
                                self.status = Status::Sleeping(wake);
                                return Ok(self.status.clone());
                            }
                        }
                        Value::Function { ref script, child } => {
                            let key = (script.clone(), child);
                            let label = vm
                                .globals
                                .iter()
                                .find(|(_, v)| **v == callable)
                                .map(|(name, _)| name.clone())
                                .unwrap_or_else(|| format!("{callable:?}"));
                            let p = vm
                                .functions
                                .get(&key)
                                .ok_or("Unresolved Lua function")?
                                .clone();
                            self.push(vm)?;
                            self.enter(p, &args, a, c, Vec::new())?;
                            self.call_path.push(label);
                        }
                        Value::Closure(id) => {
                            let label = vm
                                .globals
                                .iter()
                                .find(|(_, v)| **v == callable)
                                .map(|(name, _)| name.clone())
                                .unwrap_or_else(|| format!("{callable:?}"));
                            let closure = vm.closures.get(id).ok_or("Unknown closure")?.clone();
                            self.push(vm)?;
                            self.enter(closure.prototype, &args, a, c, closure.upvalues)?;
                            self.call_path.push(label);
                        }
                        _ => return Err("Attempt to call a non-function".into()),
                    }
                }
                27 => {
                    let end = if b == 0 {
                        self.top.ok_or("Open RETURN without result top")?
                    } else {
                        a + b - 1
                    };
                    if end < a {
                        return Err("Invalid open return range".into());
                    }
                    let values = (a..end).map(|i| self.reg(i)).collect::<Result<Vec<_>>>()?;
                    self.push(vm)?;
                    if let Some(caller) = self.callers.pop() {
                        self.call_path.pop();
                        self.prototype = caller.prototype;
                        self.registers = caller.registers;
                        self.pc = caller.pc;
                        self.top = caller.top;
                        self.upvalues = caller.upvalues;
                        self.open = caller.open;
                        self.pull(vm)?;
                        self.results(caller.destination, caller.results, &values)?;
                    } else {
                        self.open.clear();
                        self.status = Status::Complete(values);
                        return Ok(self.status.clone());
                    }
                }
                28 => {
                    let idx = number(self.reg(a)?)?;
                    let limit = number(self.reg(a + 1)?)?;
                    let step = number(self.reg(a + 2)?)?;
                    let next = idx + step;
                    if !next.is_finite() {
                        return Err("Nonfinite FORLOOP index".into());
                    }
                    let cont = if step > 0.0 {
                        next <= limit
                    } else {
                        next >= limit
                    };
                    if cont {
                        self.set(a, Value::Number(next))?;
                        self.pc = self
                            .pc
                            .checked_add_signed(bx as isize - 131071)
                            .ok_or("Invalid FORLOOP jump")?;
                    }
                }
                31 | 32 => {
                    let Value::Table(table) = self.reg(a)? else {
                        return Err("SETLIST requires a table".into());
                    };
                    // Lua 5.0 uses Bx in groups of 32 (not Lua 5.1's B/C layout).
                    let count = if op == 31 {
                        (bx & 31) + 1
                    } else {
                        self.top
                            .take()
                            .ok_or("Open SETLIST without result top")?
                            .checked_sub(a + 1)
                            .ok_or("Open SETLIST top precedes values")?
                    };
                    let base = bx & !31;
                    // Read every source before mutation; the enclosing resume is transactional.
                    let values: Vec<_> = (1..=count)
                        .map(|i| self.reg(a + i))
                        .collect::<Result<_>>()?;
                    for (i, value) in values.into_iter().enumerate().rev() {
                        vm.table_set(table, Value::Number((base + i + 1) as f64), value)?;
                    }
                }
                33 => {
                    if a > self.registers.len() {
                        return Err("CLOSE register out of range".into());
                    }
                    self.push(vm)?;
                    self.open.retain(|&register, _| register < a);
                }
                34 => {
                    let p = self
                        .prototype
                        .children
                        .get(bx)
                        .ok_or("Missing closure prototype")?
                        .clone();
                    if vm.closures.len() >= 10000 {
                        return Err("Closure limit".into());
                    }
                    let mut captures = Vec::new();
                    for _ in 0..p.upvalues {
                        let binding = *self
                            .prototype
                            .code
                            .get(self.pc)
                            .ok_or("Missing closure capture binding")?;
                        self.pc += 1;
                        let source = ((binding >> 15) & 511) as usize;
                        let cell = match binding & 63 {
                            0 => {
                                self.reg(source)?;
                                if let Some(&cell) = self.open.get(&source) {
                                    cell
                                } else {
                                    if vm.upvalues.len() >= 10000 {
                                        return Err("Upvalue cell limit".into());
                                    }
                                    let cell = vm.upvalues.len();
                                    vm.upvalues.push(self.reg(source)?);
                                    self.open.insert(source, cell);
                                    cell
                                }
                            }
                            4 => *self
                                .upvalues
                                .get(source)
                                .ok_or("Capture upvalue index out of range")?,
                            _ => return Err("Invalid closure capture opcode".into()),
                        };
                        captures.push(cell);
                    }
                    let id = vm.closures.len();
                    vm.closures.push(crate::lua50::Closure {
                        prototype: p,
                        upvalues: captures,
                    });
                    self.set(a, Value::Closure(id))?;
                }
                _ => return Err(format!("Opcode {op} not implemented; no fallback")),
            }
            self.push(vm)?;
        }
        Err("Instruction budget exhausted".into())
    }
}
/// Deterministic proposed scheduler policy, NOT verified original resume ordering.
#[derive(Clone, Default)]
pub struct Scheduler {
    pub threads: BTreeMap<u64, Thread>,
    /// Authored m_UpdateWhenPaused bindings; this is not a global pause bypass.
    pub update_when_paused: BTreeSet<u64>,
    pub paused: bool,
}
impl Scheduler {
    /// Submit device samples and execute the next tick as one transaction. Failed
    /// scripts must not consume a key edge or publish a new input frame either.
    pub fn tick_with_input(
        &mut self,
        vm: &mut Bootstrap,
        host: &mut Host,
        dt: f64,
        buttons: BTreeMap<String, crate::mission_input::Button>,
        budget_per_thread: usize,
    ) -> Result<()> {
        let mut staged = host.clone();
        let frame = staged.frame.checked_add(1).ok_or("Frame overflow")?;
        staged
            .world
            .input
            .as_mut()
            .ok_or("Controller input backend not configured")?
            .submit(frame, buttons)?;
        self.tick(vm, &mut staged, dt, budget_per_thread)?;
        *host = staged;
        Ok(())
    }
    pub fn tick(
        &mut self,
        vm: &mut Bootstrap,
        host: &mut Host,
        dt: f64,
        budget_per_thread: usize,
    ) -> Result<()> {
        if !dt.is_finite() || dt < 0.0 {
            return Err("Invalid tick duration".into());
        }
        let mut state = vm.clone();
        let mut effects = host.clone();
        let mut threads = self.threads.clone();
        if !self.paused {
            effects.time += dt;
        }
        effects.no_pause_time += dt;
        effects.frame = effects.frame.checked_add(1).ok_or("Frame overflow")?;
        if !effects.time.is_finite() || !effects.no_pause_time.is_finite() {
            return Err("Clock overflow".into());
        }
        if !self.paused {
            effects.world.advance(dt, &mut effects.positions)?;
        }
        let keys: Vec<u64> = threads.keys().copied().collect();
        for id in keys {
            if effects.world.killed_scripts.contains(&id) {
                continue;
            }
            if (!self.paused || self.update_when_paused.contains(&id))
                && let Some(thread) = threads.get_mut(&id)
            {
                if thread.pc > 0
                    && let Some(u) = effects.world.units.get(&crate::ObjectId(id))
                    && ((u.active == Some(false)
                        && !effects
                            .world
                            .tick_when_despawned
                            .contains(&crate::ObjectId(id)))
                        || (u.health <= 0.0
                            && !effects.world.tick_when_dead.contains(&crate::ObjectId(id))))
                {
                    continue;
                }
                thread.resume_staged(&mut state, &mut effects, budget_per_thread)?;
            }
        }
        threads.retain(|id, _| !effects.world.killed_scripts.contains(id));
        *vm = state;
        *host = effects;
        self.threads = threads;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn word(op: u32, a: u32, b: u32, c: u32) -> u32 {
        op | a << 24 | b << 15 | c << 6
    }
    fn proto(constants: Vec<Constant>, code: Vec<u32>) -> Prototype {
        Prototype {
            parameters: 0,
            vararg: false,
            upvalues: 0,
            stack: 8,
            constants,
            children: vec![],
            code,
        }
    }
    fn wait_script(native: &str, key: &str, duration: f64) -> Prototype {
        proto(
            vec![
                Constant::String(native.into()),
                Constant::Number(duration),
                Constant::String(key.into()),
            ],
            vec![
                word(5, 0, 0, 0),
                word(1, 1, 0, 1),
                word(25, 0, 2, 1),
                word(2, 0, 1, 0),
                word(7, 0, 0, 2),
                word(27, 0, 1, 0),
            ],
        )
    }
    #[test]
    fn input_submission_and_all_script_effects_commit_together() {
        use crate::{
            mission_controls::Controller,
            mission_input::{Button, ITEMS, Input},
            mission_world::World,
        };
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        World::install(&mut vm).unwrap();
        host.world.controller = Some(Controller::fresh_player());
        host.world.input = Some(Input::single_player());
        let script = proto(
            [
                "ReadControllerState",
                "constant",
                "CONTROL_FIRE",
                "CONTROL_JUST_PRESSED",
                "PLAYER_ONE",
                "observed",
            ]
            .map(|s| Constant::String(s.into()))
            .into(),
            vec![
                word(5, 0, 0, 0),
                word(5, 1, 0, 1),
                word(6, 2, 1, 253),
                word(6, 3, 1, 254),
                word(6, 1, 1, 252),
                word(25, 0, 4, 2),
                word(7, 0, 0, 5),
                word(27, 0, 1, 0),
            ],
        );
        let bad = proto(
            vec![Constant::String("unsupported".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 1)],
        );
        let mut scheduler = Scheduler::default();
        scheduler
            .threads
            .insert(1, Thread::new("input reader", script, &[]).unwrap());
        scheduler
            .threads
            .insert(2, Thread::new("failing callback", bad, &[]).unwrap());
        let mut buttons = ITEMS
            .iter()
            .map(|s| ((*s).into(), Button::default()))
            .collect::<BTreeMap<_, _>>();
        buttons.insert(
            "CONTROL_FIRE".into(),
            Button {
                held: true,
                just_pressed: true,
                just_released: false,
            },
        );
        assert!(
            scheduler
                .tick_with_input(&mut vm, &mut host, 0.1, buttons.clone(), 100)
                .is_err()
        );
        assert_eq!(host.world.input, Some(Input::single_player()));
        assert_eq!(host.frame, 0);
        assert_eq!(host.no_pause_time, 0.0);
        assert!(!vm.globals.contains_key("observed"));
        scheduler.threads.remove(&2);
        scheduler
            .tick_with_input(&mut vm, &mut host, 0.1, buttons, 100)
            .unwrap();
        assert_eq!(vm.globals["observed"], Value::Boolean(true));
        assert_eq!(host.frame, 1);
        assert_eq!(host.time, 0.1);
    }
    #[test]
    fn no_pause_wait_uses_separate_clock_not_a_global_pause_bypass() {
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut s = Scheduler::default();
        s.threads.insert(
            1,
            Thread::new("game wait", wait_script("WaitFor", "game", 1.0), &[]).unwrap(),
        );
        s.threads.insert(
            2,
            Thread::new("pause wait", wait_script("WaitForNoPause", "ui", 1.0), &[]).unwrap(),
        );
        s.threads.insert(
            3,
            Thread::new(
                "ineligible wait",
                wait_script("WaitForNoPause", "hidden", 1.0),
                &[],
            )
            .unwrap(),
        );
        s.update_when_paused.extend([1, 2]);
        s.tick(&mut vm, &mut host, 0.25, 100).unwrap();
        s.paused = true;
        s.tick(&mut vm, &mut host, 1.0, 100).unwrap();
        assert_eq!(host.time, 0.25);
        assert_eq!(host.no_pause_time, 1.25);
        assert_eq!(vm.globals["ui"], Value::Boolean(true));
        assert!(!vm.globals.contains_key("game"));
        assert!(!vm.globals.contains_key("hidden"));
        s.paused = false;
        s.tick(&mut vm, &mut host, 1.0, 100).unwrap();
        assert_eq!(vm.globals["game"], Value::Boolean(true));
        assert_eq!(vm.globals["hidden"], Value::Boolean(true));
    }
    #[test]
    fn no_pause_zero_wait_yields_and_invalid_wait_or_clock_does_not_commit() {
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut t =
            Thread::new("zero", wait_script("WaitForNoPause", "zero_done", 0.0), &[]).unwrap();
        t.resume(&mut vm, &mut host, 100).unwrap();
        assert!(matches!(t.status, Status::Sleeping(_)));
        host.no_pause_time = 10.0;
        t.resume(&mut vm, &mut host, 100).unwrap();
        assert!(!vm.globals.contains_key("zero_done"));
        host.frame = 1;
        t.resume(&mut vm, &mut host, 100).unwrap();
        assert_eq!(vm.globals["zero_done"], Value::Boolean(true));
        for duration in [-1.0, f64::INFINITY, f64::NAN] {
            let mut t = Thread::new(
                "invalid",
                wait_script("WaitForNoPause", "invalid_done", duration),
                &[],
            )
            .unwrap();
            assert!(t.resume(&mut vm, &mut host, 100).is_err());
            assert_eq!(t.pc, 0);
            assert!(!vm.globals.contains_key("invalid_done"));
        }
        let mut s = Scheduler {
            paused: true,
            ..Scheduler::default()
        };
        host.no_pause_time = f64::MAX;
        let before = host.frame;
        assert!(s.tick(&mut vm, &mut host, f64::MAX, 100).is_err());
        assert_eq!(host.no_pause_time, f64::MAX);
        assert_eq!(host.frame, before);
    }
    #[test]
    fn paused_bindings_resume_without_advancing_game_time() {
        let script = |key: &str| {
            proto(
                vec![
                    Constant::String("EndFrame".into()),
                    Constant::String(key.into()),
                ],
                vec![
                    word(5, 0, 0, 0),
                    word(25, 0, 1, 1),
                    word(2, 0, 1, 0),
                    word(7, 0, 0, 1),
                    word(27, 0, 1, 0),
                ],
            )
        };
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut scheduler = Scheduler::default();
        scheduler.threads.insert(
            1,
            Thread::new("pause UI", script("paused_done"), &[]).unwrap(),
        );
        scheduler.threads.insert(
            2,
            Thread::new("gameplay", script("game_done"), &[]).unwrap(),
        );
        scheduler.update_when_paused.insert(1);
        scheduler.paused = true;
        scheduler.tick(&mut vm, &mut host, 5.0, 100).unwrap();
        assert_eq!(host.time, 0.0);
        assert_eq!(scheduler.threads[&2].pc, 0);
        assert!(matches!(scheduler.threads[&1].status, Status::Sleeping(_)));
        scheduler.tick(&mut vm, &mut host, 5.0, 100).unwrap();
        assert_eq!(host.time, 0.0);
        assert_eq!(vm.globals["paused_done"], Value::Boolean(true));
        assert!(!vm.globals.contains_key("game_done"));
        scheduler.paused = false;
        scheduler.tick(&mut vm, &mut host, 0.25, 100).unwrap();
        scheduler.tick(&mut vm, &mut host, 0.25, 100).unwrap();
        assert_eq!(host.time, 0.5);
        assert_eq!(vm.globals["game_done"], Value::Boolean(true));
    }
    #[test]
    fn failing_paused_callback_rolls_back_every_global_and_frame() {
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut scheduler = Scheduler::default();
        let good = proto(
            vec![Constant::String("published".into())],
            vec![word(2, 0, 1, 0), word(7, 0, 0, 0), word(27, 0, 1, 0)],
        );
        let bad = proto(
            vec![Constant::String("unsupported".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 1)],
        );
        scheduler
            .threads
            .insert(1, Thread::new("first", good, &[]).unwrap());
        scheduler
            .threads
            .insert(2, Thread::new("second", bad, &[]).unwrap());
        scheduler.update_when_paused.extend([1, 2]);
        scheduler.paused = true;
        assert!(scheduler.tick(&mut vm, &mut host, 0.1, 100).is_err());
        assert!(!vm.globals.contains_key("published"));
        assert_eq!(host.frame, 0);
        assert_eq!(host.time, 0.0);
        assert_eq!(host.no_pause_time, 0.0);
        assert!(
            scheduler
                .threads
                .values()
                .all(|t| t.pc == 0 && t.status == Status::Runnable)
        );
    }
    #[test]
    fn yield_resume_and_pause() {
        let p = proto(
            vec![
                Constant::String("WaitFor".into()),
                Constant::Number(1.0),
                Constant::String("done".into()),
            ],
            vec![
                word(5, 0, 0, 0),
                word(1, 1, 0, 1),
                word(25, 0, 2, 1),
                word(2, 0, 1, 0),
                word(7, 0, 0, 2),
                word(27, 0, 1, 0),
            ],
        );
        let mut vm = Bootstrap::new();
        let mut h = Host::default();
        h.install(&mut vm);
        let mut s = Scheduler::default();
        s.threads.insert(1, Thread::new("wait", p, &[]).unwrap());
        s.tick(&mut vm, &mut h, 0.25, 100).unwrap();
        assert!(!vm.globals.contains_key("done"));
        s.paused = true;
        s.tick(&mut vm, &mut h, 9.0, 100).unwrap();
        assert_eq!(h.time, 0.25);
        s.paused = false;
        s.tick(&mut vm, &mut h, 0.5, 100).unwrap();
        assert!(!vm.globals.contains_key("done"));
        s.tick(&mut vm, &mut h, 0.5, 100).unwrap();
        assert_eq!(vm.globals["done"], Value::Boolean(true));
    }
    #[test]
    fn failed_tick_does_not_publish_or_advance_music_pcm() {
        use crate::{
            dsp_audio::Audio,
            mission_music::{Bank, Stereo, Transport},
        };
        use std::sync::Arc;
        let a = Audio {
            sample_rate: 8000,
            pcm: vec![1000; 8000],
            looping: false,
            loop_nibbles: [2, 17],
        };
        let bank = Bank {
            tracks: BTreeMap::from([(
                "INTRO".into(),
                Arc::new(Stereo::new(a.clone(), a).unwrap()),
            )]),
            level_gain: 0.8,
        };
        let mut h = Host::default();
        let mut music = Transport::new(Arc::new(bank));
        music
            .call("StartMusic", &[Value::Enum("INTRO".into())])
            .unwrap();
        h.world.music = Some(music);
        let mut vm = Bootstrap::new();
        let bad = proto(
            vec![Constant::String("missing".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 1)],
        );
        let mut s = Scheduler::default();
        s.threads.insert(
            1,
            Thread::new("failure after PCM generation", bad, &[]).unwrap(),
        );
        assert!(s.tick(&mut vm, &mut h, 0.5, 100).is_err());
        let music = h.world.music.as_ref().unwrap();
        assert_eq!(music.cursor, 0);
        assert_eq!(music.sequence, 0);
        assert!(music.block.is_empty());
        assert_eq!(h.time, 0.0);
        s.threads.clear();
        s.tick(&mut vm, &mut h, 0.5, 100).unwrap();
        let music = h.world.music.as_ref().unwrap();
        assert_eq!(music.cursor, 4000);
        assert_eq!(music.block.len(), 8000);
        assert_eq!(music.sequence, 1);
    }
    #[test]
    fn nested_failure_reports_actual_helper_and_rolls_back_call_path() {
        let code = vec![word(5, 0, 0, 0), word(25, 0, 1, 1), word(27, 0, 1, 0)];
        let root = proto(vec![Constant::String("helper".into())], code.clone());
        let helper = proto(vec![Constant::String("missing".into())], code);
        let mut vm = Bootstrap::new();
        vm.functions.insert(("fixture".into(), 0), helper);
        vm.globals.insert(
            "helper".into(),
            Value::Function {
                script: "fixture".into(),
                child: 0,
            },
        );
        let mut h = Host::default();
        let mut t = Thread::new("root", root, &[]).unwrap();
        let error = t.resume(&mut vm, &mut h, 100).unwrap_err();
        assert!(error.contains("root -> helper pc 1"), "{error}");
        assert_eq!(t.call_path, vec!["root"]);
        assert!(t.callers.is_empty());
    }
    #[test]
    fn unknown_call_rolls_back_globals_and_host() {
        let p = proto(
            vec![
                Constant::String("DebugOut".into()),
                Constant::String("uncommitted".into()),
            ],
            vec![
                word(5, 0, 0, 0),
                word(25, 0, 1, 1),
                word(2, 0, 1, 0),
                word(7, 0, 0, 1),
                word(25, 1, 1, 1),
            ],
        );
        let mut vm = Bootstrap::new();
        let mut h = Host::default();
        h.install(&mut vm);
        let mut t = Thread::new("bad", p, &[]).unwrap();
        assert!(t.resume(&mut vm, &mut h, 100).is_err());
        assert!(h.debug.is_empty());
        assert!(!vm.globals.contains_key("uncommitted"));
        assert_eq!(t.pc, 0);
    }
    #[test]
    fn zero_is_truthy() {
        let p = proto(
            vec![Constant::Number(0.0)],
            vec![word(1, 0, 0, 0), word(18, 1, 0, 0), word(27, 1, 2, 0)],
        );
        let mut t = Thread::new("truth", p, &[]).unwrap();
        assert_eq!(
            t.resume(&mut Bootstrap::new(), &mut Host::default(), 10)
                .unwrap(),
            Status::Complete(vec![Value::Boolean(false)])
        );
    }
    #[test]
    fn nested_lua_yield_and_open_returns() {
        let child = proto(
            vec![
                Constant::String("EndFrame".into()),
                Constant::String("GetTime".into()),
            ],
            vec![
                word(5, 1, 0, 0),
                word(25, 1, 1, 1),
                word(5, 0, 0, 1),
                word(25, 0, 1, 0),
                word(27, 0, 0, 0),
            ],
        );
        let mut p = proto(
            vec![Constant::String("DebugOut".into())],
            vec![
                word(5, 0, 0, 0),
                word(34, 1, 0, 0),
                word(25, 1, 1, 0),
                word(25, 0, 0, 1),
                word(27, 0, 1, 0),
            ],
        );
        p.children.push(child);
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut t = Thread::new("caller", p, &[]).unwrap();
        assert!(matches!(
            t.resume(&mut vm, &mut host, 100).unwrap(),
            Status::Sleeping(_)
        ));
        assert_eq!(t.callers.len(), 1);
        host.frame = 1;
        host.time = 2.5;
        assert!(matches!(
            t.resume(&mut vm, &mut host, 100).unwrap(),
            Status::Complete(_)
        ));
        assert_eq!(host.debug, vec![vec![Value::Number(2.5)]]);
        assert!(t.callers.is_empty());
    }
    fn getter() -> Prototype {
        let mut p = proto(vec![], vec![word(4, 0, 0, 0), word(27, 0, 2, 0)]);
        p.upvalues = 1;
        p
    }
    #[test]
    fn captured_siblings_share_open_and_closed_cells() {
        let mut setter = proto(vec![], vec![word(8, 0, 0, 0), word(27, 0, 1, 0)]);
        setter.upvalues = 1;
        setter.parameters = 1;
        let mut p = proto(
            vec![
                Constant::Number(10.0),
                Constant::Number(20.0),
                Constant::String("set".into()),
                Constant::String("get".into()),
                Constant::Number(30.0),
            ],
            vec![
                word(1, 0, 0, 0),
                word(34, 1, 0, 0),
                word(0, 0, 0, 0),
                word(7, 1, 0, 2),
                word(34, 2, 0, 1),
                word(0, 0, 0, 0),
                word(7, 2, 0, 3),
                word(1, 0, 0, 1),
                word(0, 3, 1, 0),
                word(1, 4, 0, 4),
                word(25, 3, 2, 1),
                word(27, 0, 2, 0),
            ],
        );
        p.children = vec![setter, getter()];
        let mut vm = Bootstrap::new();
        let mut h = Host::default();
        let mut t = Thread::new("capture", p, &[]).unwrap();
        assert_eq!(
            t.resume(&mut vm, &mut h, 100).unwrap(),
            Status::Complete(vec![Value::Number(30.0)])
        );
        assert_eq!(vm.upvalues, vec![Value::Number(30.0)]);
        let p = proto(
            vec![Constant::String("get".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 2), word(27, 0, 2, 0)],
        );
        let mut t = Thread::new("read closed", p, &[]).unwrap();
        assert_eq!(
            t.resume(&mut vm, &mut h, 100).unwrap(),
            Status::Complete(vec![Value::Number(30.0)])
        );
    }
    #[test]
    fn close_detaches_reused_registers() {
        let mut p = proto(
            vec![Constant::Number(10.0), Constant::Number(20.0)],
            vec![
                word(1, 0, 0, 0),
                word(34, 1, 0, 0),
                word(0, 0, 0, 0),
                word(33, 0, 0, 0),
                word(1, 0, 0, 1),
                word(34, 2, 0, 0),
                word(0, 0, 0, 0),
                word(27, 1, 3, 0),
            ],
        );
        p.children = vec![getter()];
        let mut vm = Bootstrap::new();
        let mut h = Host::default();
        let mut t = Thread::new("close", p, &[]).unwrap();
        t.resume(&mut vm, &mut h, 100).unwrap();
        assert_eq!(vm.upvalues, vec![Value::Number(10.0), Value::Number(20.0)]);
        for (id, expected) in [(0, 10.0), (1, 20.0)] {
            let closure = vm.closures[id].clone();
            let mut t =
                Thread::captured("closed", closure.prototype, &[], closure.upvalues).unwrap();
            assert_eq!(
                t.resume(&mut vm, &mut h, 10).unwrap(),
                Status::Complete(vec![Value::Number(expected)])
            );
        }
    }
    #[test]
    fn captured_cells_survive_yield_and_rollback_failures() {
        let mut p = proto(
            vec![
                Constant::Number(5.0),
                Constant::String("EndFrame".into()),
                Constant::Number(9.0),
            ],
            vec![
                word(1, 0, 0, 0),
                word(8, 0, 0, 0),
                word(5, 1, 0, 1),
                word(25, 1, 1, 1),
                word(1, 0, 0, 2),
                word(8, 0, 0, 0),
                word(25, 2, 1, 1),
            ],
        );
        p.upvalues = 1;
        let mut vm = Bootstrap::new();
        vm.upvalues.push(Value::Number(1.0));
        let mut h = Host::default();
        h.install(&mut vm);
        let mut t = Thread::captured("rollback", p, &[], vec![0]).unwrap();
        assert!(matches!(
            t.resume(&mut vm, &mut h, 100).unwrap(),
            Status::Sleeping(_)
        ));
        assert_eq!(vm.upvalues[0], Value::Number(5.0));
        let pc = t.pc;
        h.frame = 1;
        assert!(t.resume(&mut vm, &mut h, 100).is_err());
        assert_eq!(vm.upvalues[0], Value::Number(5.0));
        assert_eq!(t.pc, pc);
    }
    #[test]
    fn captures_inherit_cells_and_reject_bad_descriptors() {
        let mut middle = proto(
            vec![],
            vec![word(34, 0, 0, 0), word(4, 0, 0, 0), word(27, 0, 2, 0)],
        );
        middle.upvalues = 1;
        middle.children = vec![getter()];
        let mut vm = Bootstrap::new();
        vm.upvalues.push(Value::Number(42.0));
        let mut h = Host::default();
        let mut t = Thread::captured("nested", middle, &[], vec![0]).unwrap();
        assert_eq!(
            t.resume(&mut vm, &mut h, 30).unwrap(),
            Status::Complete(vec![Value::Closure(0)])
        );
        assert_eq!(vm.closures[0].upvalues, vec![0]);
        let mut p = proto(
            vec![],
            vec![word(34, 0, 0, 0), word(1, 0, 0, 0), word(27, 0, 1, 0)],
        );
        p.children = vec![getter()];
        let mut t = Thread::new("invalid capture", p, &[]).unwrap();
        assert!(
            t.resume(&mut vm, &mut h, 30)
                .unwrap_err()
                .contains("capture opcode")
        );
        assert_eq!(vm.closures.len(), 1);
        assert_eq!(t.pc, 0);
    }
    #[test]
    fn recursion_depth_failure_rolls_back() {
        let p = proto(
            vec![Constant::String("self".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 1), word(27, 0, 1, 0)],
        );
        let mut vm = Bootstrap::new();
        vm.closures.push(crate::lua50::Closure {
            prototype: p.clone(),
            upvalues: vec![],
        });
        vm.globals.insert("self".into(), Value::Closure(0));
        let mut t = Thread::new("recursion", p, &[]).unwrap();
        let mut host = Host::default();
        assert!(
            t.resume(&mut vm, &mut host, 10000)
                .unwrap_err()
                .contains("depth")
        );
        assert_eq!(t.pc, 0);
        assert!(t.callers.is_empty());
    }
    #[test]
    fn fixed_parameters_ignore_extras_and_pad_nil() {
        let mut p = proto(vec![], vec![word(27, 0, 3, 0)]);
        p.parameters = 2;
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        let mut t = Thread::new("params", p.clone(), &[Value::Number(5.0)]).unwrap();
        assert_eq!(
            t.resume(&mut vm, &mut host, 10).unwrap(),
            Status::Complete(vec![Value::Number(5.0), Value::Nil])
        );
        let mut t =
            Thread::new("params", p, &[Value::Nil, Value::Nil, Value::Number(99.0)]).unwrap();
        assert_eq!(t.reg(2).unwrap(), Value::Nil);
        t.resume(&mut vm, &mut host, 10).unwrap();
    }
    #[test]
    fn end_frame_requires_a_new_frame() {
        let p = proto(
            vec![Constant::String("EndFrame".into())],
            vec![word(5, 0, 0, 0), word(25, 0, 1, 1), word(27, 0, 1, 0)],
        );
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut t = Thread::new("yield", p, &[]).unwrap();
        assert!(matches!(
            t.resume(&mut vm, &mut host, 10).unwrap(),
            Status::Sleeping(_)
        ));
        host.time = 100.0;
        assert!(matches!(
            t.resume(&mut vm, &mut host, 10).unwrap(),
            Status::Sleeping(_)
        ));
        host.frame = 1;
        assert!(matches!(
            t.resume(&mut vm, &mut host, 10).unwrap(),
            Status::Complete(_)
        ));
    }
    #[test]
    fn failed_scheduler_tick_is_atomic() {
        let mut vm = Bootstrap::new();
        let mut host = Host::default();
        host.install(&mut vm);
        let mut scheduler = Scheduler::default();
        let p = proto(
            vec![Constant::String("changed".into())],
            vec![word(2, 0, 1, 0), word(7, 0, 0, 0), word(27, 0, 1, 0)],
        );
        scheduler
            .threads
            .insert(1, Thread::new("first", p, &[]).unwrap());
        scheduler.threads.insert(
            2,
            Thread::new("unsupported", proto(vec![], vec![word(25, 0, 1, 1)]), &[]).unwrap(),
        );
        assert!(scheduler.tick(&mut vm, &mut host, 0.5, 100).is_err());
        assert!(!vm.globals.contains_key("changed"));
        assert_eq!(host.time, 0.0);
        assert_eq!(host.frame, 0);
        assert_eq!(scheduler.threads[&1].pc, 0);
    }
    #[test]
    fn infinite_loop_is_bounded() {
        let p = proto(vec![], vec![20 | (131070 << 6)]);
        let mut t = Thread::new("loop", p, &[]).unwrap();
        assert!(
            t.resume(&mut Bootstrap::new(), &mut Host::default(), 20)
                .unwrap_err()
                .contains("budget")
        );
        assert_eq!(t.instructions, 0);
    }
}
