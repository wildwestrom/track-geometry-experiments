use std::f64::consts::PI;

use bevy::color::palettes::css::*;
use bevy::prelude::*;

use crate::alignment::geometry::{
	alpha_i, circular_arc_center, circular_arc_start, clothoid_point, unit_vector, w_i_vector,
};
use crate::terrain::{self, calculate_terrain_height};

use super::GeometryDebugLevel;
use super::components::{AlignmentGizmos, AlignmentPoint, PointType};
use super::geometry::{
	azimuth_of_tangent, circular_section_length, difference_in_azimuth, total_tangent_length,
};
use super::state::{Alignment, AlignmentState};

const CURVE_RESOLUTION: u32 = 16;

#[derive(Clone, Copy)]
struct CurveParams {
	radius: f32,
	angle: f32,
}

#[derive(Clone, Copy)]
struct WorkingVertex {
	pos: Vec3,
	params: Option<CurveParams>,
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
		let vertices = build_working_vertices(start, end, alignment);
		let mut c_i_minus_1 = None;
		for i in 0..vertices.len() - 1 {
			let gizmos: &mut Gizmos<'_, '_, AlignmentGizmos> = &mut gizmos;
			let vertices: &[WorkingVertex] = &vertices;
			let vertex_i = vertices[i];
			let vertex_i_plus_1 = vertices[i + 1];
			let tangent_vertex_i = vertex_i.pos;
			let tangent_vertex_i_plus_1 = vertex_i_plus_1.pos;
			let unit_vector_i_plus_1 = unit_vector(tangent_vertex_i_plus_1, tangent_vertex_i);

			if i > 0 {
				let Some(curve_params_i) = vertex_i.params else {
					continue;
				};
				let circular_arc_radius_i = curve_params_i.radius;
				let circular_arc_angle_i = curve_params_i.angle;

				let vertex_i_minus_1 = vertices[i - 1];
				let tangent_vertex_i_minus_1 = vertex_i_minus_1.pos;
				let azimuth_of_tangent_i = azimuth_of_tangent(tangent_vertex_i, tangent_vertex_i_minus_1);
				let azimuth_of_tangent_i_plus_1 =
					azimuth_of_tangent(tangent_vertex_i_plus_1, tangent_vertex_i);
				let difference_in_azimuth_i =
					difference_in_azimuth(azimuth_of_tangent_i, azimuth_of_tangent_i_plus_1);
				let length_of_circular_section = circular_section_length(
					circular_arc_radius_i,
					circular_arc_angle_i,
					difference_in_azimuth_i,
				);

				let unit_vector_i = unit_vector(tangent_vertex_i, tangent_vertex_i_minus_1);

				if geometry_debug_level >= 3 {
					debug_angles(
						i,
						gizmos,
						vertices,
						tangent_vertex_i,
						azimuth_of_tangent_i,
						difference_in_azimuth_i,
					);
				}

				let total_tangent_length_i = total_tangent_length(
					circular_arc_radius_i,
					circular_arc_angle_i,
					difference_in_azimuth_i,
					length_of_circular_section,
				);

				let ingoing_clothoid_start_point =
					tangent_vertex_i - total_tangent_length_i * unit_vector_i;

				if geometry_debug_level >= 2 {
					gizmos.sphere(
						Isometry3d::from_translation(ingoing_clothoid_start_point),
						10.0,
						GRAY,
					);
				}

				let r_i_abs = f64::from(circular_arc_radius_i.abs());
				let l_c_abs = f64::from(length_of_circular_section.abs());

				let cross_y = unit_vector_i.x.mul_add(
					unit_vector_i_plus_1.z,
					-(unit_vector_i.z * unit_vector_i_plus_1.x),
				);
				let lambda_i = if cross_y >= 0.0 { 1.0_f64 } else { -1.0_f64 };

				let inner = (PI * r_i_abs * l_c_abs) / lambda_i;
				let fresnel_scale = inner.abs().sqrt();
				let fresnel_scale_sign = inner.signum();

				let ingoing_clothoid = FunctionCurve::new(Interval::UNIT, |s| {
					clothoid_point(
						f64::from(s),
						ingoing_clothoid_start_point,
						l_c_abs,
						f64::from(unit_vector_i.z.atan2(unit_vector_i.x)),
						fresnel_scale,
						fresnel_scale_sign,
					)
				});

				draw_ingoing_clothoid(gizmos, ingoing_clothoid);

				let circular_arc_start = circular_arc_start(
					ingoing_clothoid_start_point,
					l_c_abs,
					f64::from(unit_vector_i.z.atan2(unit_vector_i.x)),
					fresnel_scale,
					fresnel_scale_sign,
				);
				let clothoid_angle_change =
					lambda_i as f32 * (difference_in_azimuth_i - circular_arc_angle_i) / 2.0;
				let clothoid_end_tangent_angle =
					unit_vector_i.z.atan2(unit_vector_i.x) + clothoid_angle_change;

				let w_i_vector = w_i_vector(lambda_i as f32, clothoid_end_tangent_angle);
				let circular_arc_center =
					circular_arc_center(circular_arc_radius_i, circular_arc_start, w_i_vector);
				let start_vector = circular_arc_start - circular_arc_center;
				let _alpha_i = alpha_i(start_vector);

				let arc_sweep = -lambda_i.signum() as f32 * circular_arc_angle_i;

				let arc_end_point = {
					let start_vector_from_center = circular_arc_start - circular_arc_center;
					let rotation = Quat::from_axis_angle(Vec3::Y, arc_sweep);
					let xz_pos = circular_arc_center + rotation * start_vector_from_center;
					let y_pos = calculate_terrain_height(xz_pos, &terrain_heightmap, &terrain_settings);
					Vec3::new(xz_pos.x, y_pos, xz_pos.z)
				};

				// draw circular arc
				// Custom arc: circular on xz plane with linear elevation interpolation
				if geometry_debug_level >= 1 {
					let start_elevation = circular_arc_start.y;
					let end_elevation = arc_end_point.y;

					let arc_function = FunctionCurve::new(Interval::UNIT, move |s| {
						// Rotate the start vector by the sweep angle
						let sweep_angle = arc_sweep * s;
						let rotation = Quat::from_axis_angle(Vec3::Y, sweep_angle);
						let rotated_vector = rotation * start_vector;

						let xz_position = circular_arc_center + rotated_vector;
						let interpolated_y = start_elevation * (1.0 - s) + end_elevation * s;

						Vec3::new(xz_position.x, interpolated_y, xz_position.z)
					});

					gizmos.curve_3d(
						arc_function,
						(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
						GREEN_YELLOW,
					);
				}

				let clothoid_transition_end =
					tangent_vertex_i + total_tangent_length_i * unit_vector_i_plus_1;

				if geometry_debug_level >= 2 {
					gizmos.sphere(Isometry3d::from_translation(arc_end_point), 8.0, YELLOW);
					gizmos.sphere(
						Isometry3d::from_translation(circular_arc_start),
						8.0,
						YELLOW,
					);
					gizmos.sphere(
						Isometry3d::from_translation(clothoid_transition_end),
						10.0,
						STEEL_BLUE,
					);
				}

				// draw lines
				if geometry_debug_level >= 1 {
					if let Some(c_i_minus_1) = c_i_minus_1 {
						gizmos.line(c_i_minus_1, ingoing_clothoid_start_point, AQUA);
					} else {
						gizmos.line(tangent_vertex_i_minus_1, ingoing_clothoid_start_point, AQUA);
					}
					if i == vertices.len() - 2 {
						gizmos.line(clothoid_transition_end, tangent_vertex_i_plus_1, AQUA);
					}
				}
				c_i_minus_1 = Some(clothoid_transition_end);

				let outgoing_clothoid = FunctionCurve::new(Interval::UNIT, |s| {
					clothoid_point(
						f64::from(-s),
						clothoid_transition_end,
						l_c_abs,
						f64::from(unit_vector_i_plus_1.z.atan2(unit_vector_i_plus_1.x)),
						fresnel_scale,
						-fresnel_scale_sign,
					)
				});

				draw_outgoint_clothoid(gizmos, outgoing_clothoid);
			}
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

fn debug_angles(
	i: usize,
	gizmos: &mut Gizmos<'_, '_, AlignmentGizmos>,
	vertices: &[WorkingVertex],
	tangent_vertex_i: Vec3,
	azimuth_of_tangent_i: f32,
	difference_in_azimuth_i: f32,
) {
	gizmos.arc_3d(
		azimuth_of_tangent_i,
		150.0,
		Isometry3d::new(vertices[i].pos, Quat::from_axis_angle(Vec3::Y, 0.)),
		Color::srgb(0.9, 1.0, 0.2),
	);

	gizmos.line(
		tangent_vertex_i,
		tangent_vertex_i + Vec3::ZERO.with_x(175.0),
		Color::srgb(1.0, 0.8, 0.4),
	);

	gizmos.arc_3d(
		difference_in_azimuth_i,
		200.0,
		Isometry3d::new(
			vertices[i].pos,
			Quat::from_axis_angle(Vec3::Y, azimuth_of_tangent_i),
		),
		Color::srgb(0.6, 0.0, 1.0),
	);
}

fn build_working_vertices(start: Vec3, end: Vec3, alignment: &Alignment) -> Vec<WorkingVertex> {
	let mut full = Vec::new();
	full.push(WorkingVertex {
		pos: start,
		params: None,
	});
	for seg in &alignment.segments {
		full.push(WorkingVertex {
			pos: seg.tangent_vertex,
			params: Some(CurveParams {
				radius: seg.circular_section_radius,
				angle: seg.circular_section_angle,
			}),
		});
	}
	full.push(WorkingVertex {
		pos: end,
		params: None,
	});
	assert!(full.len() >= 2, "Need at least start and end vertices");
	for (i, v) in full.iter().enumerate() {
		assert!(v.pos.is_finite(), "vertex {i} is not finite: {}", v.pos);
	}
	full
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
