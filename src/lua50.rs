//! Bounded original Lua 5.0 reader and deliberately restricted bootstrap evaluator.
//! This evaluator is bootstrap-only; mission bodies/call stacks live in mission_vm.
//! Captures and metamethods remain unsupported. Bootstrap's only host capability
//! resolves an existing object ID.
use crate::{ObjectGraph, ObjectId, Result};
use std::collections::BTreeMap;
#[derive(Debug, Clone, PartialEq)]
pub enum Constant {
    Nil,
    Number(f64),
    String(String),
}
#[derive(Debug, Clone)]
pub struct Prototype {
    pub upvalues: u8,
    pub parameters: u8,
    pub vararg: bool,
    pub stack: u8,
    pub constants: Vec<Constant>,
    pub children: Vec<Prototype>,
    pub code: Vec<u32>,
}
struct Reader<'a> {
    bytes: &'a [u8],
    cursor: usize,
    prototypes: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.cursor.checked_add(n).ok_or("Lua size overflow")?;
        let b = self
            .bytes
            .get(self.cursor..end)
            .ok_or("Truncated Lua 5.0 chunk")?;
        self.cursor = end;
        Ok(b)
    }
    fn int(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn count(&mut self, width: usize) -> Result<usize> {
        let n = self.int()? as usize;
        if n > 1000000 || n > self.bytes.len().saturating_sub(self.cursor) / width {
            return Err("Lua count limit".into());
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<Option<String>> {
        let n = self.int()? as usize;
        if n == 0 {
            return Ok(None);
        }
        let b = self.take(n)?;
        if b.last() != Some(&0) {
            return Err("Unterminated Lua string".into());
        }
        Ok(Some(
            std::str::from_utf8(&b[..b.len() - 1])
                .map_err(|e| e.to_string())?
                .into(),
        ))
    }
    fn proto(&mut self, depth: usize) -> Result<Prototype> {
        self.prototypes += 1;
        if depth > 100 || self.prototypes > 10000 {
            return Err("Lua prototype limit".into());
        }
        self.string()?;
        self.int()?;
        let h = self.take(4)?;
        let upvalues = h[0];
        let parameters = h[1];
        let vararg = h[2] != 0;
        let stack = h[3];
        if h[2] > 1 || parameters > stack || stack == 0 || stack > 250 {
            return Err("Invalid Lua stack size".into());
        }
        let lines = self.count(4)?;
        self.take(lines * 4)?;
        let locals = self.count(12)?;
        for _ in 0..locals {
            self.string()?;
            self.take(8)?;
        }
        let debug_upvalues = self.count(4)?;
        if debug_upvalues != 0 && debug_upvalues != upvalues as usize {
            return Err("Upvalue debug count mismatch".into());
        }
        for _ in 0..debug_upvalues {
            self.string()?;
        }
        let n = self.count(1)?;
        let mut constants = Vec::new();
        for _ in 0..n {
            let kind = self.take(1)?[0];
            constants.push(match kind {
                0 => Constant::Nil,
                3 => {
                    let n = f64::from_le_bytes(self.take(8)?.try_into().unwrap());
                    if !n.is_finite() {
                        return Err("Nonfinite Lua constant".into());
                    }
                    Constant::Number(n)
                }
                4 => Constant::String(self.string()?.ok_or("Null string constant")?),
                _ => return Err("Unsupported Lua constant type".into()),
            });
        }
        let n = self.count(1)?;
        let mut children = Vec::new();
        for _ in 0..n {
            children.push(self.proto(depth + 1)?);
        }
        let n = self.count(4)?;
        if lines != 0 && lines != n {
            return Err("Lua line/code size mismatch".into());
        }
        let mut code = Vec::new();
        for _ in 0..n {
            let word = self.int()?;
            let op = word & 63;
            let bx = ((word >> 6) & 262143) as usize;
            if op > 34 {
                return Err("Unknown Lua 5.0 opcode".into());
            }
            if matches!(op, 1 | 5 | 7) && bx >= constants.len() {
                return Err("Lua constant index out of range".into());
            }
            if op == 34 && bx >= children.len() {
                return Err("Lua child index out of range".into());
            }
            code.push(word);
        }
        Ok(Prototype {
            upvalues,
            parameters,
            vararg,
            stack,
            constants,
            children,
            code,
        })
    }
}
pub fn parse(bytes: &[u8]) -> Result<Prototype> {
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("Lua chunk limit".into());
    }
    let mut r = Reader {
        bytes,
        cursor: 0,
        prototypes: 0,
    };
    if r.take(14)? != b"\x1bLua\x50\x01\x04\x04\x04\x06\x08\x09\x09\x08" {
        return Err("Unsupported Lua 5.0 layout".into());
    }
    let sentinel = f64::from_le_bytes(r.take(8)?.try_into().unwrap());
    if !sentinel.is_finite() || (sentinel - 31415926.535897932).abs() > 0.001 {
        return Err("Lua numeric sentinel mismatch".into());
    }
    let prototype = r.proto(0)?;
    if r.cursor != bytes.len() {
        return Err("Trailing Lua chunk data".into());
    }
    Ok(prototype)
}
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Boolean(bool),
    Native(String),
    Enum(String),
    Number(f64),
    String(String),
    Table(usize),
    Object(ObjectId),
    /// VM-scoped opaque native identity, distinct from authored object references.
    /// General userdata-key/metatable behavior is not implemented.
    LightUserdata(u64),
    ReflectionResolver,
    Closure(usize),
    Function {
        script: String,
        child: usize,
    },
}
/// Captures reference transaction-owned cells, preserving aliasing without Rc clone leaks.
#[derive(Debug, Clone)]
pub struct Closure {
    pub prototype: Prototype,
    pub upvalues: Vec<usize>,
}
#[derive(Default, Clone)]
pub struct Bootstrap {
    pub globals: BTreeMap<String, Value>,
    pub tables: Vec<BTreeMap<String, Value>>,
    /// Integral numeric keys remain distinct from string keys on the same table identity.
    pub numeric_tables: BTreeMap<usize, BTreeMap<i64, Value>>,
    pub functions: BTreeMap<(String, usize), Prototype>,
    pub closures: Vec<Closure>,
    pub upvalues: Vec<Value>,
    pub registrations: usize,
    pub instructions: usize,
}
fn constant(p: &Prototype, index: usize) -> Result<Value> {
    Ok(match p.constants.get(index).ok_or("Missing constant")? {
        Constant::Nil => Value::Nil,
        Constant::Number(n) => Value::Number(*n),
        Constant::String(s) => Value::String(s.clone()),
    })
}
fn key(p: &Prototype, index: usize) -> Result<&str> {
    match p.constants.get(index) {
        Some(Constant::String(s)) => Ok(s),
        _ => Err("Global name is not a string".into()),
    }
}
fn rk(p: &Prototype, r: &[Value], index: usize) -> Result<Value> {
    if index >= 250 {
        constant(p, index - 250)
    } else {
        r.get(index)
            .cloned()
            .ok_or("Register index out of range".into())
    }
}
fn integer_key(value: f64) -> Result<i64> {
    if !value.is_finite() || value.fract() != 0.0 || value.abs() > 9_007_199_254_740_991.0 {
        return Err("Only finite safe-integer numeric table keys supported".into());
    }
    Ok(value as i64)
}
impl Bootstrap {
    pub fn table_get(&self, table: usize, key: &Value) -> Result<Value> {
        let strings = self.tables.get(table).ok_or("Missing table")?;
        let value = match key {
            Value::String(key) => strings.get(key),
            Value::Number(n) => {
                let key = integer_key(*n)?;
                self.numeric_tables.get(&table).and_then(|t| t.get(&key))
            }
            _ => return Err("Unsupported Lua table key type".into()),
        };
        Ok(value.cloned().unwrap_or(Value::Nil))
    }
    pub fn table_set(&mut self, table: usize, key: Value, value: Value) -> Result<()> {
        let previous = self.table_get(table, &key)?;
        let count =
            self.tables[table].len() + self.numeric_tables.get(&table).map_or(0, BTreeMap::len);
        if count >= 10000 && previous == Value::Nil && value != Value::Nil {
            return Err("Table entry limit".into());
        }
        match key {
            Value::String(key) => {
                let target = &mut self.tables[table];
                if value == Value::Nil {
                    target.remove(&key);
                } else {
                    target.insert(key, value);
                }
            }
            Value::Number(n) => {
                let key = integer_key(n)?;
                if value == Value::Nil {
                    if let Some(target) = self.numeric_tables.get_mut(&table) {
                        target.remove(&key);
                    }
                } else {
                    self.numeric_tables
                        .entry(table)
                        .or_default()
                        .insert(key, value);
                }
            }
            _ => return Err("Unsupported Lua table key type".into()),
        }
        Ok(())
    }
    pub fn new() -> Self {
        let mut vm = Self::default();
        vm.globals
            .insert("RegisterReflectionId".into(), Value::ReflectionResolver);
        vm
    }
    /// Transactional initialization: unsupported instructions never leave partial globals.
    pub fn execute(
        &mut self,
        name: &str,
        p: &Prototype,
        graph: &ObjectGraph,
        budget: usize,
    ) -> Result<()> {
        let mut staged = self.clone();
        staged.run(name, p, graph, budget)?;
        *self = staged;
        Ok(())
    }
    fn run(&mut self, name: &str, p: &Prototype, graph: &ObjectGraph, budget: usize) -> Result<()> {
        if p.upvalues != 0 {
            return Err("Bootstrap root captures unsupported".into());
        }
        let mut regs = vec![Value::Nil; p.stack as usize];
        let mut pc = 0usize;
        for _ in 0..budget {
            let word = *p.code.get(pc).ok_or("Bootstrap PC out of range")?;
            pc += 1;
            self.instructions += 1;
            let op = word & 63;
            let a = (word >> 24) as usize;
            let b = ((word >> 15) & 511) as usize;
            let c = ((word >> 6) & 511) as usize;
            let bx = ((word >> 6) & 262143) as usize;
            if a >= regs.len() && !matches!(op, 20 | 21 | 27) {
                return Err("Bootstrap destination register out of range".into());
            }
            match op {
                0 => regs[a] = regs.get(b).cloned().ok_or("MOVE register out of range")?,
                1 => regs[a] = constant(p, bx)?,
                5 => regs[a] = self.globals.get(key(p, bx)?).cloned().unwrap_or(Value::Nil),
                7 => {
                    if self.globals.len() > 10000 {
                        return Err("Global budget exceeded".into());
                    }
                    self.globals.insert(key(p, bx)?.into(), regs[a].clone());
                }
                9 => {
                    let Value::Table(table) = regs[a] else {
                        return Err("SETTABLE target is not a table".into());
                    };
                    let key = rk(p, &regs, b)?;
                    let value = rk(p, &regs, c)?;
                    self.table_set(table, key, value)?;
                }
                10 => {
                    if self.tables.len() >= 4096 {
                        return Err("Table budget".into());
                    }
                    regs[a] = Value::Table(self.tables.len());
                    self.tables.push(BTreeMap::new());
                }
                20 => {
                    pc = pc
                        .checked_add_signed(bx as isize - 131071)
                        .ok_or("Invalid jump")?;
                }
                21 => {
                    if a > 1 {
                        return Err("Invalid EQ condition flag".into());
                    }
                    if (rk(p, &regs, b)? == rk(p, &regs, c)?) != (a != 0) {
                        pc += 1;
                    }
                }
                25 => {
                    if regs[a] != Value::ReflectionResolver || b != 2 || c != 2 {
                        return Err("Bootstrap host call rejected".into());
                    }
                    let Some(Value::String(id)) = regs.get(a + 1) else {
                        return Err("Reflection ID must be a string".into());
                    };
                    if id.is_empty() || !id.bytes().all(|b| b.is_ascii_digit()) {
                        return Err("Invalid decimal reflection ID".into());
                    }
                    let id = ObjectId(id.parse::<u64>().map_err(|e| e.to_string())?);
                    if id.0 == 0 || !graph.objects.contains_key(&id) {
                        return Err(format!("Unknown reflection ID {}", id.0));
                    }
                    regs[a] = Value::Object(id);
                    self.registrations += 1;
                }
                27 => {
                    if b != 1 {
                        return Err("Bootstrap return values unsupported".into());
                    }
                    return Ok(());
                }
                34 => {
                    if p.children.get(bx).ok_or("Missing function")?.upvalues != 0 {
                        return Err("Captured closures unsupported".into());
                    }
                    self.functions
                        .insert((name.into(), bx), p.children[bx].clone());
                    regs[a] = Value::Function {
                        script: name.into(),
                        child: bx,
                    };
                }
                _ => return Err(format!("Unsupported bootstrap opcode {op} at {}", pc - 1)),
            }
        }
        Err("Bootstrap instruction budget exhausted".into())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn instruction(op: u32, a: u32, b: u32, c: u32) -> u32 {
        op | (a << 24) | (b << 15) | (c << 6)
    }
    #[test]
    fn bounded_loop() {
        let p = Prototype {
            parameters: 0,
            vararg: false,
            upvalues: 0,
            stack: 2,
            constants: vec![],
            children: vec![],
            code: vec![20 | (131070 << 6)],
        };
        let mut vm = Bootstrap::new();
        assert!(
            vm.execute("loop", &p, &ObjectGraph::default(), 10)
                .unwrap_err()
                .contains("budget")
        );
        assert_eq!(vm.instructions, 0);
    }
    #[test]
    fn tables_and_constants_use_250_offset() {
        let p = Prototype {
            parameters: 0,
            vararg: false,
            upvalues: 0,
            stack: 2,
            constants: vec![
                Constant::String("name".into()),
                Constant::Number(7.0),
                Constant::String("T".into()),
            ],
            children: vec![],
            code: vec![
                instruction(10, 0, 0, 0),
                instruction(9, 0, 250, 251),
                7 | (2 << 6),
                instruction(27, 0, 1, 0),
            ],
        };
        let mut vm = Bootstrap::new();
        vm.execute("fixture", &p, &ObjectGraph::default(), 20)
            .unwrap();
        assert_eq!(vm.tables[0]["name"], Value::Number(7.0));
        assert_eq!(vm.globals["T"], Value::Table(0));
    }
    #[test]
    fn unknown_host_is_rejected() {
        let p = Prototype {
            parameters: 0,
            vararg: false,
            upvalues: 0,
            stack: 2,
            constants: vec![],
            children: vec![],
            code: vec![instruction(25, 0, 2, 2)],
        };
        assert!(
            Bootstrap::new()
                .execute("bad", &p, &ObjectGraph::default(), 10)
                .is_err()
        );
    }
    fn binary_fixture() -> Vec<u8> {
        let mut b = b"\x1bLua\x50\x01\x04\x04\x04\x06\x08\x09\x09\x08".to_vec();
        b.extend(31415926.535897932_f64.to_le_bytes());
        b.extend([0; 8]);
        b.extend([0, 0, 0, 2]);
        b.extend([0; 20]);
        b.extend(1u32.to_le_bytes());
        b.extend(instruction(27, 0, 1, 0).to_le_bytes());
        b
    }
    #[test]
    fn binary_roundtrip_and_truncation() {
        let b = binary_fixture();
        let p = parse(&b).unwrap();
        Bootstrap::new()
            .execute("empty", &p, &ObjectGraph::default(), 10)
            .unwrap();
        for end in 0..b.len() {
            assert!(parse(&b[..end]).is_err());
        }
        let mut trailing = b.clone();
        trailing.push(0);
        assert!(parse(&trailing).is_err());
    }
    #[test]
    fn invalid_header() {
        assert!(parse(&[0; 22]).is_err());
    }
}
