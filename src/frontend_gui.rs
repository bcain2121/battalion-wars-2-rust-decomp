//! Authored `cGUIPage` / `cGUIWidget` and `sSpriteBasetype` screen-space canvas evaluator.
//!
//! Resolves the widget hierarchy (`cGUICustomWidget`, `cGUIButtonWidget`, `cGUITextBoxWidget`,
//! `cGUITextureWidget`, `cGUISpriteWidget`, `cGUIRectWidget`, `cGUIDialogBox*Widget`) and
//! `SpriteManager` (`cScriptSprite` -> `cSprite` -> `sSpriteBasetype`) against the live
//! `frontend_runtime::Runtime` Lua state (`FromLabel` / `FromFunction` / `StartWidget` / `StopWidget`).

use crate::{Object, ObjectGraph, ObjectId, Result, frontend_runtime::Runtime, lua50::Value};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum GuiQuadKind {
    Texture,
    Sprite,
    Button { selected: bool },
    TextBox { text: String },
    Rect,
    Model3D,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GuiCanvasQuad {
    pub widget_id: ObjectId,
    pub kind: GuiQuadKind,
    pub pos: [f32; 2],
    pub size: [f32; 2],
    pub rgba: [u8; 4],
    pub z_plane: u8,
    pub texture_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SpriteBaseDef {
    pub id: ObjectId,
    pub texture_name: Option<String>,
    pub tl_uv: [f32; 2],
    pub br_uv: [f32; 2],
    pub scale: [f32; 2],
    pub rgba: [u8; 4],
}

#[derive(Debug, Clone, Default)]
pub struct GuiScene {
    pub textures: BTreeMap<ObjectId, String>,
    pub sprite_bases: BTreeMap<ObjectId, SpriteBaseDef>,
    pub sprites: BTreeMap<ObjectId, ObjectId>,
    pub script_sprites: BTreeMap<u64, ObjectId>,
}

fn field_text<'a>(obj: &'a Object, name: &str) -> Option<&'a str> {
    obj.fields
        .iter()
        .find(|f| f.attributes.get("name").map(|s| s.as_str()) == Some(name))
        .and_then(|f| f.items.first())
        .map(|s| s.trim())
}

fn field_id(obj: &Object, name: &str) -> Option<ObjectId> {
    let text = field_text(obj, name)?;
    let id = text.parse::<u64>().ok()?;
    (id != 0).then_some(ObjectId(id))
}

fn field_ids(obj: &Object, name: &str) -> Vec<ObjectId> {
    obj.fields
        .iter()
        .find(|f| f.attributes.get("name").map(|s| s.as_str()) == Some(name))
        .map(|f| {
            f.items
                .iter()
                .filter_map(|s| s.trim().parse::<u64>().ok())
                .filter(|&id| id != 0)
                .map(ObjectId)
                .collect()
        })
        .unwrap_or_default()
}

fn parse_vec2(s: &str) -> Option<[f32; 2]> {
    let mut parts = s.split(',').map(|p| p.trim().parse::<f32>().ok());
    Some([parts.next()??, parts.next()??])
}

fn parse_rgba(s: &str) -> Option<[u8; 4]> {
    let mut parts = s.split(',').map(|p| {
        p.trim()
            .parse::<f32>()
            .ok()
            .map(|v| v.clamp(0.0, 255.0) as u8)
    });
    Some([
        parts.next()??,
        parts.next()??,
        parts.next()??,
        parts.next()??,
    ])
}

fn parse_z_plane(s: &str) -> u8 {
    s.strip_prefix("eGUIZPlane_")
        .and_then(|n| n.parse::<u8>().ok())
        .unwrap_or(15)
}

impl GuiScene {
    pub fn from_graph(graph: &ObjectGraph) -> Self {
        let mut scene = Self::default();
        for obj in graph.objects.values() {
            match obj.kind.as_str() {
                "cTextureResource" => {
                    if let Some(name) = field_text(obj, "mName")
                        && !name.is_empty()
                    {
                        scene.textures.insert(obj.id, name.to_owned());
                    }
                }
                "cSprite" => {
                    if let Some(base_id) = field_id(obj, "mBase") {
                        scene.sprites.insert(obj.id, base_id);
                    }
                }
                _ => {}
            }
        }
        for obj in graph.objects.values() {
            if obj.kind == "sSpriteBasetype" {
                let tex_id = field_id(obj, "texture");
                let texture_name = tex_id.and_then(|id| scene.textures.get(&id).cloned());
                let tl_uv = field_text(obj, "tlUV")
                    .and_then(parse_vec2)
                    .unwrap_or([0.0, 0.0]);
                let br_uv = field_text(obj, "brUV")
                    .and_then(parse_vec2)
                    .unwrap_or([64.0, 64.0]);
                let scale = field_text(obj, "scale")
                    .and_then(parse_vec2)
                    .unwrap_or([1.0, 1.0]);
                let rgba = field_text(obj, "colour")
                    .and_then(parse_rgba)
                    .unwrap_or([255, 255, 255, 255]);
                scene.sprite_bases.insert(
                    obj.id,
                    SpriteBaseDef {
                        id: obj.id,
                        texture_name,
                        tl_uv,
                        br_uv,
                        scale,
                        rgba,
                    },
                );
            }
        }
        // Match handle order in `frontend_sprites::Sprites::from_graph`
        let mut identities = BTreeMap::new();
        for obj in graph.objects.values().filter(|o| o.kind == "cScriptSprite") {
            if let Some(sprite_id) = field_id(obj, "mSprite") {
                let next = identities.len() as u64 + 1;
                let handle = *identities.entry(sprite_id).or_insert(next);
                scene.script_sprites.insert(handle, sprite_id);
            }
        }
        scene
    }

    pub fn evaluate_active_page_canvas(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
    ) -> Result<Vec<GuiCanvasQuad>> {
        let Some(page_name) = runtime.active_page().map(str::to_owned) else {
            return Ok(Vec::new());
        };
        self.evaluate_page_canvas(graph, runtime, &page_name)
    }

    pub fn evaluate_page_canvas(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
    ) -> Result<Vec<GuiCanvasQuad>> {
        let Some(page) = runtime.catalog().pages.get(page_name).cloned() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for child_id in page.children.iter().flatten() {
            self.eval_widget(
                graph,
                runtime,
                page_name,
                *child_id,
                [0.0, 0.0],
                [255, 255, 255, 255],
                0,
                &mut out,
            );
        }
        // Append any active `SpriteManager::DrawSprite` quads emitted during `RenderPage`
        for &(handle, x, y) in runtime.drawn_sprites() {
            if let Some(&sprite_id) = self.script_sprites.get(&handle)
                && let Some(&base_id) = self.sprites.get(&sprite_id)
                && let Some(base) = self.sprite_bases.get(&base_id)
            {
                let w = (base.br_uv[0] - base.tl_uv[0]).abs().max(16.0) * base.scale[0];
                let h = (base.br_uv[1] - base.tl_uv[1]).abs().max(16.0) * base.scale[1];
                out.push(GuiCanvasQuad {
                    widget_id: sprite_id,
                    kind: GuiQuadKind::Sprite,
                    pos: [x, y],
                    size: [w, h],
                    rgba: base.rgba,
                    z_plane: 31,
                    texture_name: base.texture_name.clone(),
                });
            }
        }
        out.sort_by_key(|q| q.z_plane);
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    fn eval_widget(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        widget_id: ObjectId,
        parent_pos: [f32; 2],
        parent_rgba: [u8; 4],
        depth: usize,
        out: &mut Vec<GuiCanvasQuad>,
    ) {
        if depth > 32 {
            return;
        }
        let Some(obj) = graph.objects.get(&widget_id) else {
            return;
        };
        if !self.eval_active(graph, runtime, page_name, obj) {
            return;
        }
        let local_pos = field_id(obj, "mpPos")
            .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
            .unwrap_or([0.0, 0.0]);
        let pos = [parent_pos[0] + local_pos[0], parent_pos[1] + local_pos[1]];
        let local_rgba = field_id(obj, "mpColour")
            .and_then(|id| self.eval_rgba(graph, runtime, page_name, id))
            .unwrap_or([255, 255, 255, 255]);
        let rgba = [
            ((parent_rgba[0] as u16 * local_rgba[0] as u16) / 255) as u8,
            ((parent_rgba[1] as u16 * local_rgba[1] as u16) / 255) as u8,
            ((parent_rgba[2] as u16 * local_rgba[2] as u16) / 255) as u8,
            ((parent_rgba[3] as u16 * local_rgba[3] as u16) / 255) as u8,
        ];
        if rgba[3] == 0 {
            return;
        }
        let z_plane = field_text(obj, "meZDepth").map(parse_z_plane).unwrap_or(15);

        match obj.kind.as_str() {
            "cGUICustomWidget" => {
                for child_id in field_ids(obj, "mWidgetArray") {
                    self.eval_widget(
                        graph,
                        runtime,
                        page_name,
                        child_id,
                        pos,
                        rgba,
                        depth + 1,
                        out,
                    );
                }
            }
            "cGUIDialogBox0Widget"
            | "cGUIDialogBox1Widget"
            | "cGUIDialogBox2Widget"
            | "cGUIDialogBox3Widget" => {
                for ptr_name in [
                    "mpBackground",
                    "mpTitle",
                    "mpMessage",
                    "mpButton",
                    "mpButton1",
                    "mpButton2",
                    "mpButton3",
                ] {
                    if let Some(child_id) = field_id(obj, ptr_name) {
                        self.eval_widget(
                            graph,
                            runtime,
                            page_name,
                            child_id,
                            pos,
                            rgba,
                            depth + 1,
                            out,
                        );
                    }
                }
            }
            "cGUITextureWidget" => {
                let size = field_id(obj, "mpSize")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([64.0, 32.0]);
                let tex_name =
                    field_id(obj, "mpTexture").and_then(|id| self.textures.get(&id).cloned());
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::Texture,
                    pos,
                    size,
                    rgba,
                    z_plane,
                    texture_name: tex_name,
                });
            }
            "cGUIRectWidget" | "cGUIVertexColouredRectWidget" => {
                let size = field_id(obj, "mpSize")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([640.0, 480.0]);
                let rect_rgba = field_id(obj, "mpTLColour")
                    .and_then(|id| self.eval_rgba(graph, runtime, page_name, id))
                    .unwrap_or(rgba);
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::Rect,
                    pos,
                    size,
                    rgba: rect_rgba,
                    z_plane,
                    texture_name: None,
                });
            }
            "cGUISpriteWidget" => {
                let scale = field_id(obj, "mpScale")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([1.0, 1.0]);
                let (size, tex_name) = field_id(obj, "mpSprite")
                    .and_then(|sid| self.sprites.get(&sid))
                    .and_then(|bid| self.sprite_bases.get(bid))
                    .map(|base| {
                        let w = (base.br_uv[0] - base.tl_uv[0]).abs().max(16.0)
                            * base.scale[0]
                            * scale[0];
                        let h = (base.br_uv[1] - base.tl_uv[1]).abs().max(16.0)
                            * base.scale[1]
                            * scale[1];
                        ([w, h], base.texture_name.clone())
                    })
                    .unwrap_or(([32.0 * scale[0], 32.0 * scale[1]], None));
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::Sprite,
                    pos,
                    size,
                    rgba,
                    z_plane,
                    texture_name: tex_name,
                });
            }
            "cGUIButtonWidget" => {
                let btn_active = field_id(obj, "mpButtonActive")
                    .map(|id| self.eval_bool(graph, runtime, page_name, id))
                    .unwrap_or(true);
                let size = field_id(obj, "mpSelectionSize")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([120.0, 32.0]);
                let tex_name = field_id(obj, "mpNormal")
                    .or_else(|| field_id(obj, "mpSelected"))
                    .or_else(|| field_id(obj, "mpSprite"))
                    .and_then(|sid| self.sprites.get(&sid))
                    .and_then(|bid| self.sprite_bases.get(bid))
                    .and_then(|b| b.texture_name.clone());
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::Button {
                        selected: btn_active,
                    },
                    pos,
                    size,
                    rgba,
                    z_plane,
                    texture_name: tex_name,
                });
                if let Some(label_id) = field_id(obj, "mpLabel") {
                    self.eval_widget(
                        graph,
                        runtime,
                        page_name,
                        label_id,
                        pos,
                        rgba,
                        depth + 1,
                        out,
                    );
                }
            }
            "cGUITextBoxWidget" => {
                let text = field_id(obj, "mpText")
                    .and_then(|id| self.eval_string(graph, runtime, page_name, id))
                    .unwrap_or_default();
                let size = field_id(obj, "mpSize")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([200.0, 28.0]);
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::TextBox { text },
                    pos,
                    size,
                    rgba,
                    z_plane,
                    texture_name: None,
                });
            }
            "cGUI3DObjectWidget" => {
                let size = field_id(obj, "mpSize")
                    .and_then(|id| self.eval_vec2(graph, runtime, page_name, id))
                    .unwrap_or([160.0, 120.0]);
                out.push(GuiCanvasQuad {
                    widget_id,
                    kind: GuiQuadKind::Model3D,
                    pos,
                    size,
                    rgba,
                    z_plane,
                    texture_name: None,
                });
            }
            _ => {}
        }
    }

    fn eval_active(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        obj: &Object,
    ) -> bool {
        if let Some(env) = runtime.env()
            && let Some(&explicit) = env.active_widgets.get(&obj.id.0)
        {
            return explicit;
        }
        let Some(active_id) = field_id(obj, "mpActive") else {
            return true;
        };
        self.eval_bool(graph, runtime, page_name, active_id)
    }

    fn eval_lua_binding(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        obj: &Object,
    ) -> Vec<Value> {
        let Some(name) = field_text(obj, "mcString") else {
            return Vec::new();
        };
        if name.is_empty() {
            return Vec::new();
        }
        let mut args = Vec::new();
        for param_id in field_ids(obj, "mpParameter") {
            if let Some(p_obj) = graph.objects.get(&param_id) {
                if let Some(n) = self.eval_number_obj(graph, runtime, page_name, p_obj) {
                    args.push(Value::Number(n as f64));
                } else if let Some(s) = self.eval_string(graph, runtime, page_name, param_id) {
                    args.push(Value::String(s));
                }
            }
        }
        if let Ok(member) = runtime.page_member(page_name, name) {
            match member {
                Value::Closure(_) | Value::Native(_) => {
                    if let Ok(vals) = runtime.call_page_method(page_name, name, &args, 15_000) {
                        return vals;
                    }
                }
                Value::Nil => {}
                other => return vec![other],
            }
        }
        if let Some(global_val) = runtime.global(name).cloned() {
            match global_val {
                Value::Closure(_) | Value::Native(_) => {
                    if let Ok(vals) = runtime.global_call(name, &args, 15_000) {
                        return vals;
                    }
                }
                Value::Nil => {}
                other => return vec![other],
            }
        }
        Vec::new()
    }

    fn eval_bool(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        id: ObjectId,
    ) -> bool {
        let Some(obj) = graph.objects.get(&id) else {
            return true;
        };
        match obj.kind.as_str() {
            "cGUIBoolBool" => field_text(obj, "meBool") != Some("eFalse"),
            "cGUIBoolFromLabel" | "cGUIBoolFromFunction" => {
                let vals = self.eval_lua_binding(graph, runtime, page_name, obj);
                match vals.first() {
                    Some(Value::Boolean(b)) => *b,
                    Some(Value::Number(n)) => *n != 0.0,
                    Some(Value::Nil) | None => false,
                    _ => true,
                }
            }
            _ => true,
        }
    }

    fn eval_number_obj(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        obj: &Object,
    ) -> Option<f32> {
        match obj.kind.as_str() {
            "cGUINumberNumber" | "cGUIColourNumber" => {
                field_text(obj, "msNumber")?.parse::<f32>().ok()
            }
            "cGUINumberFromLabel" | "cGUINumberFromFunction" => {
                let vals = self.eval_lua_binding(graph, runtime, page_name, obj);
                match vals.first()? {
                    Value::Number(n) => Some(*n as f32),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn eval_vec2(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        id: ObjectId,
    ) -> Option<[f32; 2]> {
        let obj = graph.objects.get(&id)?;
        match obj.kind.as_str() {
            "cGUIVectorVector" | "cGUIScaleVector" => parse_vec2(field_text(obj, "msVector")?),
            "cGUIVectorFromLabel"
            | "cGUIVectorFromFunction"
            | "cGUIScaleFromLabel"
            | "cGUIScaleFromFunction" => {
                let vals = self.eval_lua_binding(graph, runtime, page_name, obj);
                match vals.as_slice() {
                    [Value::Number(x), Value::Number(y), ..] => Some([*x as f32, *y as f32]),
                    [Value::Number(s)] => Some([*s as f32, *s as f32]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn eval_rgba(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        id: ObjectId,
    ) -> Option<[u8; 4]> {
        let obj = graph.objects.get(&id)?;
        match obj.kind.as_str() {
            "cGUIColourColour" => parse_rgba(field_text(obj, "msColour")?),
            "cGUIColourNumber" => {
                let a = field_text(obj, "msNumber")?
                    .parse::<f32>()
                    .ok()?
                    .clamp(0.0, 255.0) as u8;
                Some([255, 255, 255, a])
            }
            "cGUIColourFromLabel" | "cGUIColourFromFunction" => {
                let vals = self.eval_lua_binding(graph, runtime, page_name, obj);
                match vals.as_slice() {
                    [
                        Value::Number(r),
                        Value::Number(g),
                        Value::Number(b),
                        Value::Number(a),
                        ..,
                    ] => Some([
                        r.clamp(0.0, 255.0) as u8,
                        g.clamp(0.0, 255.0) as u8,
                        b.clamp(0.0, 255.0) as u8,
                        a.clamp(0.0, 255.0) as u8,
                    ]),
                    [Value::Number(a)] => Some([255, 255, 255, a.clamp(0.0, 255.0) as u8]),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    fn eval_string(
        &self,
        graph: &ObjectGraph,
        runtime: &mut Runtime,
        page_name: &str,
        id: ObjectId,
    ) -> Option<String> {
        let obj = graph.objects.get(&id)?;
        match obj.kind.as_str() {
            "cGUIStringString" => field_text(obj, "mcString").map(str::to_owned),
            "cGUIStringFromLabel" | "cGUIStringFromFunction" => {
                let vals = self.eval_lua_binding(graph, runtime, page_name, obj);
                vals.first().and_then(|v| runtime.resolve_string(v))
            }
            _ => None,
        }
    }
}
