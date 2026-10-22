//! Bevy consumer for Rust-owned LightningX snapshots. No native Tequila parity claim.
use crate::mission_lightning::Lightning;
use bevy::{asset::RenderAssetUsages, mesh::PrimitiveTopology, prelude::*};
type Slot = (Entity, Handle<Mesh>, Handle<StandardMaterial>);
#[derive(Default, Resource)]
pub struct Renderer {
    slots: Vec<Option<Slot>>,
}
impl Renderer {
    pub fn sync(
        &mut self,
        effects: &[Lightning],
        camera: &Transform,
        commands: &mut Commands,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) {
        while self.slots.len() > effects.len() {
            if let Some((entity, _, _)) = self.slots.pop().unwrap() {
                // Drop ownership; Bevy releases assets after component handles disappear.
                commands.entity(entity).despawn();
            }
        }
        self.slots.resize_with(effects.len(), || None);
        for (index, effect) in effects.iter().enumerate() {
            let frame = effect.frame();
            if frame.triangles.is_empty() {
                // Do not upload zero-length vertex buffers: keep the resident mesh hidden.
                if let Some((entity, _, _)) = &self.slots[index] {
                    commands.entity(*entity).insert(Visibility::Hidden);
                }
                continue;
            }
            let positions: Vec<[f32; 3]> = frame.triangles.iter().flatten().copied().collect();
            let count = positions.len();
            let mut mesh = Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::default(),
            );
            mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
            mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, vec![[0., 0., 1.]; count]);
            mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, vec![[0., 0.]; count]);
            let material = StandardMaterial {
                base_color: Color::srgba(
                    frame.colour[0],
                    frame.colour[1],
                    frame.colour[2],
                    frame.colour[3],
                ),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                cull_mode: None,
                ..default()
            };
            if let Some((entity, mesh_handle, material_handle)) = &self.slots[index] {
                *meshes.get_mut(mesh_handle).expect("owned lightning mesh") = mesh;
                *materials
                    .get_mut(material_handle)
                    .expect("owned lightning material") = material;
                commands
                    .entity(*entity)
                    .insert((*camera, Visibility::Visible));
            } else {
                let mesh = meshes.add(mesh);
                let material = materials.add(material);
                let entity = commands
                    .spawn((
                        Mesh3d(mesh.clone()),
                        MeshMaterial3d(material.clone()),
                        *camera,
                    ))
                    .id();
                self.slots[index] = Some((entity, mesh, material));
            }
        }
    }
    pub fn instance_count(&self) -> usize {
        self.slots.iter().flatten().count()
    }
}
