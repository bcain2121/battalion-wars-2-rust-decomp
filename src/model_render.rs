//! Original geometry/material adapter. Callers own animated mesh deformation and attachments.
use crate::{Result, model::Model, texture::DecodedTexture};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Part {
    pub mesh: Handle<Mesh>,
    pub material: Handle<StandardMaterial>,
    pub transform: Mat4,
    pub node_index: usize,
    pub surface_index: usize,
}
pub struct RenderModel {
    pub parts: Vec<Part>,
    pub bones: BTreeMap<String, Mat4>,
}
pub fn build(
    model: &Model,
    textures: &BTreeMap<String, Handle<Image>>,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) -> Result<RenderModel> {
    let mut out = RenderModel {
        parts: Vec::new(),
        bones: BTreeMap::new(),
    };
    let transforms = crate::model_geometry::bind_transforms(model)?;
    for (i, node) in model.nodes.iter().enumerate() {
        let transform = transforms[i];
        out.bones.insert(node.name.clone(), transform);
        if node.name.contains("NODRAW")
            || node.name.contains("COLLIDE")
            || node.name.contains("COLLISION")
        {
            continue;
        }
        for (surface_index, surface) in node.surfaces.iter().enumerate() {
            if surface.indices.is_empty() {
                continue;
            }
            let mat = node
                .materials
                .get(surface.material)
                .ok_or("Surface material index out of bounds")?;
            let blend_flag = node
                .material_blend_modes
                .get(surface.material)
                .copied()
                .unwrap_or(0);
            let tex_name = mat
                .iter()
                .find(|s| !s.is_empty())
                .map(|s| s.to_ascii_uppercase())
                .unwrap_or_default();
            let texture = mat
                .iter()
                .find(|s| !s.is_empty())
                .map(|s| {
                    textures
                        .get(&s.to_ascii_lowercase())
                        .cloned()
                        .ok_or_else(|| format!("Missing decoded texture {s}"))
                })
                .transpose()?;
            let has_normals = surface.vertices.iter().any(|v| v.normal.is_some());
            let node_upper = node.name.to_ascii_uppercase();
            let model_upper = model.name.to_ascii_uppercase();
            let is_decal = node_upper.contains("DECAL")
                || node_upper.contains("SCORCH")
                || node_upper.contains("CRATER")
                || model_upper.contains("SCORCH")
                || model_upper.contains("CRATER")
                || tex_name.contains("SCORCH")
                || tex_name.contains("CRATER");
            let is_glass = node_upper.contains("GLASS")
                || node_upper.contains("CANOPY")
                || node_upper.contains("WINDOW")
                || tex_name.contains("GLASS")
                || tex_name.contains("CANOPY");
            let is_additive = blend_flag == 2
                || node_upper.contains("RAINBOW")
                || node_upper.contains("THRUSTER")
                || node_upper.contains("LIGHTBEAM")
                || tex_name.contains("THRUSTER")
                || tex_name.contains("MUZZLE")
                || tex_name.contains("GLOW");
            let alpha_mode = if is_additive {
                AlphaMode::Add
            } else if blend_flag == 1 || is_decal || is_glass {
                AlphaMode::Blend
            } else {
                AlphaMode::Mask(0.3)
            };
            let material = materials.add(StandardMaterial {
                base_color_texture: texture,
                perceptual_roughness: if is_glass { 0.25 } else { 0.85 },
                metallic: 0.0,
                reflectance: if is_glass { 0.5 } else { 0.2 },
                unlit: !has_normals || is_additive || is_decal,
                cull_mode: None,
                alpha_mode,
                depth_bias: if is_decal { 16.0 } else { 0.0 },
                ..default()
            });
            let mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_POSITION,
                surface
                    .vertices
                    .iter()
                    .map(|v| v.position)
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_NORMAL,
                surface
                    .vertices
                    .iter()
                    .map(|v| v.normal.unwrap_or([0.0, 1.0, 0.0]))
                    .collect::<Vec<_>>(),
            )
            .with_inserted_attribute(
                Mesh::ATTRIBUTE_UV_0,
                surface.vertices.iter().map(|v| v.uv).collect::<Vec<_>>(),
            )
            .with_inserted_indices(Indices::U32(surface.indices.clone()));
            out.parts.push(Part {
                mesh: meshes.add(mesh),
                material,
                transform,
                node_index: i,
                surface_index,
            });
        }
    }
    Ok(out)
}
pub fn image(texture: DecodedTexture, images: &mut Assets<Image>) -> Handle<Image> {
    let mut image = Image::new(
        Extent3d {
            width: texture.width as u32,
            height: texture.height as u32,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        texture.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::default(),
    );
    image.sampler = ImageSampler::Descriptor(ImageSamplerDescriptor {
        address_mode_u: ImageAddressMode::Repeat,
        address_mode_v: ImageAddressMode::Repeat,
        ..ImageSamplerDescriptor::linear()
    });
    images.add(image)
}
pub fn spawn(commands: &mut Commands, model: &RenderModel, root: Mat4) {
    for p in &model.parts {
        commands.spawn((
            Mesh3d(p.mesh.clone()),
            MeshMaterial3d(p.material.clone()),
            Transform::from_matrix(root * p.transform),
        ));
    }
}
