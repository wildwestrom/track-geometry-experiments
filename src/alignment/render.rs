use std::f64::consts::PI;

use bevy::color::palettes::css::*;
use bevy::math::ops::atan2;
use bevy::prelude::*;
use spec_math::Fresnel;

use super::GeometryDebugLevel;
use super::components::{AlignmentGizmos, AlignmentPoint, PointType};
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
				let circular_section_radius_i = curve_params_i.radius;
				let circular_section_angle_i = curve_params_i.angle;

				let vertex_i_minus_1 = vertices[i - 1];
				let tangent_vertex_i_minus_1 = vertex_i_minus_1.pos;
				let azimuth_of_tangent_i = azimuth_of_tangent(tangent_vertex_i, tangent_vertex_i_minus_1);
				let azimuth_of_tangent_i_plus_1 =
					azimuth_of_tangent(tangent_vertex_i_plus_1, tangent_vertex_i);
				let difference_in_azimuth_i =
					difference_in_azimuth(azimuth_of_tangent_i, azimuth_of_tangent_i_plus_1);
				let length_of_circular_section = circular_section_length(
					circular_section_radius_i,
					circular_section_angle_i,
					difference_in_azimuth_i,
				);

				let unit_vector_i = unit_vector(tangent_vertex_i, tangent_vertex_i_minus_1);

				if geometry_debug_level >= 3 {
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

				let total_tangent_length_i = total_tangent_length(
					circular_section_radius_i,
					circular_section_angle_i,
					difference_in_azimuth_i,
					length_of_circular_section,
				);

				let t_i = tangent_vertex_i - total_tangent_length_i * unit_vector_i;

				if geometry_debug_level >= 2 {
					gizmos.sphere(
						Isometry3d::from_translation(t_i),
						10.0,
						Color::srgb(1.0, 1.0, 0.0),
					);
				}

				let r_i_abs = f64::from(circular_section_radius_i.abs());
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
						t_i,
						l_c_abs,
						f64::from(unit_vector_i.z.atan2(unit_vector_i.x)),
						fresnel_scale,
						fresnel_scale_sign,
					)
				});

				if geometry_debug_level >= 1 {
					gizmos.curve_3d(
						ingoing_clothoid,
						(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
						MAGENTA,
					);
				}

				let f_i = f_i(
					t_i,
					l_c_abs,
					f64::from(unit_vector_i.z.atan2(unit_vector_i.x)),
					fresnel_scale,
					fresnel_scale_sign,
				);
				let clothoid_angle_change =
					lambda_i as f32 * (difference_in_azimuth_i - circular_section_angle_i) / 2.0;
				let clothoid_end_tangent_angle =
					unit_vector_i.z.atan2(unit_vector_i.x) + clothoid_angle_change;

				let w_i = w_i(lambda_i as f32, clothoid_end_tangent_angle);
				let o_i = o_i(circular_section_radius_i, f_i, w_i);
				let start_vector = f_i - o_i;
				let alpha_i = alpha_i(start_vector);

				let arc_sweep = -lambda_i.signum() as f32 * circular_section_angle_i;
				if geometry_debug_level >= 1 {
					gizmos
						.arc_3d(
							arc_sweep,
							circular_section_radius_i,
							Isometry3d::new(o_i, Quat::from_axis_angle(Vec3::Y, -alpha_i)),
							GREEN_YELLOW,
						)
						.resolution(CURVE_RESOLUTION);
				}

				let arc_end_point = {
					let start_vector_from_center = f_i - o_i;
					let rotation = Quat::from_axis_angle(Vec3::Y, arc_sweep);
					o_i + rotation * start_vector_from_center
				};

				let c_i = tangent_vertex_i + total_tangent_length_i * unit_vector_i_plus_1;

				if geometry_debug_level >= 2 {
					gizmos.sphere(Isometry3d::from_translation(arc_end_point), 8.0, YELLOW);
					gizmos.sphere(Isometry3d::from_translation(f_i), 8.0, YELLOW);
					gizmos.sphere(Isometry3d::from_translation(c_i), 10.0, YELLOW);
				}

				if geometry_debug_level >= 1 {
					if let Some(c_i_minus_1) = c_i_minus_1 {
						gizmos.line(c_i_minus_1, t_i, AQUA);
					} else {
						gizmos.line(tangent_vertex_i_minus_1, t_i, AQUA);
					}
					if i == vertices.len() - 2 {
						gizmos.line(c_i, tangent_vertex_i_plus_1, AQUA);
					}
				}
				c_i_minus_1 = Some(c_i);

				let outgoing_clothoid = FunctionCurve::new(Interval::UNIT, |s| {
					clothoid_point(
						f64::from(-s),
						c_i,
						l_c_abs,
						f64::from(unit_vector_i_plus_1.z.atan2(unit_vector_i_plus_1.x)),
						fresnel_scale,
						-fresnel_scale_sign,
					)
				});

				if geometry_debug_level >= 1 {
					gizmos.curve_3d(
						outgoing_clothoid,
						(0..=CURVE_RESOLUTION).map(|i| i as f32 / CURVE_RESOLUTION as f32),
						MAGENTA,
					);
				}
			}
		}
	}
}

fn clothoid_point(
	s: f64,
	clothoid_endpoint: Vec3,
	l_c_abs: f64,
	beta_i: f64,
	fresnel_scale: f64,
	fresnel_scale_sign: f64,
) -> Vec3 {
	let tilde_s = s * l_c_abs;
	let fresnel_arg = tilde_s / fresnel_scale;
	let fresnel = fresnel_arg.fresnel();
	let i_x = (fresnel_scale
		* ((beta_i * fresnel_scale_sign).cos() * fresnel.c
			- (beta_i * fresnel_scale_sign).sin() * fresnel.s)) as f32;
	let i_z = (fresnel_scale_sign
		* fresnel_scale
		* ((beta_i * fresnel_scale_sign).sin() * fresnel.c
			+ (beta_i * fresnel_scale_sign).cos() * fresnel.s)) as f32;
	clothoid_endpoint + Vec3::new(i_x, 0.0, i_z)
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

fn alpha_i(start_vector: Vec3) -> f32 {
	start_vector.z.atan2(start_vector.x)
}

fn o_i(circular_section_radius_i: f32, f_i: Vec3, w_i: Vec3) -> Vec3 {
	f_i + circular_section_radius_i * w_i
}

fn w_i(lambda_i: f32, clothoid_end_tangent_angle: f32) -> Vec3 {
	if lambda_i > 0.0 {
		Vec3::new(
			-(clothoid_end_tangent_angle.sin()),
			0.0,
			clothoid_end_tangent_angle.cos(),
		)
	} else {
		Vec3::new(
			clothoid_end_tangent_angle.sin(),
			0.0,
			-(clothoid_end_tangent_angle.cos()),
		)
	}
}

fn f_i(t_i: Vec3, l_c_abs: f64, beta_i: f64, fresnel_scale: f64, fresnel_scale_sign: f64) -> Vec3 {
	let fresnel_arg = l_c_abs / fresnel_scale;
	let fresnel = fresnel_arg.fresnel();
	let i_x = (fresnel_scale
		* ((beta_i * fresnel_scale_sign).cos() * fresnel.c
			- (beta_i * fresnel_scale_sign).sin() * fresnel.s)) as f32;
	let i_z = (fresnel_scale_sign
		* fresnel_scale
		* ((beta_i * fresnel_scale_sign).sin() * fresnel.c
			+ (beta_i * fresnel_scale_sign).cos() * fresnel.s)) as f32;
	t_i + Vec3::new(i_x, 0.0, i_z)
}

fn total_tangent_length(
	circular_section_radius_i: f32,
	circular_section_angle_i: f32,
	difference_in_azimuth_i: f32,
	length_of_circular_section: f32,
) -> f32 {
	let theta_i_abs = f64::from(difference_in_azimuth_i.abs());
	let omega_i_abs = f64::from(circular_section_angle_i.abs());
	let r_i_abs = f64::from(circular_section_radius_i.abs());
	let l_c_abs = f64::from(length_of_circular_section.abs());
	let clothoid_angle = theta_i_abs - omega_i_abs;

	let fresnel_arg = (l_c_abs / (PI * r_i_abs)).sqrt();
	let fresnel_scale = (PI * r_i_abs * l_c_abs).sqrt();

	let fresnel = fresnel_arg.fresnel();
	let pf_i = fresnel_scale * fresnel.s;
	let tp_i = fresnel_scale * fresnel.c;

	let cos_half_clothoid_angle = (clothoid_angle / 2.0).cos();
	let sin_half_omega = (omega_i_abs / 2.0).sin();
	let sin_half_interior_angle = ((PI - theta_i_abs) / 2.0).sin();

	let ph_i = pf_i * (clothoid_angle / 2.0).tan();
	let hv_i =
		(r_i_abs + pf_i / cos_half_clothoid_angle) * (sin_half_omega / sin_half_interior_angle);

	let total_tangent_length: f32 = (tp_i + ph_i + hv_i) as f32;
	total_tangent_length
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

fn unit_vector(tangent_vertex_i: Vec3, tangent_vertex_i_minus_1: Vec3) -> Vec3 {
	(tangent_vertex_i - tangent_vertex_i_minus_1).normalize()
}

fn circular_section_length(
	circular_section_radius_i: f32,
	circular_section_angle_i: f32,
	difference_in_azimuth_i: f32,
) -> f32 {
	circular_section_radius_i * (difference_in_azimuth_i - circular_section_angle_i)
}

fn difference_in_azimuth(azimuth_of_tangent_i: f32, azimuth_of_tangent_i_plus_1: f32) -> f32 {
	use std::f32::consts::PI;
	let mut diff = azimuth_of_tangent_i_plus_1 - azimuth_of_tangent_i;
	if diff < 0.0 {
		diff += 2.0 * PI;
	}
	if diff > PI {
		diff = 2_f32.mul_add(PI, -diff);
	}
	diff
}

fn azimuth_of_tangent(tangent_vertex_i: Vec3, tangent_vertex_i_minus_1: Vec3) -> f32 {
	let delta_x = tangent_vertex_i.x - tangent_vertex_i_minus_1.x;
	let delta_z = tangent_vertex_i.z - tangent_vertex_i_minus_1.z;
	let angle = atan2(delta_z, delta_x);
	-angle
}
