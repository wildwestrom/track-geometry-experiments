use std::collections::HashMap;

use crate::camera::CameraMode;

use bevy::{
	gltf::GltfAssetLabel,
	math::Ray3d,
	picking::{
		Pickable,
		backend::ray::RayMap,
		mesh_picking::{
			MeshPickingPlugin,
			ray_cast::{MeshRayCast, MeshRayCastSettings},
		},
		pointer::PointerId,
	},
	prelude::*,
	render::render_resource::Face,
};
use crate::terrain::{self as terrain, TerrainMesh};
use terrain::{HeightMap, TerrainUpdateSet, calculate_terrain_height};

pub struct PinPlugin;

impl Plugin for PinPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<PinDragState>()
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

#[derive(Default, Resource)]
struct PinDragState {
	entries: HashMap<Entity, PinDragData>,
}

#[derive(Clone, Copy)]
struct PinDragData {
	offset: Vec3,
	pointer_id: PointerId,
	camera: Entity,
}

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
	pin_query: Query<&Transform, With<Pin>>,
	mut camera_mode: ResMut<CameraMode>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	mut drag_state: ResMut<PinDragState>,
) {
	if let Ok(pin_transform) = pin_query.get(drag_start.entity) {
		// Disable camera movement while dragging
		camera_mode.disable_camera_movement();

		let pointer_id = drag_start.pointer_id;
		let camera = drag_start.event.hit.camera;
		let terrain_entity = *terrain_mesh;
		let heightmap = *terrain_heightmap;
		let mut offset = Vec3::ZERO;

		if let Some(ray) = pointer_ray(&ray_map, pointer_id, camera) {
			if let Some(raycast_point) = raycast_terrain_point(ray, terrain_entity, &mut raycast) {
				let terrain_point = calculate_terrain_point_from_raycast(
					raycast_point,
					pin_transform.translation,
					&heightmap,
					&settings,
				);
				offset = pin_transform.translation - terrain_point;
			}
		}

		drag_state.entries.insert(
			drag_start.entity,
			PinDragData {
				offset,
				pointer_id,
				camera,
			},
		);
	}
}

// Observer function to handle pin drag end
fn on_pin_drag_end(
	drag_end: On<Pointer<DragEnd>>,
	pin_query: Query<Entity, With<Pin>>,
	mut camera_mode: ResMut<CameraMode>,
	mut drag_state: ResMut<PinDragState>,
) {
	if let Ok(_) = pin_query.get(drag_end.entity) {
		// Disable camera movement while dragging
		camera_mode.enable_camera_movement();
		drag_state.entries.remove(&drag_end.entity);
	}
}

/// Observer to update pin position when dragging
fn on_pin_drag_update(
	drag: On<Pointer<Drag>>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	mut pin_transform_query: Query<&mut Transform, With<Pin>>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	drag_state: Res<PinDragState>,
) {
	let Ok(mut pin_transform) = pin_transform_query.get_mut(drag.entity) else {
		return;
	};

	let Some(drag_data) = drag_state.entries.get(&drag.entity).copied() else {
		return;
	};

	if drag.pointer_id != drag_data.pointer_id {
		return;
	}

	if let Some(ray) = pointer_ray(&ray_map, drag_data.pointer_id, drag_data.camera) {
		if let Some(raycast_point) = raycast_terrain_point(ray, *terrain_mesh, &mut raycast) {
			let terrain_point = calculate_terrain_point_from_raycast(
				raycast_point,
				pin_transform.translation,
				*terrain_heightmap,
				&settings,
			);
			pin_transform.translation = terrain_point + drag_data.offset;
		}
	}
}

/// System to scale pins based on their distance from the camera and FOV
fn scale_pins_by_distance(
	mut pin_query: Query<&mut Transform, With<Pin>>,
	camera_query: Single<(&GlobalTransform, &Projection), With<bevy_panorbit_camera::PanOrbitCamera>>,
) {
	let (camera_transform, camera_projection) = *camera_query;
	let camera_pos = camera_transform.translation();

	// Get current FOV from the camera projection
	let current_fov = if let Projection::Perspective(persp) = camera_projection {
		persp.fov
	} else {
		// Shouldn't happen since we're using perspective projection, but handle gracefully
		return;
	};

	let reference_distance = 3000.0; // Distance at which pins have base scale
	let reference_fov = 60.0_f32.to_radians(); // Reference FOV (normal perspective mode)
	let min_scale = 1.0;

	// Calculate FOV-based scale factor
	// When FOV is very small (orthographic-like), objects appear larger, so we need to scale down
	// The relationship is: apparent_size ∝ tan(FOV/2)
	// Thank you LLMs. Would've taken me ages to figure out the math.
	let fov_scale_factor = (current_fov * 0.5).tan() / (reference_fov * 0.5).tan();

	for mut pin_transform in &mut pin_query {
		let distance = camera_pos.distance(pin_transform.translation);

		// Calculate scale factor based on distance
		// As distance increases, scale increases to maintain visual size
		let distance_scale_factor = (distance / reference_distance).max(min_scale);

		// Combine distance and FOV scale factors
		let scale_factor = distance_scale_factor * fov_scale_factor;
		pin_transform.scale = Vec3::splat(scale_factor);
	}
}

fn pointer_ray(ray_map: &RayMap, pointer_id: PointerId, camera: Entity) -> Option<Ray3d> {
	ray_map
		.iter()
		.find(|(ray_id, _)| ray_id.pointer == pointer_id && ray_id.camera == camera)
		.map(|(_, ray)| *ray)
}

fn calculate_terrain_point_from_raycast(
	raycast_point: Vec3,
	pin_translation: Vec3,
	heightmap: &HeightMap,
	settings: &terrain::Settings,
) -> Vec3 {
	let terrain_height = calculate_terrain_height(pin_translation, heightmap, settings);
	Vec3::new(raycast_point.x, terrain_height, raycast_point.z)
}

fn raycast_terrain_point(
	ray: Ray3d,
	terrain_entity: Entity,
	raycast: &mut MeshRayCast,
) -> Option<Vec3> {
	let filter = |entity: Entity| entity == terrain_entity;
	let raycast_settings = MeshRayCastSettings::default().with_filter(&filter);
	let hits = raycast.cast_ray(ray, &raycast_settings);
	hits
		.iter()
		.find(|(entity, _)| *entity == terrain_entity)
		.map(|(_, hit)| hit.point)
}
