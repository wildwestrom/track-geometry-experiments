use alignment_path::{PathSegment, constraints as path_constraints};
use bevy::{
	color::palettes::css::YELLOW,
	picking::{
		backend::ray::RayMap,
		hover::PickingInteraction,
		mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
		pointer::PointerId,
	},
	prelude::*,
};

use crate::camera::PrimaryCamera3d;

use crate::pin::{PinDragState, create_pin};
use crate::terrain::{self, HeightMap, TerrainMesh, calculate_terrain_height};
use terrain::spatial::world_size_for_height;

use super::components::{AlignmentPoint, PointType};
use super::state::{
	AlignmentState, DraftAlignment, TangentSnapSettings, TrackBuildingMode, alignment_end_tangent,
	build_preview_alignment, extend_alignment_with_preview, snapped_segment_end_with_lock,
};

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
	alignment_pins: Query<
		(&Transform, &AlignmentPoint),
		(Changed<Transform>, Without<DraftAlignmentPin>),
	>,
	mut alignment_state: ResMut<AlignmentState>,
	track_building_mode: Res<TrackBuildingMode>,
) {
	if track_building_mode.active {
		return;
	}

	let current_id = alignment_state.current_alignment;
	let mut start_pos = None;
	let mut end_pos = None;

	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == current_id {
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

	// Only update the current alignment, not all alignments
	if let Some(alignment) = alignment_state.alignments.get_mut(&current_id) {
		if alignment.start != new_start || alignment.end != new_end {
			alignment.start = new_start;
			alignment.end = new_end;
		}
	}
}

pub(crate) fn update_alignment_pins(
	mut commands: Commands,
	alignment_state: Res<AlignmentState>,
	existing_pins: Query<Entity, (With<AlignmentPoint>, Without<DraftAlignmentPin>)>,
	settings: Res<terrain::Settings>,
	track_building_mode: Res<TrackBuildingMode>,
	draft_alignment: Res<DraftAlignment>,
	mut last_current_alignment: Local<Option<usize>>,
	mut last_segment_count: Local<Option<usize>>,
	mut last_has_current_alignment: Local<Option<bool>>,
	mut last_hide_current_alignment: Local<Option<bool>>,
) {
	let current_alignment = alignment_state.current_alignment;
	let current_alignment_data = alignment_state.alignments.get(&current_alignment);
	let has_current_alignment = current_alignment_data.is_some();
	let current_segment_count = current_alignment_data
		.map(|alignment| alignment.segments.len())
		.unwrap_or_default();
	let hide_current_alignment = track_building_mode.active
		&& draft_alignment.start.is_some()
		&& draft_alignment.active_alignment_id.is_none();

	if *last_current_alignment == Some(current_alignment)
		&& *last_segment_count == Some(current_segment_count)
		&& *last_has_current_alignment == Some(has_current_alignment)
		&& *last_hide_current_alignment == Some(hide_current_alignment)
	{
		return;
	}
	*last_current_alignment = Some(current_alignment);
	*last_segment_count = Some(current_segment_count);
	*last_has_current_alignment = Some(has_current_alignment);
	*last_hide_current_alignment = Some(hide_current_alignment);

	for entity in existing_pins.iter() {
		commands.entity(entity).despawn();
	}

	if hide_current_alignment {
		return;
	}

	if let Some(alignment) = current_alignment_data {
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

		for (i, _) in alignment.segments.iter().enumerate() {
			let Some(control_point) = alignment.segment_control_point(i) else {
				continue;
			};
			let normalized_pos = control_point / world_size;
			let alignment_point = AlignmentPoint {
				alignment_id: current_alignment,
				point_type: PointType::Intermediate { segment_index: i },
			};
			let point_color = if matches!(alignment.segments.get(i), Some(PathSegment::Straight(_))) {
				Color::Srgba(YELLOW)
			} else {
				alignment_point.get_color()
			};
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
	mut intermediate_pins: Query<
		(&mut Transform, &AlignmentPoint),
		(Changed<Transform>, Without<DraftAlignmentPin>),
	>,
	mut alignment_state: ResMut<AlignmentState>,
	track_building_mode: Res<TrackBuildingMode>,
) {
	if track_building_mode.active {
		return;
	}

	let current_id = alignment_state.current_alignment;

	for (mut transform, intermediate_point) in intermediate_pins.iter_mut() {
		if intermediate_point.alignment_id != current_id {
			continue;
		}

		if let PointType::Intermediate { segment_index } = intermediate_point.point_type {
			if let Some(alignment) = alignment_state
				.alignments
				.get_mut(&intermediate_point.alignment_id)
			{
				let had_control_point = alignment.segment_control_point(segment_index);
				if had_control_point
					.is_some_and(|point| point.distance_squared(transform.translation) > f32::EPSILON)
				{
					alignment.set_segment_control_point(segment_index, transform.translation);
				}
				// Keep straight-section pins visually snapped to their tangent span even while dragging.
				if let Some(snapped_control_point) = alignment.segment_control_point(segment_index) {
					if snapped_control_point.distance_squared(transform.translation) > f32::EPSILON {
						transform.translation = snapped_control_point;
					}
				}
			}
		}
	}
}

pub(crate) fn update_pins_from_alignment_state(
	alignment_state: Res<AlignmentState>,
	drag_state: Res<PinDragState>,
	mut alignment_pins: Query<(
		Entity,
		&mut Transform,
		&AlignmentPoint,
		Option<&PickingInteraction>,
	)>,
) {
	let current_id = alignment_state.current_alignment;
	if let Some(alignment) = alignment_state.alignments.get(&current_id) {
		if alignment.start != Vec3::ZERO || alignment.end != Vec3::ZERO {
			for (entity, mut transform, alignment_point, interaction) in &mut alignment_pins {
				if alignment_point.alignment_id == current_id {
					if drag_state.is_dragging(entity)
						|| matches!(interaction, Some(PickingInteraction::Pressed))
					{
						continue;
					}

					match alignment_point.point_type {
						PointType::Start => {
							transform.translation = alignment.start;
						}
						PointType::End => {
							transform.translation = alignment.end;
						}
						PointType::Intermediate { segment_index } => {
							if let Some(control_point) = alignment.segment_control_point(segment_index) {
								transform.translation = control_point;
							}
						}
					}
				}
			}
		}
	}
}

pub(crate) fn update_draft_cursor_pin(
	mut commands: Commands,
	track_building_mode: Res<TrackBuildingMode>,
	mut draft_alignment: ResMut<DraftAlignment>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	snap_settings: Res<TangentSnapSettings>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	camera_query: Single<Entity, With<PrimaryCamera3d>>,
	mut draft_pins: Query<(Entity, &mut Transform, &AlignmentPoint), With<DraftAlignmentPin>>,
) {
	let mut cursor_pin_entity = None;
	for (entity, _, point) in draft_pins.iter_mut() {
		if point.alignment_id == usize::MAX && matches!(point.point_type, PointType::End) {
			cursor_pin_entity = Some(entity);
			break;
		}
	}

	let should_show = track_building_mode.active && draft_alignment.start.is_some();
	if !should_show {
		if let Some(entity) = cursor_pin_entity {
			commands.entity(entity).despawn();
		}
		return;
	}

	let camera_entity = *camera_query;
	let terrain_entity = *terrain_mesh;
	let heightmap = *terrain_heightmap;

	let Some(ray) = ray_map
		.iter()
		.find(|(ray_id, _)| ray_id.pointer == PointerId::Mouse && ray_id.camera == camera_entity)
		.map(|(_, ray)| *ray)
	else {
		if let Some(entity) = cursor_pin_entity {
			commands.entity(entity).despawn();
		}
		return;
	};

	let filter = |entity: Entity| entity == terrain_entity;
	let raycast_settings = MeshRayCastSettings::default().with_filter(&filter);
	let hits = raycast.cast_ray(ray, &raycast_settings);
	let Some(hit_point) = hits
		.iter()
		.find(|(entity, _)| *entity == terrain_entity)
		.map(|(_, hit)| hit.point)
	else {
		if let Some(entity) = cursor_pin_entity {
			commands.entity(entity).despawn();
		}
		return;
	};

	let terrain_height = calculate_terrain_height(hit_point, &heightmap, &settings);
	let raw_cursor_position = Vec3::new(hit_point.x, terrain_height, hit_point.z);
	let cursor_position = if let Some(start) = draft_alignment.start {
		let (mut snapped, snap_active) = snapped_segment_end_with_lock(
			start,
			raw_cursor_position,
			draft_alignment.previous_tangent,
			draft_alignment.tangent_snap_locked,
			*snap_settings,
		);
		draft_alignment.tangent_snap_locked = snap_active;
		if snap_active && (snapped.x != raw_cursor_position.x || snapped.z != raw_cursor_position.z) {
			snapped.y = calculate_terrain_height(snapped, &heightmap, &settings);
		}
		snapped
	} else {
		draft_alignment.tangent_snap_locked = false;
		raw_cursor_position
	};

	if let Some(entity) = cursor_pin_entity {
		if let Ok((_, mut transform, _)) = draft_pins.get_mut(entity) {
			transform.translation = cursor_position;
		}
		return;
	}

	let world_size = world_size_for_height(&settings);
	let point = AlignmentPoint {
		alignment_id: usize::MAX,
		point_type: PointType::End,
	};
	let color = Color::srgb(0.22, 1.0, 0.08);
	commands.queue(create_draft_pin(
		cursor_position / world_size,
		world_size,
		point,
		color,
	));
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
			draft_alignment.previous_tangent = None;
			draft_alignment.tangent_snap_locked = false;
			draft_alignment.active_alignment_id = None;
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
	draft_alignment.previous_tangent = None;
	draft_alignment.tangent_snap_locked = false;
	draft_alignment.active_alignment_id = None;

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
	use crate::pin::Pin;
	use bevy::{gltf::GltfAssetLabel, render::render_resource::Face};

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

/// System that handles clicking to commit the first segment.
/// Only active when track building mode is enabled and a start point has been placed.
pub(crate) fn commit_first_segment(
	mut commands: Commands,
	mouse_button: Res<ButtonInput<MouseButton>>,
	track_building_mode: Res<TrackBuildingMode>,
	mut draft_alignment: ResMut<DraftAlignment>,
	mut alignment_state: ResMut<AlignmentState>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	terrain_heightmap: Single<&HeightMap>,
	settings: Res<terrain::Settings>,
	snap_settings: Res<TangentSnapSettings>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	camera_query: Single<Entity, With<PrimaryCamera3d>>,
	mut egui_contexts: bevy_egui::EguiContexts,
	existing_draft_pins: Query<Entity, With<DraftAlignmentPin>>,
) {
	// Only handle when in track building mode with a start point placed
	if !track_building_mode.active {
		return;
	}

	let Some(start_position) = draft_alignment.start else {
		return;
	};

	// Check for left mouse button click
	if !mouse_button.just_pressed(MouseButton::Left) {
		return;
	}

	// Don't commit when clicking on egui UI
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

	// Calculate proper terrain height at end point
	let terrain_height = calculate_terrain_height(hit_point, &heightmap, &settings);
	let raw_end_position = Vec3::new(hit_point.x, terrain_height, hit_point.z);
	let (mut end_position, snap_active) = snapped_segment_end_with_lock(
		start_position,
		raw_end_position,
		draft_alignment.previous_tangent,
		draft_alignment.tangent_snap_locked,
		*snap_settings,
	);
	draft_alignment.tangent_snap_locked = snap_active;
	if end_position.x != raw_end_position.x || end_position.z != raw_end_position.z {
		end_position.y = calculate_terrain_height(end_position, &heightmap, &settings);
	}

	let (current_alignment_id, committed_end_tangent) =
		if let Some(active_alignment_id) = draft_alignment.active_alignment_id {
			if let Some(active_alignment) = alignment_state.alignments.get_mut(&active_alignment_id) {
				extend_alignment_with_preview(
					active_alignment,
					start_position,
					end_position,
					draft_alignment.previous_tangent,
					*snap_settings,
				);
				(
					active_alignment_id,
					alignment_end_tangent(start_position, end_position, active_alignment),
				)
			} else {
				let mut new_alignment = build_preview_alignment(
					start_position,
					end_position,
					draft_alignment.previous_tangent,
					*snap_settings,
				);
				path_constraints::enforce_alignment_constraints(&mut new_alignment);
				let new_alignment_id = alignment_state.next_alignment_id;
				alignment_state
					.alignments
					.insert(new_alignment_id, new_alignment.clone());
				alignment_state.next_alignment_id += 1;
				draft_alignment.active_alignment_id = Some(new_alignment_id);
				(
					new_alignment_id,
					alignment_end_tangent(start_position, end_position, &new_alignment),
				)
			}
		} else {
			let mut new_alignment = build_preview_alignment(
				start_position,
				end_position,
				draft_alignment.previous_tangent,
				*snap_settings,
			);
			path_constraints::enforce_alignment_constraints(&mut new_alignment);
			let new_alignment_id = alignment_state.next_alignment_id;
			alignment_state
				.alignments
				.insert(new_alignment_id, new_alignment.clone());
			alignment_state.next_alignment_id += 1;
			draft_alignment.active_alignment_id = Some(new_alignment_id);
			(
				new_alignment_id,
				alignment_end_tangent(start_position, end_position, &new_alignment),
			)
		};
	alignment_state.current_alignment = current_alignment_id;

	// Continue building from the end of the committed segment.
	draft_alignment.start = Some(end_position);
	draft_alignment.previous_tangent = committed_end_tangent;
	draft_alignment.tangent_snap_locked = false;
	for entity in existing_draft_pins.iter() {
		commands.entity(entity).despawn();
	}

	let world_size = world_size_for_height(&settings);
	let normalized_pos = end_position / world_size;
	let start_point = AlignmentPoint {
		alignment_id: usize::MAX,
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
