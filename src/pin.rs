use crate::{
	camera::CameraMode,
	terrain::{self, HeightMap, TerrainUpdateSet},
};
use bevy::{prelude::*, window::PrimaryWindow};

pub struct PinPlugin;

impl Plugin for PinPlugin {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, startup)
			.add_systems(
				Update,
				(
					move_pins_above_terrain,
					update_dragged_pin_position,
					cancel_drag_on_global_mouse_release,
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

fn startup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	settings: Res<terrain::Settings>,
) {
	let world_size = settings.world_x().min(settings.world_z());
	create_pin(
		&mut commands,
		&mut meshes,
		&mut materials,
		Vec3::new(0.45, 0.0, 0.0),
		Color::srgb(0.8, 0.0, 0.0), // Red
		world_size,
	);
	create_pin(
		&mut commands,
		&mut meshes,
		&mut materials,
		Vec3::new(0.0, 0.0, 0.45),
		Color::srgb(0.0, 0.0, 0.8), // Blue
		world_size,
	);
}

#[derive(Component)]
struct Pin;

#[derive(Resource, Default)]
struct PinDragState {
	dragging_pin: Option<Entity>,
}

#[derive(Resource, Default, Debug, Clone, Copy)]
struct CursorWorldPos(pub Option<Vec3>);

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
	let grid_x = settings.grid_x();
	let grid_z = settings.grid_z();
	let world_x = settings.world_x();
	let world_z = settings.world_z();

	// Step along the ray to find intersection with terrain
	let max_distance = 10000.0; // Maximum ray distance
	let step_size = 0.2; // Step size for ray marching

	let mut t = 0.0;
	let mut last_valid_point = None;

	while t < max_distance {
		let point = ray.origin + ray.direction * t;

		// Clamp point to terrain bounds for height lookup
		let clamped_x = point.x.clamp(-world_x / 2.0, world_x / 2.0);
		let clamped_z = point.z.clamp(-world_z / 2.0, world_z / 2.0);

		// Convert from world space to grid space
		let grid_x_f = (clamped_x + world_x / 2.0) / world_x * grid_x as f32;
		let grid_z_f = (clamped_z + world_z / 2.0) / world_z * grid_z as f32;

		// Clamp to valid grid coordinates
		let grid_x_clamped = (grid_x_f as u32).min(grid_x);
		let grid_z_clamped = (grid_z_f as u32).min(grid_z);

		// TODO: Really use the mesh, because the heightmap doesn't always match the mesh. The mesh will have gradients whereas the heightmap has finite discrete points.
		// Get height from heightmap and apply the same scaling as the terrain mesh
		let terrain_height = heightmap.get(grid_x_clamped, grid_z_clamped)
			* world_x.min(world_z)
			* settings.height_multiplier;

		// Check if ray point is at or below terrain height
		// Offset the cylinder height so that the sphere always follows the mouse when dragging.
		if point.y - PIN_OFFSET <= terrain_height {
			// Use clamped position for the result
			return Some(Vec3::new(clamped_x, terrain_height, clamped_z));
		}

		// Store the last valid point in case we need to fall back to terrain boundary
		if point.x >= -world_x / 2.0
			&& point.x <= world_x / 2.0
			&& point.z >= -world_z / 2.0
			&& point.z <= world_z / 2.0
		{
			last_valid_point = Some(Vec3::new(clamped_x, terrain_height, clamped_z));
		}

		t += step_size;
	}

	// If we didn't find an intersection, return the last valid point on terrain boundary
	last_valid_point
}

fn create_pin(
	commands: &mut Commands<'_, '_>,
	meshes: &mut ResMut<'_, Assets<Mesh>>,
	materials: &mut ResMut<'_, Assets<StandardMaterial>>,
	initial_position: Vec3,
	pin_head_color: Color,
	world_size: f32,
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
		// Convert world coordinates to grid coordinates
		let grid_x = settings.grid_x();
		let grid_z = settings.grid_z();
		let world_x = settings.world_x();
		let world_z = settings.world_z();

		for (entity, mut transform) in pin_transforms.iter_mut() {
			// Skip positioning for the pin that's being dragged
			if let Some(dragging_entity) = drag_state.dragging_pin {
				if entity == dragging_entity {
					continue;
				}
			}

			// Convert from world space (-world_size/2 to world_size/2) to grid space (0 to grid_length/width)
			let grid_x =
				((transform.translation.x + world_x / 2.0) / world_x * grid_x as f32) as u32;
			let grid_z =
				((transform.translation.z + world_z / 2.0) / world_z * grid_z as f32) as u32;

			// Clamp to valid grid coordinates
			let grid_x = grid_x.min(grid_x);
			let grid_z = grid_z.min(grid_z);

			// Get height and apply the same scaling as the terrain mesh
			let height =
				heightmap.get(grid_x, grid_z) * world_x.min(world_z) * settings.height_multiplier
					+ PIN_OFFSET;

			// Position the base at ground level - the sphere will follow as a child
			transform.translation.y = height;
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
	camera_mode.user_enabled = false;
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
fn update_dragged_pin_position(
	drag_state: Res<PinDragState>,
	mut pin_query: Query<&mut Transform, With<Pin>>,
	cursor_world_pos: Res<CursorWorldPos>,
	settings: Res<terrain::Settings>,
) {
	if let Some(dragging_entity) = drag_state.dragging_pin {
		if let Ok(mut pin_transform) = pin_query.get_mut(dragging_entity) {
			if let Some(world_pos) = cursor_world_pos.0 {
				let half_world_x = settings.world_x() / 2.0;
				let half_world_z = settings.world_z() / 2.0;

				// Position cylinder at terrain height so sphere hovers above it
				pin_transform.translation.x = world_pos.x.clamp(-half_world_x, half_world_x);
				pin_transform.translation.z = world_pos.z.clamp(-half_world_z, half_world_z);
				pin_transform.translation.y = world_pos.y + PIN_OFFSET;
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
		camera_mode.user_enabled = true;
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
			camera_mode.user_enabled = true;
			drag_state.dragging_pin = None;
			info!("Stopped dragging pin {:?}", trigger.target());
		}
	}
}
