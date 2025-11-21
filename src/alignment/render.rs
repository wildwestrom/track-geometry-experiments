use alignment_path::{CurveSegment, HeightSampler, calculate_alignment_geometry};
use bevy::color::palettes::css::*;
use bevy::prelude::*;

use crate::terrain::{self, calculate_terrain_height};

use super::GeometryDebugLevel;
use super::components::{AlignmentGizmos, AlignmentPoint, PointType};
use super::state::AlignmentState;

const CURVE_RESOLUTION: u32 = 16;

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
) {
	let geometry_debug_level = geometry_debug_level.0;

	let (start, end) = match get_start_and_end_points(&alignment_state, alignment_pins) {
		Some(value) => value,
		None => return,
	};

	if let Some(alignment) = alignment_state.alignments.get(&alignment_state.turns) {
		let sampler = TerrainHeightSampler {
			heightmap: &terrain_heightmap,
			settings: &terrain_settings,
		};
		let alignment_geometry = calculate_alignment_geometry(start, end, alignment, &sampler);

		let mut c_i_minus_1 = None;
		for (index, segment) in alignment_geometry.segments.iter().enumerate() {
			if geometry_debug_level >= 3 {
				debug_angles(&mut gizmos, segment);
			}

			if geometry_debug_level >= 2 {
				gizmos.sphere(
					Isometry3d::from_translation(segment.ingoing_clothoid_start),
					10.0,
					GRAY,
				);
			}

			let ingoing_params = segment.ingoing_clothoid;
			let ingoing_clothoid =
				FunctionCurve::new(Interval::UNIT, move |s| ingoing_params.point_at(s));
			draw_ingoing_clothoid(&mut gizmos, ingoing_clothoid);

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

			if geometry_debug_level >= 1 {
				if let Some(previous_transition_end) = c_i_minus_1 {
					gizmos.line(
						previous_transition_end,
						segment.ingoing_clothoid_start,
						AQUA,
					);
				} else {
					gizmos.line(
						segment.tangent_vertex_prev,
						segment.ingoing_clothoid_start,
						AQUA,
					);
				}

				if index == alignment_geometry.segments.len().saturating_sub(1) {
					gizmos.line(
						segment.outgoing_clothoid_end,
						segment.tangent_vertex_next,
						AQUA,
					);
				}
			}
			c_i_minus_1 = Some(segment.outgoing_clothoid_end);

			let outgoing_params = segment.outgoing_clothoid;
			let outgoing_clothoid =
				FunctionCurve::new(Interval::UNIT, move |s| outgoing_params.point_at(s));
			draw_outgoint_clothoid(&mut gizmos, outgoing_clothoid);
		}
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
		if alignment_point.alignment_id == alignment_state.turns {
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
