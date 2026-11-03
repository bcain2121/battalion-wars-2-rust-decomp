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

pub const SHARED_SCRIPTS: [&str; 10] = [
    "GStringID",
    "Music",
    "Backgrounds",
    "SpriteTravel",
    "Unlocking",
    "NANDDialog",
    "YesNoDialog",
    "Campaign",
    "MPGlobal",
    "GlobeSelect",
];

#[derive(Clone)]
pub struct Runtime {
    vm: Bootstrap,
    host: Host,
    catalog: Arc<Catalog>,
    scripts: Arc<BTreeMap<String, Prototype>>,
    pages: BTreeMap<String, usize>,
    active_page: Option<String>,
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
            active_page: None,
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
    pub fn init_shared_scripts(&mut self, budget: usize) -> Result<()> {
        for name in SHARED_SCRIPTS {
            self.root(name, budget)?;
        }
        Ok(())
    }
    pub fn set_string_tables(
        &mut self,
        global_strings: Vec<String>,
        frontend_strings: Vec<String>,
    ) {
        if let Some(env) = self.host.frontend.as_mut() {
            env.set_string_tables(global_strings, frontend_strings);
        }
    }
    pub fn env(&self) -> Option<&Environment> {
        self.host.frontend.as_ref()
    }
    pub fn env_mut(&mut self) -> Option<&mut Environment> {
        self.host.frontend.as_mut()
    }
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub fn active_page(&self) -> Option<&str> {
        self.active_page.as_deref()
    }
    pub fn drawn_sprites(&self) -> &[(u64, f32, f32)] {
        self.host
            .frontend_sprites
            .as_ref()
            .map(|s| s.drawn_sprites())
            .unwrap_or(&[])
    }
    pub fn clear_drawn_sprites(&mut self) {
        if let Some(s) = self.host.frontend_sprites.as_mut() {
            s.clear_drawn_sprites();
        }
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
        if let Value::Native(native_name) = callable
            && let Some(env) = self.host.frontend.as_mut()
        {
            return env.call_with_vm(native_name, args, &mut self.vm);
        }
        let mut thread = Thread::from_callable(name, callable, args, &self.vm)?;
        match thread.resume(&mut self.vm, &mut self.host, budget)? {
            Status::Complete(values) => Ok(values),
            _ => Err(format!("Frontend callback {name} unexpectedly yielded")),
        }
    }
    pub fn resolve_string(&self, val: &Value) -> Option<String> {
        match val {
            Value::String(s) => Some(s.clone()),
            Value::Number(id) if id.is_finite() && *id >= 0.0 => self
                .host
                .frontend
                .as_ref()
                .map(|env| env.lookup_string(*id as usize)),
            _ => None,
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
    pub fn set_global(&mut self, name: &str, value: Value) {
        self.vm.globals.insert(name.into(), value);
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
    pub fn ensure_page_constructed(&mut self, name: &str, budget: usize) -> Result<()> {
        if !self.pages.contains_key(name) {
            self.construct_page(name, budget)?;
        }
        Ok(())
    }
    pub fn construct_all_pages(&mut self, budget: usize) -> Result<usize> {
        let page_names: Vec<String> = self.catalog.pages.keys().cloned().collect();
        for name in &page_names {
            self.ensure_page_constructed(name, budget)?;
        }
        Ok(self.pages.len())
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
    pub fn reset_page_stack(&mut self, page: &str, budget: usize) -> Result<()> {
        if let Some(prev) = self.active_page.take() {
            let _ = self.call_page_method(&prev, "Close", &[], budget);
        }
        self.global_call("ResetPageStack", &[Value::String(page.into())], budget)?;
        Ok(())
    }
    pub fn push_page_stack(&mut self, page: &str, budget: usize) -> Result<()> {
        self.global_call("PushPageStack", &[Value::String(page.into())], budget)?;
        Ok(())
    }
    pub fn pop_page_stack(&mut self, budget: usize) -> Result<()> {
        self.global_call("PopPageStack", &[], budget)?;
        Ok(())
    }
    /// Steps the authored `FrontendBW2` page-stack loop (`OpenPage`, `UpdatePage`, `RenderPage`, `ClosePage`)
    /// for one frame and returns the currently active page (if any).
    pub fn step_page_stack(&mut self, dt: f64, budget: usize) -> Result<Option<String>> {
        self.host.time += dt;
        self.host.no_pause_time += dt;
        self.host.frame += 1;
        if let Some(env) = self.host.frontend.as_mut() {
            env.elapsed_time += dt as f32;
            if env.video_mode != 0 {
                env.video_frame_num = env.video_frame_num.saturating_add(1);
            }
        }
        self.clear_drawn_sprites();

        let state = self.stack_state()?;
        if let Some(current) = self.active_page.clone()
            && (state.page_done
                || state.next.as_deref() != Some(current.as_str())
                || self.global("FrontendDone") == Some(&Value::Boolean(true)))
        {
            self.call_page_method(&current, "Close", &[], budget)?;
            self.active_page = None;
        }

        if self.global("FrontendDone") == Some(&Value::Boolean(true)) {
            if let Some(Value::Number(level)) = self.global("SelectedLevel").cloned()
                && let Some(env) = self.host.frontend.as_mut()
                && env.launched_level.is_none()
            {
                env.launched_level = Some(level as i32);
            }
            return Ok(None);
        }

        if self.active_page.is_none() {
            let state = self.stack_state()?;
            let Some(next_page) = state.next.or(state.top) else {
                return Ok(None);
            };
            self.ensure_page_constructed(&next_page, budget)?;
            self.call_page_method(&next_page, "Open", &[], budget)?;
            self.vm
                .globals
                .insert("PageDone".into(), Value::Boolean(false));
            self.active_page = Some(next_page);
        }

        if let Some(current) = self.active_page.clone() {
            self.call_page_method(&current, "Update", &[], budget)?;
            if matches!(self.page_member(&current, "Render"), Ok(Value::Closure(_))) {
                self.call_page_method(&current, "Render", &[], budget)?;
            }
            if matches!(
                self.page_member(&current, "renderBeziers"),
                Ok(Value::Closure(_))
            ) {
                self.call_page_method(&current, "renderBeziers", &[], budget)?;
            }
        }
        Ok(self.active_page.clone())
    }
    pub fn active_sprite_count(&self) -> usize {
        self.host
            .frontend_sprites
            .as_ref()
            .map_or(0, |s| s.active_count())
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
