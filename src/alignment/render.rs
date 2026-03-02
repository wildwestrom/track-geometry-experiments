use alignment_path::{CurveSegment, GeometrySegment, HeightSampler, calculate_alignment_geometry};
use bevy::color::palettes::css::*;
use bevy::picking::{
	backend::ray::RayMap,
	mesh_picking::ray_cast::{MeshRayCast, MeshRayCastSettings},
	pointer::PointerId,
};
use bevy::prelude::*;

use crate::camera::PrimaryCamera3d;
use crate::terrain::{self, calculate_terrain_height};

use super::GeometryDebugLevel;
use super::components::{AlignmentGizmos, AlignmentPoint, PointType};
use super::state::{
	AlignmentState, DraftAlignment, TangentSnapSettings, TrackBuildingMode, build_preview_alignment,
	snapped_segment_end_with_lock, snapped_tangent_direction_with_lock,
};
use crate::terrain::{HeightMap, TerrainMesh};

const CURVE_RESOLUTION: u32 = 16;
const TANGENT_RAY_DASH_LENGTH: f32 = 16.0;
const TANGENT_RAY_GAP_LENGTH: f32 = 10.0;
const TANGENT_RAY_EXTENT_MULTIPLIER: f32 = 12.0;
const TANGENT_RAY_MIN_LENGTH: f32 = 8_000.0;
const TANGENT_RAY_COLOR: Color = Color::srgba(0.22, 1.0, 0.08, 0.7);

struct TerrainHeightSampler<'a> {
	heightmap: &'a terrain::HeightMap,
	settings: &'a terrain::Settings,
}

impl<'a> HeightSampler for TerrainHeightSampler<'a> {
	fn height_at(&self, position: Vec3) -> f32 {
		calculate_terrain_height(position, self.heightmap, self.settings)
	}
}

pub(crate) fn render_alignment_path(
	mut gizmos: Gizmos<AlignmentGizmos>,
	alignment_state: Res<AlignmentState>,
	alignment_pins: Query<(&Transform, &AlignmentPoint)>,
	geometry_debug_level: Res<GeometryDebugLevel>,
	terrain_heightmap: Single<&terrain::HeightMap>,
	terrain_settings: Res<terrain::Settings>,
	track_building_mode: Res<TrackBuildingMode>,
	draft_alignment: Res<DraftAlignment>,
	snap_settings: Res<TangentSnapSettings>,
	terrain_mesh: Single<Entity, With<TerrainMesh>>,
	ray_map: Res<RayMap>,
	mut raycast: MeshRayCast,
	camera_query: Single<Entity, With<PrimaryCamera3d>>,
) {
	let geometry_debug_level = geometry_debug_level.0;
	let heightmap = *terrain_heightmap;
	let sampler = TerrainHeightSampler {
		heightmap: &heightmap,
		settings: &terrain_settings,
	};
	let hide_current_alignment = track_building_mode.active
		&& draft_alignment.start.is_some()
		&& draft_alignment.active_alignment_id.is_none();

	if !hide_current_alignment {
		if let Some((start, end)) = get_start_and_end_points(&alignment_state, alignment_pins)
			&& let Some(alignment) = alignment_state
				.alignments
				.get(&alignment_state.current_alignment)
		{
			draw_alignment_geometry(
				&mut gizmos,
				start,
				end,
				alignment,
				geometry_debug_level,
				&sampler,
			);
		}
	}

	if !track_building_mode.active {
		return;
	}

	let Some(preview_start) = draft_alignment.start else {
		return;
	};
	let Some(cursor_position) = cursor_terrain_position(
		*camera_query,
		*terrain_mesh,
		heightmap,
		&terrain_settings,
		&ray_map,
		&mut raycast,
	) else {
		return;
	};
	let (mut preview_end, snap_active) = snapped_segment_end_with_lock(
		preview_start,
		cursor_position,
		draft_alignment.previous_tangent,
		draft_alignment.tangent_snap_locked,
		*snap_settings,
	);
	if preview_end.x != cursor_position.x || preview_end.z != cursor_position.z {
		preview_end.y = calculate_terrain_height(preview_end, heightmap, &terrain_settings);
	}
	if snap_active
		&& let Some(tangent_direction) = snapped_tangent_direction_with_lock(
			preview_start,
			cursor_position,
			draft_alignment.previous_tangent,
			draft_alignment.tangent_snap_locked,
			*snap_settings,
		) {
		draw_dashed_tangent_ray(
			&mut gizmos,
			preview_start,
			tangent_direction,
			terrain_settings.world_x().max(terrain_settings.world_z()),
		);
	}

	let mut preview_alignment = build_preview_alignment(
		preview_start,
		preview_end,
		draft_alignment.previous_tangent,
		*snap_settings,
	);
	alignment_path::constraints::enforce_alignment_constraints(&mut preview_alignment);
	draw_alignment_geometry(
		&mut gizmos,
		preview_start,
		preview_end,
		&preview_alignment,
		geometry_debug_level,
		&sampler,
	);
}

fn cursor_terrain_position(
	camera_entity: Entity,
	terrain_entity: Entity,
	heightmap: &HeightMap,
	terrain_settings: &terrain::Settings,
	ray_map: &RayMap,
	raycast: &mut MeshRayCast,
) -> Option<Vec3> {
	let ray = ray_map
		.iter()
		.find(|(ray_id, _)| ray_id.pointer == PointerId::Mouse && ray_id.camera == camera_entity)
		.map(|(_, ray)| *ray)?;

	let filter = |entity: Entity| entity == terrain_entity;
	let raycast_settings = MeshRayCastSettings::default().with_filter(&filter);
	let hits = raycast.cast_ray(ray, &raycast_settings);

	let hit_point = hits
		.iter()
		.find(|(entity, _)| *entity == terrain_entity)
		.map(|(_, hit)| hit.point)?;

	let terrain_height = calculate_terrain_height(hit_point, heightmap, terrain_settings);
	Some(Vec3::new(hit_point.x, terrain_height, hit_point.z))
}

fn draw_alignment_geometry<H: HeightSampler>(
	gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>,
	start: Vec3,
	end: Vec3,
	alignment: &alignment_path::Alignment,
	geometry_debug_level: u8,
	sampler: &H,
) {
	let alignment_geometry = calculate_alignment_geometry(start, end, alignment, sampler);

	// Degenerate fallback when the geometry pipeline has no drawable pieces.
	if alignment_geometry.segments.is_empty() && geometry_debug_level >= 1 {
		gizmos.line(start, end, AQUA);
		return;
	}

	for segment in alignment_geometry.segments.iter() {
		if let GeometrySegment::Straight(straight) = segment {
			if geometry_debug_level >= 1 {
				gizmos.line(straight.start, straight.end, AQUA);
			}
			continue;
		}

		let GeometrySegment::Turn(segment) = segment else {
			continue;
		};
		if geometry_debug_level >= 3 {
			debug_angles(gizmos, segment);
		}

		if geometry_debug_level >= 2 {
			gizmos.sphere(
				Isometry3d::from_translation(segment.ingoing_clothoid_start),
				10.0,
				GRAY,
			);
		}

		let ingoing_params = segment.ingoing_clothoid;
		let ingoing_clothoid = FunctionCurve::new(Interval::UNIT, move |s| ingoing_params.point_at(s));
		draw_ingoing_clothoid(gizmos, ingoing_clothoid);

		if geometry_debug_level >= 1 {
			let arc_geometry = segment.circular_arc;
			let arc_function = FunctionCurve::new(Interval::UNIT, move |s| arc_geometry.point_at(s));

			gizmos.curve_3d(
				arc_function,
				(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
				GREEN_YELLOW,
			);
		}

		if geometry_debug_level >= 2 {
			gizmos.sphere(
				Isometry3d::from_translation(segment.circular_arc.end_point),
				8.0,
				YELLOW,
			);
			gizmos.sphere(
				Isometry3d::from_translation(segment.circular_arc.start_point),
				8.0,
				YELLOW,
			);
			gizmos.sphere(
				Isometry3d::from_translation(segment.outgoing_clothoid_end),
				10.0,
				STEEL_BLUE,
			);
		}

		let outgoing_params = segment.outgoing_clothoid;
		let outgoing_clothoid =
			FunctionCurve::new(Interval::UNIT, move |s| outgoing_params.point_at(s));
		draw_outgoint_clothoid(gizmos, outgoing_clothoid);
	}
}

fn draw_outgoint_clothoid(
	gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>,
	outgoing_clothoid: FunctionCurve<Vec3, impl Fn(f32) -> Vec3>,
) {
	gizmos.curve_3d(
		outgoing_clothoid,
		(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
		MAGENTA,
	);
}

fn draw_ingoing_clothoid(
	gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>,
	ingoing_clothoid: FunctionCurve<Vec3, impl Fn(f32) -> Vec3>,
) {
	gizmos.curve_3d(
		ingoing_clothoid,
		(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
		MAGENTA,
	);
}

fn draw_dashed_tangent_ray(
	gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>,
	origin: Vec3,
	direction: Vec3,
	world_size: f32,
) {
	let ray_length = (world_size * TANGENT_RAY_EXTENT_MULTIPLIER).max(TANGENT_RAY_MIN_LENGTH);
	let step = TANGENT_RAY_DASH_LENGTH + TANGENT_RAY_GAP_LENGTH;
	let mut distance = 0.0;
	while distance < ray_length {
		let dash_start = origin + direction * distance;
		let dash_end = origin + direction * (distance + TANGENT_RAY_DASH_LENGTH).min(ray_length);
		gizmos.line(dash_start, dash_end, TANGENT_RAY_COLOR);
		distance += step;
	}
}

fn debug_angles(gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>, segment: &CurveSegment) {
	gizmos.arc_3d(
		segment.azimuth_of_tangent,
		150.0,
		Isometry3d::new(segment.tangent_vertex, Quat::from_axis_angle(Vec3::Y, 0.)),
		Color::srgb(0.9, 1.0, 0.2),
	);

	gizmos.line(
		segment.tangent_vertex,
		segment.tangent_vertex + Vec3::ZERO.with_x(175.0),
		Color::srgb(1.0, 0.8, 0.4),
	);

	gizmos.arc_3d(
		segment.difference_in_azimuth,
		200.0,
		Isometry3d::new(
			segment.tangent_vertex,
			Quat::from_axis_angle(Vec3::Y, segment.azimuth_of_tangent),
		),
		Color::srgb(0.6, 0.0, 1.0),
	);
}

fn get_start_and_end_points(
	alignment_state: &Res<'_, AlignmentState>,
	alignment_pins: Query<'_, '_, (&Transform, &AlignmentPoint)>,
) -> Option<(Vec3, Vec3)> {
	let mut start = None;
	let mut end = None;
	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == alignment_state.current_alignment {
			match alignment_point.point_type {
				PointType::Start => start = Some(transform.translation),
				PointType::End => end = Some(transform.translation),
				PointType::Intermediate { .. } => {}
			}
		}
	}
	let (Some(start), Some(end)) = (start, end) else {
		return None;
	};
	if !start.is_finite() || !end.is_finite() || start == end {
		return None;
	}
	Some((start, end))
}
