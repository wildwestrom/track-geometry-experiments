use crate::{
	camera::CameraMode,
	spatial::{calculate_terrain_height, clamp_to_terrain_bounds, world_size_for_height},
	terrain::{self, HeightMap, TerrainUpdateSet},
};
use bevy::{
	gltf::GltfAssetLabel, prelude::*, render::render_resource::Face, window::PrimaryWindow,
};

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
					move_pins_above_terrain.after(TerrainUpdateSet),
					scale_pins_by_distance,
				),
			)
			.add_plugins(MeshPickingPlugin)
			.insert_resource(PinDragState::default());
	}
}

#[derive(Component)]
pub struct Pin;

#[derive(Resource, Default)]
pub struct PinDragState {
	dragging_pin: Option<Entity>,
}

/// Performs ray-terrain intersection by stepping along the ray and checking heights
fn raycast_terrain(
	ray: &Ray3d,
	heightmap: &HeightMap,
	settings: &terrain::Settings,
) -> Option<Vec3> {
	let world_x = settings.world_x();
	let world_z = settings.world_z();

	// Calculate terrain bounds
	let min_x = -world_x / 2.0;
	let max_x = world_x / 2.0;
	let min_z = -world_z / 2.0;
	let max_z = world_z / 2.0;

	// Find the maximum possible terrain height for bounds checking
	let max_terrain_height = world_size_for_height(settings) * settings.height_multiplier;
	let min_terrain_height = 0.0; // Assuming terrain doesn't go below 0

	// Calculate intersection with terrain bounding box
	let mut t_min = 0.0f32;
	let mut t_max = f32::INFINITY;

	// Check X bounds
	if ray.direction.x != 0.0 {
		let tx1 = (min_x - ray.origin.x) / ray.direction.x;
		let tx2 = (max_x - ray.origin.x) / ray.direction.x;
		t_min = t_min.max(tx1.min(tx2));
		t_max = t_max.min(tx1.max(tx2));
	} else if ray.origin.x < min_x || ray.origin.x > max_x {
		return None; // Ray is parallel to X bounds and outside
	}

	// Check Z bounds
	if ray.direction.z != 0.0 {
		let tz1 = (min_z - ray.origin.z) / ray.direction.z;
		let tz2 = (max_z - ray.origin.z) / ray.direction.z;
		t_min = t_min.max(tz1.min(tz2));
		t_max = t_max.min(tz1.max(tz2));
	} else if ray.origin.z < min_z || ray.origin.z > max_z {
		return None; // Ray is parallel to Z bounds and outside
	}

	// Check Y bounds (terrain height range)
	if ray.direction.y != 0.0 {
		let ty1 = (min_terrain_height - ray.origin.y) / ray.direction.y;
		let ty2 = (max_terrain_height - ray.origin.y) / ray.direction.y;
		t_min = t_min.max(ty1.min(ty2));
		t_max = t_max.min(ty1.max(ty2));
	}

	// No intersection with bounding box
	if t_min > t_max || t_max < 0.0 {
		return None;
	}

	// Start from the entry point of the bounding box
	let mut t = t_min.max(0.0);
	let step_size = 0.2;
	let max_iterations = 10000; // Safety limit to prevent infinite loops
	let mut iterations = 0;

	let mut last_valid_point = None;

	while t <= t_max && iterations < max_iterations {
		iterations += 1;

		let point = ray.origin + ray.direction * t;

		// Clamp point to terrain bounds for height lookup
		let clamped_point = clamp_to_terrain_bounds(point, settings);

		// Get height from heightmap and apply the same scaling as the terrain mesh
		let terrain_height = calculate_terrain_height(clamped_point, heightmap, settings);

		// Check if ray point is at or below terrain height
		if point.y <= terrain_height {
			// Use clamped position for the result
			return Some(Vec3::new(clamped_point.x, terrain_height, clamped_point.z));
		}

		// Store the last valid point in case we need to fall back to terrain boundary
		if point.x >= min_x && point.x <= max_x && point.z >= min_z && point.z <= max_z {
			last_valid_point = Some(Vec3::new(clamped_point.x, terrain_height, clamped_point.z));
		}

		t += step_size;
	}

	// If we didn't find an intersection, return the last valid point on terrain boundary
	last_valid_point
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
			.observe(on_pin_drag_start)
			.observe(on_pin_drag_end)
			.observe(on_pin_drag_update)
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

fn move_pins_above_terrain(
	mut pin_transforms: Query<(Entity, &mut Transform), With<Pin>>,
	terrain_heightmap: Query<&HeightMap>,
	settings: Res<terrain::Settings>,
	drag_state: Res<PinDragState>,
) {
	if let Ok(heightmap) = terrain_heightmap.single() {
		for (entity, mut transform) in pin_transforms.iter_mut() {
			// Skip positioning for the pin that's being dragged
			if let Some(dragging_entity) = drag_state.dragging_pin {
				if entity == dragging_entity {
					continue;
				}
			}

			// Get height using spatial utilities
			let terrain_height =
				calculate_terrain_height(transform.translation, heightmap, &settings);

			// Position the base so its bottom sits on the terrain surface
			transform.translation.y = terrain_height;
		}
	} else {
		warn!("No heightmap found");
	}
}

// Observer function to handle pin drag start
fn on_pin_drag_start(
	trigger: Trigger<Pointer<Pressed>>,
	mut drag_state: ResMut<PinDragState>,
	mut camera_mode: ResMut<CameraMode>,
) {
	drag_state.dragging_pin = Some(trigger.target());

	// Disable camera movement while dragging
	camera_mode.disable_camera_movement();
}

/// Observer to update pin position when dragging (combines cursor raycast + position update)
fn on_pin_drag_update(
	trigger: Trigger<Pointer<Drag>>,
	windows: Query<&Window, With<PrimaryWindow>>,
	camera_query: Query<(&Camera, &GlobalTransform), With<bevy_panorbit_camera::PanOrbitCamera>>,
	terrain_heightmap: Query<&HeightMap>,
	settings: Res<terrain::Settings>,
	mut pin_query: Query<&mut Transform, With<Pin>>,
	drag_state: Res<PinDragState>,
) {
	// Only update if this entity is currently being dragged
	if let Some(dragging_entity) = drag_state.dragging_pin {
		if dragging_entity != trigger.target() {
			return;
		}
	} else {
		return;
	}

	// Get the pin transform for the dragged entity
	let Ok(mut pin_transform) = pin_query.get_mut(trigger.target()) else {
		return;
	};

	let Ok(window) = windows.single() else {
		return;
	};
	let Some(cursor_pos) = window.cursor_position() else {
		return;
	};
	let Ok((camera, camera_transform)) = camera_query.single() else {
		return;
	};

	// Get the heightmap for terrain intersection
	let Ok(heightmap) = terrain_heightmap.single() else {
		return;
	};

	// Raycast from camera through cursor
	if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
		// Perform ray-terrain intersection by stepping along the ray
		if let Some(intersection) = raycast_terrain(&ray, heightmap, &settings) {
			// Clamp position to terrain bounds and update pin position directly
			let clamped_pos = clamp_to_terrain_bounds(intersection, &settings);
			pin_transform.translation.x = clamped_pos.x;
			pin_transform.translation.z = clamped_pos.z;
			pin_transform.translation.y = clamped_pos.y;
		}
	}
}

/// System to scale pins based on their distance from the camera
fn scale_pins_by_distance(
	mut pin_query: Query<&mut Transform, With<Pin>>,
	camera_query: Query<&GlobalTransform, With<bevy_panorbit_camera::PanOrbitCamera>>,
) {
	if let Ok(camera_transform) = camera_query.single() {
		let camera_pos = camera_transform.translation();

		let reference_distance = 3000.0; // Distance at which pins have base scale
		let min_scale = 1.0;

		for mut pin_transform in pin_query.iter_mut() {
			let distance = camera_pos.distance(pin_transform.translation);

			// Calculate scale factor based on distance
			// As distance increases, scale increases to maintain visual size
			let scale_factor = (distance / reference_distance).max(min_scale);
			pin_transform.scale = Vec3::splat(scale_factor);
		}
	}
}

// Observer function to handle pin drag end
fn on_pin_drag_end(
	trigger: Trigger<Pointer<Released>>,
	mut drag_state: ResMut<PinDragState>,
	mut camera_mode: ResMut<CameraMode>,
) {
	if let Some(dragging_entity) = drag_state.dragging_pin {
		if dragging_entity == trigger.target() {
			camera_mode.enable_camera_movement();
			drag_state.dragging_pin = None;
		}
	}
}
