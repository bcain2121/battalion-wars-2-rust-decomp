//! Bevy adapter for original terrain texture pairs. No engine-specific data in parsers.
use crate::{terrain::Terrain, texture::DecodedTexture};
use bevy::{
    asset::RenderAssetUsages,
    image::{ImageAddressMode, ImageSampler, ImageSamplerDescriptor},
    mesh::{Indices, PrimitiveTopology},
    prelude::*,
    render::render_resource::{AsBindGroup, Extent3d, TextureDimension, TextureFormat},
    shader::ShaderRef,
};
use std::collections::BTreeMap;

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub struct TerrainMaterial {
    #[texture(0)]
    #[sampler(1)]
    pub first: Handle<Image>,
    #[texture(2)]
    #[sampler(3)]
    pub second: Handle<Image>,
}
impl Material for TerrainMaterial {
    fn fragment_shader() -> ShaderRef {
        "shaders/terrain.wgsl".into()
    }
}

pub fn spawn(
    commands: &mut Commands,
    terrain: Terrain,
    textures: BTreeMap<String, DecodedTexture>,
    meshes: &mut Assets<Mesh>,
    images: &mut Assets<Image>,
    materials: &mut Assets<TerrainMaterial>,
) {
    let mut handles = BTreeMap::new();
    for (name, texture) in textures {
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
        handles.insert(name, images.add(image));
    }
    let quantize_xz = |p: [f32; 3]| -> (i32, i32) {
        ((p[0] * 16.0).round() as i32, (p[2] * 16.0).round() as i32)
    };
    let mut global_normals: BTreeMap<(i32, i32), Vec3> = BTreeMap::new();
    for tri in terrain.indices.chunks_exact(3) {
        let p0 = Vec3::from_array(terrain.positions[tri[0] as usize]);
        let p1 = Vec3::from_array(terrain.positions[tri[1] as usize]);
        let p2 = Vec3::from_array(terrain.positions[tri[2] as usize]);
        let mut face_n = (p1 - p0).cross(p2 - p0);
        if face_n.y < 0.0 {
            face_n = -face_n;
        }
        if face_n.length_squared() > 1e-8 {
            for &vi in tri {
                let key = quantize_xz(terrain.positions[vi as usize]);
                *global_normals.entry(key).or_insert(Vec3::ZERO) += face_n;
            }
        }
    }
    for n in global_normals.values_mut() {
        *n = n.try_normalize().unwrap_or(Vec3::Y);
    }

    for (material_index, pair) in terrain.materials.iter().enumerate() {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut colors = Vec::new();
        let mut uv0 = Vec::new();
        let mut uv1 = Vec::new();
        let mut indices = Vec::new();
        for (tile, index) in terrain.tile_materials.iter().enumerate() {
            if *index != material_index {
                continue;
            }
            let base = positions.len() as u32;
            let start = tile * 16;
            for v in start..start + 16 {
                let pos = terrain.positions[v];
                let norm = global_normals
                    .get(&quantize_xz(pos))
                    .copied()
                    .unwrap_or(Vec3::Y);
                positions.push(pos);
                normals.push(norm.to_array());
            }
            colors.extend_from_slice(&terrain.colors[start..start + 16]);
            uv0.extend_from_slice(&terrain.uv0[start..start + 16]);
            uv1.extend_from_slice(&terrain.uv1[start..start + 16]);
            for i in &terrain.indices[tile * 54..tile * 54 + 54] {
                indices.push(base + *i - start as u32);
            }
        }
        if positions.is_empty() {
            continue;
        }
        let mesh = Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::default(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
        .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
        .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, colors)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, uv0)
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_1, uv1)
        .with_inserted_indices(Indices::U32(indices));
        let first = handles[&pair[0].to_ascii_lowercase()].clone();
        let second = handles[&pair[1].to_ascii_lowercase()].clone();
        commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(TerrainMaterial { first, second })),
        ));
    }
}
