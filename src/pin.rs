use crate::camera::CameraMode;

use bevy::{
	gltf::GltfAssetLabel,
	picking::{Pickable, mesh_picking::MeshPickingPlugin},
	prelude::*,
	render::render_resource::Face,
	window::PrimaryWindow,
};
use bevy_procedural_terrain_gen as terrain;
use terrain::{
	HeightMap, TerrainUpdateSet, calculate_terrain_height, clamp_to_terrain_bounds, raycast_terrain,
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
			.add_plugins(MeshPickingPlugin);
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
fn on_pin_drag_start(_trigger: On<Pointer<DragStart>>, mut camera_mode: ResMut<CameraMode>) {
	// Disable camera movement while dragging
	camera_mode.disable_camera_movement();
}

/// Observer to update pin position when dragging (combines cursor raycast + position update)
fn on_pin_drag_update(
	on: On<Pointer<Drag>>,
	window: Single<&Window, With<PrimaryWindow>>,
	camera_query: Single<(&Camera, &GlobalTransform), With<bevy_panorbit_camera::PanOrbitCamera>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	mut pin_query: Query<&mut Transform, With<Pin>>,
) {
	let Ok(mut pin_transform) = pin_query.get_mut(on.event().entity) else {
		// Unless my mental model is wrong, if this gets triggered,
		// that means whatever was being dragged was not a pin.
		panic!("If this gets triggered, that means whatever was being dragged was not a pin");
	};

	let (camera, camera_transform) = *camera_query;

	let Some(cursor_pos) = window.cursor_position() else {
		warn!("No cursor position found");
		return;
	};

	// Raycast from camera through cursor
	match camera.viewport_to_world(camera_transform, cursor_pos) {
		Ok(ray) => {
			// TODO: Replace with bevy's built in raycasting system
			if let Some(intersection) = raycast_terrain(&ray, &terrain_heightmap, &settings) {
				// Clamp position to terrain bounds and update pin position directly
				let clamped_pos = clamp_to_terrain_bounds(intersection, &settings);
				pin_transform.translation.x = clamped_pos.x;
				pin_transform.translation.z = clamped_pos.z;
				pin_transform.translation.y = clamped_pos.y;
			} else {
				panic!("No intersection found between ray and terrain");
			}
		}
		Err(e) => {
			warn!("Failed to create a ray due to {e}");
		}
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

// Observer function to handle pin drag end
fn on_pin_drag_end(_: On<Pointer<DragEnd>>, mut camera_mode: ResMut<CameraMode>) {
	camera_mode.enable_camera_movement();
}
