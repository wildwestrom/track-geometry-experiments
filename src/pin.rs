use crate::{
	camera::CameraMode,
	spatial::{calculate_terrain_height, clamp_to_terrain_bounds, world_size, world_size_for_height},
	terrain::{self, HeightMap, TerrainUpdateSet},
};
use bevy::{prelude::*, window::PrimaryWindow};

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
					update_cursor_world_pos,
					update_dragged_pin_position,
					cancel_drag_on_global_mouse_release,
				),
			)
			.add_plugins(MeshPickingPlugin)
			.insert_resource(PinDragState::default())
			.insert_resource(CursorWorldPos::default());
	}
}

#[derive(Component)]
pub struct Pin;

#[derive(Resource, Default)]
pub struct PinDragState {
	dragging_pin: Option<Entity>,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct CursorWorldPos(pub Option<Vec3>);

const PINHEAD_RADIUS: f32 = 50_f32;
const PIN_RADIUS: f32 = PINHEAD_RADIUS * (3.0 / 16.0);
const PIN_HEIGHT: f32 = PINHEAD_RADIUS * 5.0;

// This is so the pinhead sticks out above the terrain and is slightly nailed into the ground.
const PIN_OFFSET: f32 = PIN_HEIGHT * 0.8;

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
		// Offset the cylinder height so that the sphere always follows the mouse when dragging.
		if point.y - PIN_OFFSET <= terrain_height {
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
	commands: &mut Commands<'_, '_>,
	meshes: &mut ResMut<'_, Assets<Mesh>>,
	materials: &mut ResMut<'_, Assets<StandardMaterial>>,
	initial_position: Vec3,
	pin_head_color: Color,
	world_size: f32,
	point_id: impl Component,
) {
	let sphere_mesh = Sphere::new(PINHEAD_RADIUS).mesh().build();
	let cylinder_mesh = Cylinder::new(PIN_RADIUS, PIN_HEIGHT).mesh().build();

	// Create separate mesh handles
	let sphere_handle = meshes.add(sphere_mesh);
	let cylinder_handle = meshes.add(cylinder_mesh);

	// Create different materials
	let sphere_material = materials.add(pin_head_color);
	// Red sphere
	let cylinder_material = materials.add(Color::srgb(0.8, 0.8, 0.8));
	// Light gray cylinder

	// Spawn the sphere (head) as the parent with the cylinder as a child
	commands
		.spawn((
			Mesh3d(sphere_handle),
			MeshMaterial3d(sphere_material),
			Transform::from_xyz(0.0, 0.0, 0.0)
				.with_translation(initial_position * world_size + Vec3::new(0.0, 0.0, 0.0)),
			Pin,
			point_id,
			Pickable::default(),
		))
		.observe(on_pin_drag_start)
		.observe(on_pin_drag_end)
		.with_children(|parent| {
			parent.spawn((
				Mesh3d(cylinder_handle),
				MeshMaterial3d(cylinder_material),
				Transform::from_xyz(0.0, -PIN_HEIGHT * 0.5, 0.0), // Position cylinder below sphere
				Pickable::default(),
			));
		});
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

			// Position the base at ground level - the sphere will follow as a child
			transform.translation.y = terrain_height + PIN_OFFSET;
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
	info!("Started dragging pin {:?}", trigger.target());
	drag_state.dragging_pin = Some(trigger.target());

	// Disable camera movement while dragging
	camera_mode.disable_camera_movement();
}

/// System to update the world position under the cursor every frame
fn update_cursor_world_pos(
	windows: Query<&Window, With<PrimaryWindow>>,
	camera_query: Query<(&Camera, &GlobalTransform), With<bevy_panorbit_camera::PanOrbitCamera>>,
	terrain_heightmap: Query<&HeightMap>,
	settings: Res<terrain::Settings>,
	mut cursor_world_pos: ResMut<CursorWorldPos>,
) {
	let window = if let Ok(window) = windows.single() {
		window
	} else {
		return;
	};
	let Some(cursor_pos) = window.cursor_position() else {
		cursor_world_pos.0 = None;
		return;
	};
	let (camera, camera_transform) = if let Ok(val) = camera_query.single() {
		val
	} else {
		return;
	};

	// Get the heightmap for terrain intersection
	let Ok(heightmap) = terrain_heightmap.single() else {
		cursor_world_pos.0 = None;
		return;
	};

	// Raycast from camera through cursor
	if let Ok(ray) = camera.viewport_to_world(camera_transform, cursor_pos) {
		// Perform ray-terrain intersection by stepping along the ray
		if let Some(intersection) = raycast_terrain(&ray, heightmap, &settings) {
			cursor_world_pos.0 = Some(intersection);
			return;
		}
	}
	cursor_world_pos.0 = None;
}

/// System to update the position of the currently dragged pin every frame
pub fn update_dragged_pin_position(
	drag_state: Res<PinDragState>,
	mut pin_query: Query<&mut Transform, With<Pin>>,
	cursor_world_pos: Res<CursorWorldPos>,
	settings: Res<terrain::Settings>,
) {
	if let Some(dragging_entity) = drag_state.dragging_pin {
		if let Ok(mut pin_transform) = pin_query.get_mut(dragging_entity) {
			if let Some(world_pos) = cursor_world_pos.0 {
				// Clamp position to terrain bounds
				let clamped_pos = clamp_to_terrain_bounds(world_pos, &settings);

				// Position pin so the sphere (head) is above the terrain intersection point
				pin_transform.translation.x = clamped_pos.x;
				pin_transform.translation.z = clamped_pos.z;
				pin_transform.translation.y = clamped_pos.y + PIN_OFFSET;
			}
		}
	}
}

/// System to cancel pin dragging if the mouse button is released anywhere
fn cancel_drag_on_global_mouse_release(
	mouse_buttons: Res<ButtonInput<MouseButton>>,
	mut drag_state: ResMut<PinDragState>,
	mut camera_mode: ResMut<CameraMode>,
) {
	if drag_state.dragging_pin.is_some() && mouse_buttons.just_released(MouseButton::Left) {
		camera_mode.enable_camera_movement();
		drag_state.dragging_pin = None;
		info!("Stopped dragging pin due to global mouse release");
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
			info!("Stopped dragging pin {:?}", trigger.target());
		}
	}
}
