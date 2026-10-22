//! Bounded OpenSprite table binding (0x800db804 / 0x80192eb4).
//! Does not render sprites or infer texture/animation playback.
use crate::{
    ObjectGraph, ObjectId, Result,
    lua50::{Bootstrap, Value},
};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OpenState {
    /// Raw sprite +0x5c value, reset by OpenSprite. Meaning not extrapolated.
    pub field_5c: f32,
    /// Raw sprite +0x70 byte, reset by OpenSprite.
    pub byte_70: u8,
}
#[derive(Clone, Debug, Default)]
pub struct Sprites {
    scripts: BTreeMap<ObjectId, Option<u64>>,
    identities: BTreeMap<ObjectId, u64>,
    states: BTreeMap<u64, Option<OpenState>>,
}
impl Sprites {
    /// Scope identities to this frontend VM. Neither native pointers nor authored IDs
    /// are used as runtime pointer identity. Clones preserve identity and isolate state.
    pub fn from_graph(graph: &ObjectGraph) -> Result<Self> {
        let mut out = Self::default();
        for object in graph.objects.values().filter(|o| o.kind == "cScriptSprite") {
            let fields: Vec<_> = object
                .fields
                .iter()
                .filter(|f| f.attributes.get("name").is_some_and(|n| n == "mSprite"))
                .collect();
            let [field] = fields.as_slice() else {
                return Err("Missing/ambiguous script sprite reference".into());
            };
            let [text] = field.items.as_slice() else {
                return Err("Invalid script sprite reference".into());
            };
            if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
                return Err("Invalid sprite ID".into());
            }
            let id = ObjectId(text.parse::<u64>().map_err(|_| "Sprite ID overflow")?);
            let identity = if id.0 == 0 {
                None
            } else {
                let sprite = graph
                    .objects
                    .get(&id)
                    .ok_or("Script sprite target missing")?;
                if sprite.kind != "cSprite" {
                    return Err("Script sprite target has wrong type".into());
                }
                let next = out.identities.len() as u64 + 1;
                let handle = *out.identities.entry(id).or_insert(next);
                out.states.entry(handle).or_insert(None);
                Some(handle)
            };
            out.scripts.insert(object.id, identity);
        }
        Ok(out)
    }
    /// Native CloseSprite clears only this table's cached pointer, not shared sprite state.
    pub fn close(&mut self, vm: &mut Bootstrap, args: &[Value]) -> Result<Vec<Value>> {
        let [Value::Table(table)] = args else {
            return Err("Unsupported CloseSprite signature".into());
        };
        let key = Value::String("objectPtr".into());
        if let Value::LightUserdata(handle) = vm.table_get(*table, &key)?
            && handle != 0
        {
            vm.table_set(*table, key, Value::LightUserdata(0))?;
        }
        Ok(vec![])
    }
    pub fn state(&self, sprite: ObjectId) -> Result<Option<OpenState>> {
        let handle = self
            .identities
            .get(&sprite)
            .ok_or("Unknown frontend sprite")?;
        Ok(self.states[handle])
    }
    /// Restricted numeric/object resource input. Native function/string-producing
    /// descriptors need separate Lua evaluation/coercion support and are rejected.
    pub fn open(&mut self, vm: &mut Bootstrap, args: &[Value]) -> Result<Vec<Value>> {
        let [Value::Table(table)] = args else {
            return Err("Unsupported OpenSprite signature".into());
        };
        let descriptor = vm.table_get(*table, &Value::Number(1.0))?;
        let id = match descriptor {
            Value::Object(id) => id,
            Value::Number(n)
                if n.is_finite() && n >= 0.0 && n <= u32::MAX as f64 && n.fract() == 0.0 =>
            {
                ObjectId(n as u64)
            }
            _ => return Err("OpenSprite descriptor evaluation not configured".into()),
        };
        if id.0 == 0 {
            return Ok(vec![]);
        }
        let key = Value::String("objectPtr".into());
        let cached = vm.table_get(*table, &key)?;
        if let Value::LightUserdata(handle) = cached
            && handle != 0
        {
            if !self.states.contains_key(&handle) {
                return Err("Unknown cached sprite identity".into());
            }
            return Ok(vec![]);
        }
        let handle = self
            .scripts
            .get(&id)
            .ok_or("OpenSprite resource not configured")?;
        if let Some(handle) = handle {
            // Validate/commit Lua binding before the infallible state write.
            vm.table_set(*table, key, Value::LightUserdata(*handle))?;
            self.states.insert(
                *handle,
                Some(OpenState {
                    field_5c: 0.0,
                    byte_70: 0,
                }),
            );
        }
        Ok(vec![])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cached_open_preserves_state_and_close_reopen_resets_it() {
        let mut sprites = Sprites::default();
        sprites.scripts.insert(ObjectId(10), Some(1));
        sprites.identities.insert(ObjectId(99), 1);
        let advanced = Some(OpenState {
            field_5c: 9.5,
            byte_70: 7,
        });
        sprites.states.insert(1, advanced);
        let mut vm = Bootstrap::new();
        vm.tables.push(Default::default());
        vm.table_set(0, Value::Number(1.0), Value::Object(ObjectId(10)))
            .unwrap();
        vm.table_set(
            0,
            Value::String("objectPtr".into()),
            Value::LightUserdata(1),
        )
        .unwrap();
        sprites.open(&mut vm, &[Value::Table(0)]).unwrap();
        assert_eq!(sprites.state(ObjectId(99)).unwrap(), advanced);
        sprites.close(&mut vm, &[Value::Table(0)]).unwrap();
        assert_eq!(sprites.state(ObjectId(99)).unwrap(), advanced);
        sprites.open(&mut vm, &[Value::Table(0)]).unwrap();
        assert_eq!(
            sprites.state(ObjectId(99)).unwrap(),
            Some(OpenState {
                field_5c: 0.0,
                byte_70: 0
            })
        );
    }
}
