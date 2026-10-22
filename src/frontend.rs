//! Authored GUI structure, not a replacement menu state machine.
//! Retains null array slots, shared widget identities and unresolved dynamic fields.
use crate::{Field, Object, ObjectGraph, ObjectId, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Debug, Clone, Serialize)]
pub struct Event {
    pub id: ObjectId,
    pub signal: String,
    pub callback: Option<String>,
    pub blip: bool,
}
#[derive(Debug, Clone, Serialize)]
pub struct Widget {
    pub id: ObjectId,
    pub kind: String,
    pub children: Vec<Option<ObjectId>>,
    pub events: Vec<Option<Event>>,
    pub fields: Vec<Field>,
}
#[derive(Debug, Clone, Serialize)]
pub struct Page {
    pub id: ObjectId,
    pub initializer: ObjectId,
    pub script: String,
    pub children: Vec<Option<ObjectId>>,
    pub events: Vec<Option<Event>>,
    pub fields: Vec<Field>,
}
#[derive(Debug, Serialize)]
pub struct Catalog {
    pub pages: BTreeMap<String, Page>,
    pub widgets: BTreeMap<ObjectId, Widget>,
}
fn field<'a>(object: &'a Object, name: &str) -> Result<Option<&'a Field>> {
    let mut matches = object
        .fields
        .iter()
        .filter(|f| f.attributes.get("name").is_some_and(|n| n == name));
    let first = matches.next();
    if matches.next().is_some() {
        return Err(format!("Duplicate GUI field {name} on {}", object.id.0));
    }
    Ok(first)
}
fn scalar(object: &Object, name: &str) -> Result<String> {
    let f = field(object, name)?
        .ok_or_else(|| format!("Missing GUI field {name} on {}", object.id.0))?;
    if f.items.len() != 1 {
        return Err(format!("Non-scalar GUI field {name}"));
    }
    Ok(f.items[0].clone())
}
fn refs(object: &Object, name: &str) -> Result<Vec<Option<ObjectId>>> {
    let Some(f) = field(object, name)? else {
        return Ok(vec![]);
    };
    if !matches!(f.kind.as_str(), "Pointer" | "Resource") {
        return Err(format!("GUI field {name} is not a reference"));
    }
    let count = f
        .attributes
        .get("elements")
        .ok_or("GUI reference missing element count")?
        .parse::<usize>()
        .map_err(|_| "Invalid GUI element count")?;
    if count != f.items.len() || count > 4096 {
        return Err("GUI reference count mismatch or limit exceeded".into());
    }
    f.items
        .iter()
        .map(|s| {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Invalid GUI reference ID".into());
            }
            let id = s.parse::<u64>().map_err(|_| "GUI ID overflow")?;
            Ok((id != 0).then_some(ObjectId(id)))
        })
        .collect()
}
fn pointer(object: &Object, name: &str) -> Result<Option<ObjectId>> {
    let values = refs(object, name)?;
    match values.as_slice() {
        [] => Ok(None),
        [v] => Ok(*v),
        _ => Err(format!("Non-scalar GUI pointer {name}")),
    }
}
fn object<'a>(graph: &'a ObjectGraph, id: ObjectId, kind: &str) -> Result<&'a Object> {
    let o = graph
        .objects
        .get(&id)
        .ok_or_else(|| format!("Missing GUI object {}", id.0))?;
    if o.kind != kind {
        return Err(format!(
            "GUI object {} expected {kind}, found {}",
            id.0, o.kind
        ));
    }
    Ok(o)
}
fn events(graph: &ObjectGraph, owner: &Object) -> Result<Vec<Option<Event>>> {
    let Some(id) = pointer(owner, "mpEventHandler")? else {
        return Ok(vec![]);
    };
    let handler = object(graph, id, "cGUIEventHandler")?;
    refs(handler, "mpEventArray")?
        .into_iter()
        .map(|id| {
            let Some(id) = id else { return Ok(None) };
            let e = object(graph, id, "cGUIEvent")?;
            let callback = scalar(e, "mcString")?;
            let blip = match scalar(e, "meBlip")?.as_str() {
                "eTrue" => true,
                "eFalse" => false,
                _ => return Err("Unsupported GUI event blip enum".into()),
            };
            Ok(Some(Event {
                id,
                signal: scalar(e, "meEvent")?,
                callback: (!callback.is_empty()).then_some(callback),
                blip,
            }))
        })
        .collect()
}
impl Catalog {
    pub fn load(graph: &ObjectGraph) -> Result<Self> {
        let mut catalog = Self {
            pages: BTreeMap::new(),
            widgets: BTreeMap::new(),
        };
        for o in graph
            .objects
            .values()
            .filter(|o| o.kind.starts_with("cGUI") && o.kind.ends_with("Widget"))
        {
            catalog.widgets.insert(
                o.id,
                Widget {
                    id: o.id,
                    kind: o.kind.clone(),
                    children: refs(o, "mWidgetArray")?,
                    events: events(graph, o)?,
                    fields: o.fields.clone(),
                },
            );
        }
        for o in graph.objects.values().filter(|o| o.kind == "cGUIPage") {
            let init = object(
                graph,
                pointer(o, "mpScript")?.ok_or("GUI page has no script")?,
                "cInitialisationScriptEntity",
            )?;
            let resource = object(
                graph,
                pointer(init, "mpScript")?.ok_or("GUI initializer has no script")?,
                "cGameScriptResource",
            )?;
            let script = scalar(resource, "mName")?;
            if script.is_empty() {
                return Err("Empty GUI page script".into());
            }
            let page = Page {
                id: o.id,
                initializer: init.id,
                script: script.clone(),
                children: refs(o, "mWidgetArray")?,
                events: events(graph, o)?,
                fields: o.fields.clone(),
            };
            if catalog.pages.insert(script, page).is_some() {
                return Err("Duplicate GUI page script identity".into());
            }
        }
        if catalog.pages.is_empty() {
            return Err("No authored GUI pages".into());
        }
        // Validate all widgets, including disconnected definitions. Shared children are legal;
        // only ancestry cycles are rejected. Memoization bounds work on heavily shared DAGs.
        let mut done = BTreeSet::new();
        for id in catalog.widgets.keys() {
            catalog.visit(*id, &mut BTreeSet::new(), &mut done, 0)?;
        }
        for p in catalog.pages.values() {
            for id in p.children.iter().flatten() {
                if !catalog.widgets.contains_key(id) {
                    return Err(format!("Missing page widget {}", id.0));
                }
            }
        }
        Ok(catalog)
    }
    fn visit(
        &self,
        id: ObjectId,
        path: &mut BTreeSet<ObjectId>,
        done: &mut BTreeSet<ObjectId>,
        depth: usize,
    ) -> Result<()> {
        if depth > 128 {
            return Err("GUI widget nesting limit exceeded".into());
        }
        if done.contains(&id) {
            return Ok(());
        }
        if !path.insert(id) {
            return Err("GUI widget ancestry cycle".into());
        }
        let widget = self
            .widgets
            .get(&id)
            .ok_or_else(|| format!("Missing child widget {}", id.0))?;
        for child in widget.children.iter().flatten() {
            self.visit(*child, path, done, depth + 1)?
        }
        path.remove(&id);
        done.insert(id);
        Ok(())
    }
}
