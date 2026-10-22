//! Executes the original PageStack Lua resource. Does not open/render pages or skip movies.
use crate::{
    Result,
    archive::{chunks, named_payload},
    frontend::Catalog,
    lua50::{self, Bootstrap, Value},
    mission_vm::{Host, Status, Thread},
};
#[derive(Clone)]
pub struct PageStack {
    vm: Bootstrap,
    host: Host,
}
#[derive(Debug, Clone)]
pub enum Command {
    Push(String),
    Pop,
    Replace(String),
    Reset(String),
}
#[derive(Debug, PartialEq, serde::Serialize)]
pub struct State {
    pub top: Option<String>,
    pub next: Option<String>,
    pub page_done: bool,
}
fn page(value: Value) -> Result<Option<String>> {
    match value {
        Value::Nil => Ok(None),
        Value::String(s) => Ok(Some(s)),
        Value::Number(0.0) => Ok(None),
        _ => Err("Unsupported frontend page value".into()),
    }
}
impl PageStack {
    pub fn from_archive(archive: &[u8]) -> Result<Self> {
        let mut found = None;
        for chunk in chunks(archive)? {
            if chunk.tag != *b"PRCS" {
                continue;
            }
            let (name, bytes) = named_payload(chunk.payload)?;
            if name == "PageStack" {
                if found.is_some() {
                    return Err("Duplicate PageStack script".into());
                }
                found = Some(lua50::parse(bytes)?);
            }
        }
        let mut runtime = Self {
            vm: Bootstrap::new(),
            host: Host::default(),
        };
        let mut root = Thread::new(
            "PageStack root",
            found.ok_or("PageStack script missing")?,
            &[],
        )?;
        if !matches!(
            root.resume(&mut runtime.vm, &mut runtime.host, 100000)?,
            Status::Complete(_)
        ) {
            return Err("PageStack root unexpectedly yielded".into());
        }
        runtime.state()?;
        Ok(runtime)
    }
    fn call(&mut self, name: &str, args: &[Value], budget: usize) -> Result<Vec<Value>> {
        let callable = self
            .vm
            .globals
            .get(name)
            .ok_or_else(|| format!("Missing frontend function {name}"))?;
        let mut thread = Thread::from_callable(name, callable, args, &self.vm)?;
        match thread.resume(&mut self.vm, &mut self.host, budget)? {
            Status::Complete(values) => Ok(values),
            _ => Err("Frontend stack callback unexpectedly yielded".into()),
        }
    }
    pub fn state(&self) -> Result<State> {
        let mut probe = self.clone();
        let values = probe.call("GetPageAtTop", &[], 1000)?;
        let [top] = values.as_slice() else {
            return Err("Frontend getter must return one value".into());
        };
        let next = probe
            .vm
            .globals
            .get("NextPage")
            .ok_or("Missing NextPage")?
            .clone();
        let Some(Value::Boolean(page_done)) = probe.vm.globals.get("PageDone") else {
            return Err("Missing boolean PageDone".into());
        };
        Ok(State {
            top: page(top.clone())?,
            next: page(next)?,
            page_done: *page_done,
        })
    }
    /// Whole callback plus output validation commits together. No page lifecycle is simulated.
    pub fn apply(&mut self, catalog: &Catalog, command: Command, budget: usize) -> Result<State> {
        let (name, target) = match command {
            Command::Push(s) => ("PushPageStack", Some(s)),
            Command::Pop => ("PopPageStack", None),
            Command::Replace(s) => ("SetPageStack", Some(s)),
            Command::Reset(s) => ("ResetPageStack", Some(s)),
        };
        if let Some(s) = &target
            && !catalog.pages.contains_key(s)
        {
            return Err(format!("Unknown authored GUI page {s}"));
        }
        let args: Vec<_> = target.into_iter().map(Value::String).collect();
        let mut staged = self.clone();
        staged.call(name, &args, budget)?;
        let state = staged.state()?;
        for name in [&state.top, &state.next].into_iter().flatten() {
            if !catalog.pages.contains_key(name) {
                return Err(format!("Frontend produced unknown page {name}"));
            }
        }
        *self = staged;
        Ok(state)
    }
}
