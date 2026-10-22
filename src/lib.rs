//! Original level/preload data model. No gameplay or rendering assumptions.
pub mod animation;
pub mod archive;
#[cfg(feature = "viewer")]
pub mod autopilot;
pub mod battle;
#[cfg(feature = "viewer")]
pub mod camera_collision;
#[cfg(feature = "viewer")]
pub mod controls;
pub mod custom_terrain;
pub mod dsp_audio;
#[cfg(feature = "viewer")]
pub mod first_mission;
pub mod frontend;
pub mod frontend_environment;
pub mod frontend_runtime;
pub mod frontend_sprites;
pub mod frontend_stack;
pub mod level_catalog;
pub mod localization;
pub mod lua50;
pub mod mission_air;
pub mod mission_audio;
pub mod mission_bomber;
pub mod mission_camera;
pub mod mission_combat;
pub mod mission_controls;
pub mod mission_damage;
pub mod mission_destruction;
pub mod mission_effect_chain;
pub mod mission_effect_light;
pub mod mission_effect_sound;
pub mod mission_guidance;
pub mod mission_hud;
pub mod mission_input;
pub mod mission_lightning;
#[cfg(feature = "viewer")]
pub mod mission_lightning_render;
pub mod mission_messages;
pub mod mission_missile;
pub mod mission_music;
pub mod mission_pickup;
pub mod mission_program;
pub mod mission_random;
pub mod mission_rewrite;
#[cfg(feature = "viewer")]
pub mod mission_rewrite_scene;
pub mod mission_spatial_cell;
pub mod mission_spatial_grid;
pub mod mission_spatial_removal;
pub mod mission_tutorial;
pub mod mission_vm;
pub mod mission_waypoints;
pub mod mission_world;
pub mod model;
#[cfg(feature = "viewer")]
pub mod model_geometry;
pub mod model_node_kind;
#[cfg(feature = "viewer")]
pub mod model_render;
pub mod movie;
pub mod navigation;
pub mod original_scene;
pub mod scenario;
#[cfg(feature = "viewer")]
pub mod skinning;
pub mod skirmish;
pub mod sound_samples;
pub mod terrain;
#[cfg(feature = "viewer")]
pub mod terrain_render;
pub mod texture;
pub mod unit_animation;
pub mod unit_collision;
#[cfg(feature = "viewer")]
pub mod vehicle_boxes;
pub mod vehicle_profile;
#[cfg(feature = "viewer")]
pub mod vehicle_wheels;
use serde::Serialize;
use std::collections::BTreeMap;

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_XML_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct ObjectId(pub u64);

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub kind: String,
    pub attributes: BTreeMap<String, String>,
    pub items: Vec<String>,
    /// Exact source fragment preserves unknown tags, attributes and nested content.
    pub original_xml: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Object {
    pub id: ObjectId,
    pub kind: String,
    pub attributes: BTreeMap<String, String>,
    pub fields: Vec<Field>,
}

#[derive(Debug, Default)]
pub struct ObjectGraph {
    pub objects: BTreeMap<ObjectId, Object>,
}

#[derive(Debug, Serialize)]
pub struct Reference {
    pub source: ObjectId,
    pub field: String,
    pub target: ObjectId,
}

#[derive(Debug, Serialize)]
pub struct Summary {
    pub unique_objects: usize,
    pub object_types: BTreeMap<String, usize>,
    pub nonzero_references: usize,
    pub unresolved_references: Vec<Reference>,
}

fn id(text: &str) -> Result<ObjectId> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("Invalid decimal object ID: {text:?}"));
    }
    text.parse::<u64>().map(ObjectId).map_err(|e| e.to_string())
}

pub fn parse_objects(xml: &str) -> Result<Vec<Object>> {
    if xml.len() > MAX_XML_BYTES {
        return Err("XML size limit exceeded".into());
    }
    let doc = roxmltree::Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 2_000_000,
            ..Default::default()
        },
    )
    .map_err(|e| e.to_string())?;
    let root = doc.root_element();
    if root.tag_name().name() != "Instances" {
        return Err("Expected Instances root".into());
    }
    let mut objects = Vec::new();
    for node in root.children().filter(|n| n.is_element()) {
        if node.tag_name().name() == "Hints" {
            continue;
        }
        if node.tag_name().name() != "Object" {
            return Err("Unknown Instances child".into());
        }
        let object_id = id(node.attribute("id").ok_or("Object missing id")?)?;
        if object_id.0 == 0 {
            return Err("Object ID zero is reserved for null".into());
        }
        let kind = node
            .attribute("type")
            .filter(|s| !s.is_empty())
            .ok_or("Object missing type")?
            .to_owned();
        let fields = node
            .children()
            .filter(|n| n.is_element())
            .map(|field| Field {
                kind: field.tag_name().name().to_owned(),
                attributes: field
                    .attributes()
                    .map(|a| (a.name().into(), a.value().into()))
                    .collect(),
                items: field
                    .children()
                    .filter(|n| n.has_tag_name("Item"))
                    .map(|n| {
                        n.children()
                            .filter(|c| c.is_text())
                            .filter_map(|c| c.text())
                            .collect::<String>()
                    })
                    .collect(),
                original_xml: xml[field.range()].to_owned(),
            })
            .collect();
        objects.push(Object {
            id: object_id,
            kind,
            attributes: node
                .attributes()
                .map(|a| (a.name().into(), a.value().into()))
                .collect(),
            fields,
        });
    }
    Ok(objects)
}

impl ObjectGraph {
    /// Fail atomically on duplicate IDs; do not guess original merge precedence.
    pub fn merge(&mut self, objects: Vec<Object>) -> Result<()> {
        let mut staged = BTreeMap::new();
        for object in objects {
            let key = object.id;
            if self.objects.contains_key(&key) || staged.contains_key(&key) {
                return Err(format!("Duplicate object ID {}", key.0));
            }
            staged.insert(key, object);
        }
        self.objects.extend(staged);
        Ok(())
    }

    pub fn merge_dedup(&mut self, objects: Vec<Object>) -> Result<()> {
        for object in objects {
            self.objects.entry(object.id).or_insert(object);
        }
        Ok(())
    }

    pub fn summary(&self) -> Result<Summary> {
        let mut result = Summary {
            unique_objects: self.objects.len(),
            object_types: BTreeMap::new(),
            nonzero_references: 0,
            unresolved_references: Vec::new(),
        };
        for object in self.objects.values() {
            *result.object_types.entry(object.kind.clone()).or_default() += 1;
            for field in &object.fields {
                if !matches!(field.kind.as_str(), "Pointer" | "Resource") {
                    continue;
                }
                for value in &field.items {
                    let target = id(value.trim())?;
                    if target.0 == 0 {
                        continue;
                    }
                    result.nonzero_references += 1;
                    if !self.objects.contains_key(&target) {
                        result.unresolved_references.push(Reference {
                            source: object.id,
                            field: field.attributes.get("name").cloned().unwrap_or_default(),
                            target,
                        });
                    }
                }
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> &'static str {
        r#"<Instances><Object id="1" type="Unit"><Pointer name="base"><Item>2</Item><Item>0</Item></Pointer><Future strange="yes"><Item>A &amp; B</Item><Item/></Future></Object></Instances>"#
    }
    #[test]
    fn preserves_unknown_fields_and_empty_items() {
        let parsed = parse_objects(sample()).unwrap();
        assert_eq!(parsed[0].fields[1].items, ["A & B", ""]);
        assert!(parsed[0].fields[1].original_xml.contains("strange=\"yes\""));
    }
    #[test]
    fn resolves_after_merge() {
        let mut graph = ObjectGraph::default();
        graph.merge(parse_objects(sample()).unwrap()).unwrap();
        assert_eq!(graph.summary().unwrap().unresolved_references.len(), 1);
        graph
            .merge(parse_objects(r#"<Instances><Object id="2" type="Base"/></Instances>"#).unwrap())
            .unwrap();
        let s = graph.summary().unwrap();
        assert_eq!(s.nonzero_references, 1);
        assert!(s.unresolved_references.is_empty());
    }
    #[test]
    fn duplicate_merge_is_atomic() {
        let mut graph = ObjectGraph::default();
        graph.merge(parse_objects(sample()).unwrap()).unwrap();
        let data = r#"<Instances><Object id="3" type="X"/><Object id="1" type="X"/></Instances>"#;
        assert!(graph.merge(parse_objects(data).unwrap()).is_err());
        assert_eq!(graph.objects.len(), 1);
    }
    #[test]
    fn rejects_dtd_and_malformed_input() {
        for xml in [
            "<Instances>",
            "<Wrong/>",
            "<!DOCTYPE Instances><Instances/>",
            r#"<Instances><Object id="-1" type="X"/></Instances>"#,
            r#"<Instances><Object id="0" type="X"/></Instances>"#,
        ] {
            assert!(parse_objects(xml).is_err(), "{xml}");
        }
    }
    #[test]
    fn rejects_malformed_reference() {
        let mut graph = ObjectGraph::default();
        graph
            .merge(parse_objects(&sample().replace("<Item>2</Item>", "<Item>oops</Item>")).unwrap())
            .unwrap();
        assert!(graph.summary().is_err());
    }
}
