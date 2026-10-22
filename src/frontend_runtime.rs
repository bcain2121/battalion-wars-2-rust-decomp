//! Shared original frontend Lua environment and authored page callbacks.
//! Construction/callback execution is not Open/Update/Render/Close lifecycle acceptance.
use crate::{
    ObjectGraph, Result,
    archive::{chunks, named_payload},
    frontend::Catalog,
    frontend_environment::Environment,
    frontend_stack::State,
    level_catalog,
    lua50::{self, Bootstrap, Prototype, Value},
    mission_vm::{Host, Status, Thread},
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone)]
pub struct Runtime {
    vm: Bootstrap,
    host: Host,
    catalog: Arc<Catalog>,
    scripts: Arc<BTreeMap<String, Prototype>>,
    pages: BTreeMap<String, usize>,
}
impl Runtime {
    pub fn load(
        graph: &ObjectGraph,
        archive: &[u8],
        levels: &level_catalog::Catalog,
        frame_rate: f32,
    ) -> Result<Self> {
        let mut scripts = BTreeMap::new();
        for chunk in chunks(archive)? {
            if chunk.tag == *b"PRCS" {
                let (name, bytes) = named_payload(chunk.payload)?;
                if scripts
                    .insert(name.to_owned(), lua50::parse(bytes)?)
                    .is_some()
                {
                    return Err(format!("Duplicate frontend script {name}"));
                }
            }
        }
        let mut runtime = Self {
            vm: Bootstrap::new(),
            host: Host {
                frontend: Some(Environment::new(levels, frame_rate)?),
                frontend_sprites: Some(crate::frontend_sprites::Sprites::from_graph(graph)?),
                ..Host::default()
            },
            catalog: Arc::new(Catalog::load(graph)?),
            scripts: Arc::new(scripts),
            pages: BTreeMap::new(),
        };
        let entity = runtime
            .scripts
            .get("EntityInitialise")
            .ok_or("Missing EntityInitialise")?;
        runtime
            .vm
            .execute("EntityInitialise", entity, graph, 100000)?;
        runtime.host.install(&mut runtime.vm);
        Environment::install(&mut runtime.vm)?;
        for name in ["PageStack", "Global"] {
            runtime.root(name, 100000)?;
        }
        Ok(runtime)
    }
    pub fn root(&mut self, name: &str, budget: usize) -> Result<()> {
        let p = self
            .scripts
            .get(name)
            .ok_or_else(|| format!("Missing frontend script {name}"))?
            .clone();
        let mut thread = Thread::new(name, p, &[])?;
        match thread.resume(&mut self.vm, &mut self.host, budget)? {
            Status::Complete(_) => Ok(()),
            _ => Err(format!("Frontend root {name} unexpectedly yielded")),
        }
    }
    fn call(
        &mut self,
        name: &str,
        callable: &Value,
        args: &[Value],
        budget: usize,
    ) -> Result<Vec<Value>> {
        let mut thread = Thread::from_callable(name, callable, args, &self.vm)?;
        match thread.resume(&mut self.vm, &mut self.host, budget)? {
            Status::Complete(values) => Ok(values),
            _ => Err(format!("Frontend callback {name} unexpectedly yielded")),
        }
    }
    pub fn global_call(&mut self, name: &str, args: &[Value], budget: usize) -> Result<Vec<Value>> {
        let callable = self
            .vm
            .globals
            .get(name)
            .ok_or_else(|| format!("Missing frontend function {name}"))?
            .clone();
        self.call(name, &callable, args, budget)
    }
    pub fn read_matrix4(&self, table_id: usize) -> Result<[[f32; 4]; 4]> {
        Environment::read_matrix4(&self.vm, table_id)
    }
    /// Runs the entire original constructor, retaining its shared closure/table identities.
    /// Does not claim the page has been opened or rendered.
    pub fn construct_page(&mut self, name: &str, budget: usize) -> Result<()> {
        if self.pages.contains_key(name) {
            return Err(format!("Page already constructed: {name}"));
        }
        let page = self
            .catalog
            .pages
            .get(name)
            .ok_or_else(|| format!("Unknown frontend page {name}"))?;
        let id = page.initializer;
        let script = page.script.clone();
        let mut staged = self.clone();
        staged.root(&script, budget)?;
        staged.global_call(&script, &[Value::Object(id)], budget)?;
        let values = staged.global_call(&format!("Get{script}Page"), &[], budget)?;
        let [Value::Table(table)] = values.as_slice() else {
            return Err("Page getter did not return one table".into());
        };
        if *table >= staged.vm.tables.len() {
            return Err("Invalid page table".into());
        }
        staged.pages.insert(name.into(), *table);
        *self = staged;
        Ok(())
    }
    pub fn page_member(&self, page: &str, member: &str) -> Result<Value> {
        let table = self
            .pages
            .get(page)
            .ok_or_else(|| format!("Page not constructed: {page}"))?;
        self.vm.table_get(*table, &Value::String(member.into()))
    }
    /// Callback plus shared stack/output validation commit together. Caller owns lifecycle.
    pub fn call_page_method(
        &mut self,
        page: &str,
        member: &str,
        args: &[Value],
        budget: usize,
    ) -> Result<Vec<Value>> {
        let callable = self.page_member(page, member)?;
        let mut staged = self.clone();
        let result = staged.call(&format!("{page}.{member}"), &callable, args, budget)?;
        staged.stack_state()?;
        *self = staged;
        Ok(result)
    }
    /// Executes the script part of an XML event, never a Rust-authored transition target.
    /// Caller still owns input, optional event sounds, and page opening/rendering.
    pub fn dispatch_page_callback(
        &mut self,
        page: &str,
        signal: &str,
        args: &[Value],
        budget: usize,
    ) -> Result<Vec<Value>> {
        let spec = self
            .catalog
            .pages
            .get(page)
            .ok_or("Unknown frontend page")?;
        let matches: Vec<_> = spec
            .events
            .iter()
            .flatten()
            .filter(|event| event.signal == signal)
            .collect();
        let [event] = matches.as_slice() else {
            return Err("Missing/ambiguous authored page event".into());
        };
        let callback = event.callback.clone().ok_or("Page event has no callback")?;
        self.call_page_method(page, &callback, args, budget)
    }
    pub fn sprite_state(
        &self,
        sprite: crate::ObjectId,
    ) -> Result<Option<crate::frontend_sprites::OpenState>> {
        self.host
            .frontend_sprites
            .as_ref()
            .ok_or("Frontend sprite backend not configured")?
            .state(sprite)
    }
    pub fn global(&self, name: &str) -> Option<&Value> {
        self.vm.globals.get(name)
    }
    pub fn stack_state(&self) -> Result<State> {
        let mut probe = self.clone();
        let values = probe.global_call("GetPageAtTop", &[], 1000)?;
        let [top] = values.as_slice() else {
            return Err("Invalid frontend stack getter result".into());
        };
        let convert = |value: &Value| -> Result<Option<String>> {
            match value {
                Value::Nil | Value::Number(0.0) => Ok(None),
                Value::String(name) if self.catalog.pages.contains_key(name) => {
                    Ok(Some(name.clone()))
                }
                _ => Err("Invalid/unknown page in frontend stack".into()),
            }
        };
        let next = self.vm.globals.get("NextPage").ok_or("Missing NextPage")?;
        let Some(Value::Boolean(page_done)) = self.vm.globals.get("PageDone") else {
            return Err("Missing PageDone".into());
        };
        Ok(State {
            top: convert(top)?,
            next: convert(next)?,
            page_done: *page_done,
        })
    }
}
