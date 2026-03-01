use bevy::{
	picking::{
		backend::ray::RayMap,
		mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
		pointer::PointerId,
	},
	prelude::*,
};

use crate::camera::PrimaryCamera3d;

use crate::pin::create_pin;
use crate::terrain::{self, HeightMap, TerrainMesh, calculate_terrain_height};
use terrain::spatial::world_size_for_height;

use super::components::{AlignmentPoint, PointType};
use super::state::{AlignmentState, DraftAlignment, TrackBuildingMode};

pub(crate) fn toggle_track_building_mode(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut mode: ResMut<TrackBuildingMode>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyF) {
		mode.active = !mode.active;
	}
	if keyboard_input.just_pressed(KeyCode::Escape) {
		mode.active = false;
	}
}

pub(crate) fn update_alignment_from_pins(
	alignment_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	let mut start_pos = None;
	let mut end_pos = None;

	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == alignment_state.turns {
			match alignment_point.point_type {
				PointType::Start => start_pos = Some(transform.translation),
				PointType::End => end_pos = Some(transform.translation),
				PointType::Intermediate { .. } => {}
			}
		}
	}

	let (Some(new_start), Some(new_end)) = (start_pos, end_pos) else {
		return;
	};

	for alignment in alignment_state.alignments.values_mut() {
		if alignment.start != new_start || alignment.end != new_end {
			alignment.start = new_start;
			alignment.end = new_end;
		}
	}
}

pub(crate) fn update_alignment_pins(
	mut commands: Commands,
	alignment_state: Res<AlignmentState>,
	existing_pins: Query<Entity, With<AlignmentPoint>>,
	settings: Res<terrain::Settings>,
	mut last_current_alignment: Local<Option<usize>>,
) {
	let current_alignment = alignment_state.turns;
	if *last_current_alignment == Some(current_alignment) {
		return;
	}
	*last_current_alignment = Some(current_alignment);

	for entity in existing_pins.iter() {
		commands.entity(entity).despawn();
	}

	if let Some(alignment) = alignment_state.alignments.get(&current_alignment) {
		let world_size = world_size_for_height(&settings);

		let start_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::Start,
		};
		let start_color = start_point.get_color();
		commands.queue(create_pin(
			alignment.start / world_size,
			world_size,
			start_point,
			start_color,
		));

		let end_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::End,
		};
		let end_color = end_point.get_color();
		commands.queue(create_pin(
			alignment.end / world_size,
			world_size,
			end_point,
			end_color,
		));

		for (i, segment) in alignment.segments.iter().enumerate() {
			let normalized_pos = segment.tangent_vertex / world_size;
			let alignment_point = AlignmentPoint {
				alignment_id: current_alignment,
				point_type: PointType::Intermediate { segment_index: i },
			};
			let point_color = alignment_point.get_color();
			commands.queue(create_pin(
				normalized_pos,
				world_size,
				alignment_point,
				point_color,
			));
		}
	}
}

pub(crate) fn update_alignment_from_intermediate_pins(
	intermediate_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	for (transform, intermediate_point) in intermediate_pins.iter() {
		if let PointType::Intermediate { segment_index } = intermediate_point.point_type {
			if let Some(alignment) = alignment_state
				.alignments
				.get_mut(&intermediate_point.alignment_id)
			{
				if let Some(segment) = alignment.segments.get_mut(segment_index) {
					segment.tangent_vertex = transform.translation;
				}
			}
		}
	}
}

pub(crate) fn update_pins_from_alignment_state(
	alignment_state: Res<AlignmentState>,
	mut alignment_pins: Query<(&mut Transform, &AlignmentPoint)>,
) {
	if let Some(alignment) = alignment_state.alignments.values().next() {
		if alignment.start != Vec3::ZERO || alignment.end != Vec3::ZERO {
			for (mut transform, alignment_point) in &mut alignment_pins {
				if alignment_point.alignment_id == alignment_state.turns {
					match alignment_point.point_type {
						PointType::Start => {
							transform.translation = alignment.start;
						}
						PointType::End => {
							transform.translation = alignment.end;
						}
						PointType::Intermediate { .. } => {}
					}
				}
			}
		}
	}
}

/// Marker component for draft alignment pins (start point being placed)
#[derive(Component)]
pub(crate) struct DraftAlignmentPin;

/// System that handles clicking on terrain to place the initial point of a track segment.
/// Only active when track building mode is enabled and no draft alignment start point exists.
pub(crate) fn place_initial_point(
	mut commands: Commands,
	mouse_button: Res<ButtonInput<MouseButton>>,
	track_building_mode: Res<TrackBuildingMode>,
	mut draft_alignment: ResMut<DraftAlignment>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	camera_query: Single<Entity, With<PrimaryCamera3d>>,
	mut egui_contexts: bevy_egui::EguiContexts,
	existing_draft_pins: Query<Entity, With<DraftAlignmentPin>>,
) {
	// Only handle clicks when track building mode is active
	if !track_building_mode.active {
		// Clean up draft state when exiting track building mode
		if draft_alignment.start.is_some() {
			draft_alignment.start = None;
			for entity in existing_draft_pins.iter() {
				commands.entity(entity).despawn();
			}
		}
		return;
	}

	// Only place initial point if we haven't placed one yet
	if draft_alignment.start.is_some() {
		return;
	}

	// Check for left mouse button click
	if !mouse_button.just_pressed(MouseButton::Left) {
		return;
	}

	// Don't place points when clicking on egui UI
	if let Ok(ctx) = egui_contexts.ctx_mut() {
		if ctx.wants_pointer_input() || ctx.is_pointer_over_area() {
			return;
		}
	}

	let camera_entity = *camera_query;
	let terrain_entity = *terrain_mesh;
	let heightmap = *terrain_heightmap;

	// Find the ray for the mouse pointer
	let Some(ray) = ray_map
		.iter()
		.find(|(ray_id, _)| ray_id.pointer == PointerId::Mouse && ray_id.camera == camera_entity)
		.map(|(_, ray)| *ray)
	else {
		return;
	};

	// Raycast to terrain
	let filter = |entity: Entity| entity == terrain_entity;
	let raycast_settings = MeshRayCastSettings::default().with_filter(&filter);
	let hits = raycast.cast_ray(ray, &raycast_settings);

	let Some(hit_point) = hits
		.iter()
		.find(|(entity, _)| *entity == terrain_entity)
		.map(|(_, hit)| hit.point)
	else {
		return;
	};

	// Calculate proper terrain height at hit point
	let terrain_height = calculate_terrain_height(hit_point, &heightmap, &settings);
	let start_position = Vec3::new(hit_point.x, terrain_height, hit_point.z);

	// Store the start position in draft alignment
	draft_alignment.start = Some(start_position);

	// Create a visual pin at the start position
	let world_size = world_size_for_height(&settings);
	let normalized_pos = start_position / world_size;

	let start_point = AlignmentPoint {
		alignment_id: usize::MAX, // Use MAX as sentinel for draft
		point_type: PointType::Start,
	};
	let start_color = start_point.get_color();

	commands.queue(create_draft_pin(
		normalized_pos,
		world_size,
		start_point,
		start_color,
	));
}

/// Creates a draft pin (similar to regular pin but with DraftAlignmentPin marker)
fn create_draft_pin(
	initial_position: Vec3,
	world_size: f32,
	point_id: AlignmentPoint,
	pinhead_color: Color,
) -> impl Command {
	use bevy::{gltf::GltfAssetLabel, render::render_resource::Face};
	use crate::pin::Pin;

	move |world: &mut World| {
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

		world
			.spawn((
				Pin,
				DraftAlignmentPin,
				point_id,
				bevy::picking::Pickable::default(),
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
