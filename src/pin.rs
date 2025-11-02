use crate::camera::CameraMode;

use bevy::{
	gltf::GltfAssetLabel,
	picking::{Pickable, backend::ray::RayMap, mesh_picking::MeshPickingPlugin},
	prelude::*,
	render::render_resource::Face,
};
use bevy_procedural_terrain_gen::{self as terrain, TerrainMesh};
use terrain::{HeightMap, TerrainUpdateSet, calculate_terrain_height};

pub struct PinPlugin;

impl Plugin for PinPlugin {
	fn build(&self, app: &mut App) {
		app
			//.add_systems(Startup, startup)
			.add_systems(
				Update,
				(
					// This is to make sure when we grab a point from the heightmap
					// we're always indexing the array within the bounds of the heightmap.
					move_pins_above_terrain
						.after(TerrainUpdateSet)
						.run_if(heightmap_changed.or(new_pins_added)),
					scale_pins_by_distance,
				),
			)
			.add_plugins(MeshPickingPlugin)
			.add_observer(on_pin_drag_update)
			.add_observer(on_pin_drag_start)
			.add_observer(on_pin_drag_end);
	}
}

#[derive(Component)]
pub struct Pin;

pub fn create_pin(
	initial_position: Vec3,
	world_size: f32,
	point_id: impl Component,
	pinhead_color: Color,
) -> impl Command {
	move |world: &mut World| {
		// Load both meshes from the GLTF primitives
		let needle_mesh = {
			let asset_server = world.resource::<AssetServer>();
			asset_server.load(
				GltfAssetLabel::Primitive {
					mesh: 0,
					primitive: 0,
				}
				.from_asset("pin.glb"),
			)
		};

		let pinhead_mesh = {
			let asset_server = world.resource::<AssetServer>();
			asset_server.load(
				GltfAssetLabel::Primitive {
					mesh: 1,
					primitive: 0,
				}
				.from_asset("pin.glb"),
			)
		};

		let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
		let needle_material = materials.add(StandardMaterial::default());
		let pinhead_material = materials.add(StandardMaterial {
			base_color: pinhead_color,
			cull_mode: Some(Face::Back),
			..default()
		});

		let final_position = initial_position * world_size;

		// Spawn a parent entity with the pin components and children
		world
			.spawn((
				Pin,
				point_id,
				Pickable::default(),
				Transform::from_translation(final_position),
				Visibility::default(),
				InheritedVisibility::default(),
				ViewVisibility::default(),
			))
			.with_children(|parent| {
				parent.spawn((
					Mesh3d(needle_mesh),
					MeshMaterial3d(needle_material),
					Transform::default(),
				));
				parent.spawn((
					Mesh3d(pinhead_mesh),
					MeshMaterial3d(pinhead_material),
					Transform::default(),
				));
			});
	}
}

fn heightmap_changed(terrain_query: Query<(), (With<TerrainMesh>, Changed<HeightMap>)>) -> bool {
	!terrain_query.is_empty()
}

fn new_pins_added(pin_query: Query<(), Added<Pin>>) -> bool {
	!pin_query.is_empty()
}

fn move_pins_above_terrain(
	mut pin_transforms: Query<&mut Transform, With<Pin>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
) {
	let heightmap = *terrain_heightmap;
	for mut transform in &mut pin_transforms {
		// Get height using spatial utilities
		let terrain_height = calculate_terrain_height(transform.translation, heightmap, &settings);

		// Position the base so its bottom sits on the terrain surface
		transform.translation.y = terrain_height;
	}
}

// Observer function to handle pin drag start
fn on_pin_drag_start(
	drag_start: On<Pointer<DragStart>>,
	pin_query: Query<Entity, With<Pin>>,
	mut camera_mode: ResMut<CameraMode>,
) {
	if let Ok(_) = pin_query.get(drag_start.entity) {
		// Disable camera movement while dragging
		camera_mode.disable_camera_movement();
	}
}

// Observer function to handle pin drag end
fn on_pin_drag_end(
	drag_end: On<Pointer<DragEnd>>,
	pin_query: Query<Entity, With<Pin>>,
	mut camera_mode: ResMut<CameraMode>,
) {
	if let Ok(_) = pin_query.get(drag_end.entity) {
		// Disable camera movement while dragging
		camera_mode.enable_camera_movement();
	}
}

/// Observer to update pin position when dragging
fn on_pin_drag_update(
	drag: On<Pointer<Drag>>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	mut pin_transform_query: Query<&mut Transform, With<Pin>>,
	camera_query: Single<Entity, With<bevy_panorbit_camera::PanOrbitCamera>>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
) {
	let Ok(mut pin_transform) = pin_transform_query.get_mut(drag.entity) else {
		return;
	};

	let Some((_, ray)) = ray_map
		.iter()
		.filter(|(ray_id, _)| ray_id.camera == *camera_query)
		.next()
	else {
		return;
	};
	let filter = |e: Entity| e == *terrain_mesh;
	let raycast_settings = MeshRayCastSettings::default().with_filter(&filter);
	let hits = raycast.cast_ray(*ray, &raycast_settings);
	if let Some((_, hit)) = hits
		.iter()
		.filter(|(entity, _)| *entity == *terrain_mesh)
		.next()
	{
		let point = hit.point;
		pin_transform.translation = point;
	}
}

/// System to scale pins based on their distance from the camera
fn scale_pins_by_distance(
	mut pin_query: Query<&mut Transform, With<Pin>>,
	camera_query: Single<&GlobalTransform, With<bevy_panorbit_camera::PanOrbitCamera>>,
) {
	let camera_transform = *camera_query;
	let camera_pos = camera_transform.translation();

	let reference_distance = 3000.0; // Distance at which pins have base scale
	let min_scale = 1.0;

	for mut pin_transform in &mut pin_query {
		let distance = camera_pos.distance(pin_transform.translation);

		// Calculate scale factor based on distance
		// As distance increases, scale increases to maintain visual size
		let scale_factor = (distance / reference_distance).max(min_scale);
		pin_transform.scale = Vec3::splat(scale_factor);
	}
}
