use std::f64::consts::PI;

use glam::{Quat, Vec3};
use spec_math::Fresnel;

use crate::path::Alignment;

pub trait HeightSampler {
	fn height_at(&self, position: Vec3) -> f32;
}

// Compute azimuth of the tangent from previous point to current point
pub fn azimuth_of_tangent(current: Vec3, previous: Vec3) -> f32 {
	let delta_x = current.x - previous.x;
	let delta_z = current.z - previous.z;
	let angle = delta_z.atan2(delta_x);
	-angle
}

// Compute the minimal absolute difference between two azimuths in [0, PI]
pub fn difference_in_azimuth(azimuth_i: f32, azimuth_ip1: f32) -> f32 {
	use std::f32::consts::PI;
	let mut diff = azimuth_ip1 - azimuth_i;
	if diff < 0.0 {
		diff += 2.0 * PI;
	}
	if diff > PI {
		diff = 2_f32.mul_add(PI, -diff);
	}
	diff
}

pub fn circular_section_length(
	circular_section_radius_i: f32,
	circular_section_angle_i: f32,
	difference_in_azimuth_i: f32,
) -> f32 {
	circular_section_radius_i * (difference_in_azimuth_i - circular_section_angle_i)
}

pub fn total_tangent_length(
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

pub fn circular_arc_center(circular_section_radius_i: f32, f_i: Vec3, w_i: Vec3) -> Vec3 {
	f_i + circular_section_radius_i * w_i
}

pub fn w_i_vector(lambda_i: f32, clothoid_end_tangent_angle: f32) -> Vec3 {
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

pub fn circular_arc_start(
	t_i: Vec3,
	l_c_abs: f64,
	beta_i: f64,
	fresnel_scale: f64,
	fresnel_scale_sign: f64,
) -> Vec3 {
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

pub fn unit_vector(tangent_vertex_i: Vec3, tangent_vertex_i_minus_1: Vec3) -> Vec3 {
	(tangent_vertex_i - tangent_vertex_i_minus_1).normalize()
}

pub fn clothoid_point(
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

#[derive(Clone)]
pub struct AlignmentGeometry {
	pub segments: Vec<CurveSegment>,
}

#[derive(Clone, Copy)]
pub struct CurveSegment {
	pub tangent_vertex_prev: Vec3,
	pub tangent_vertex: Vec3,
	pub tangent_vertex_next: Vec3,
	pub ingoing_clothoid_start: Vec3,
	pub ingoing_clothoid: ClothoidParameters,
	pub circular_arc: CircularArcGeometry,
	pub outgoing_clothoid_end: Vec3,
	pub outgoing_clothoid: ClothoidParameters,
	pub azimuth_of_tangent: f32,
	pub difference_in_azimuth: f32,
}

#[derive(Clone, Copy)]
pub struct ClothoidParameters {
	pub endpoint: Vec3,
	pub length: f64,
	pub beta: f64,
	pub fresnel_scale: f64,
	pub fresnel_scale_sign: f64,
	pub s_multiplier: f64,
}

impl ClothoidParameters {
	pub fn point_at(&self, s: f32) -> Vec3 {
		clothoid_point(
			self.s_multiplier * f64::from(s),
			self.endpoint,
			self.length,
			self.beta,
			self.fresnel_scale,
			self.fresnel_scale_sign,
		)
	}
}

#[derive(Clone, Copy)]
pub struct CircularArcGeometry {
	pub start_point: Vec3,
	pub center: Vec3,
	pub start_vector: Vec3,
	pub arc_sweep: f32,
	pub start_elevation: f32,
	pub end_point: Vec3,
	pub end_elevation: f32,
}

impl CircularArcGeometry {
	pub fn point_at(&self, s: f32) -> Vec3 {
		let sweep_angle = self.arc_sweep * s;
		let rotation = Quat::from_axis_angle(Vec3::Y, sweep_angle);
		let rotated_vector = rotation * self.start_vector;
		let xz_position = self.center + rotated_vector;
		let interpolated_y = self.start_elevation * (1.0 - s) + self.end_elevation * s;
		Vec3::new(xz_position.x, interpolated_y, xz_position.z)
	}
}

pub fn calculate_alignment_geometry<H: HeightSampler>(
	start: Vec3,
	end: Vec3,
	alignment: &Alignment,
	height_sampler: &H,
) -> AlignmentGeometry {
	let mut segments = Vec::new();

	assert!(start.is_finite(), "start vertex must be finite: {start}");
	assert!(end.is_finite(), "end vertex must be finite: {end}");
	for (i, seg) in alignment.segments.iter().enumerate() {
		assert!(
			seg.tangent_vertex.is_finite(),
			"segment {i} vertex is not finite: {}",
			seg.tangent_vertex
		);
		assert!(
			seg.circular_section_radius.is_finite(),
			"segment {i} radius is not finite: {}",
			seg.circular_section_radius
		);
		assert!(
			seg.circular_section_angle.is_finite(),
			"segment {i} angle is not finite: {}",
			seg.circular_section_angle
		);
	}

	if alignment.segments.is_empty() {
		return AlignmentGeometry { segments };
	}

	for (i, segment) in alignment.segments.iter().enumerate() {
		let tangent_vertex_i_minus_1 = if i == 0 {
			start
		} else {
			alignment.segments[i - 1].tangent_vertex
		};
		let tangent_vertex_i = segment.tangent_vertex;
		let tangent_vertex_i_plus_1 = alignment
			.segments
			.get(i + 1)
			.map(|next| next.tangent_vertex)
			.unwrap_or(end);

		let circular_arc_radius_i = segment.circular_section_radius;
		let circular_arc_angle_i = segment.circular_section_angle;

		let unit_vector_i = unit_vector(tangent_vertex_i, tangent_vertex_i_minus_1);
		let unit_vector_i_plus_1 = unit_vector(tangent_vertex_i_plus_1, tangent_vertex_i);

		let azimuth_of_tangent_i = azimuth_of_tangent(tangent_vertex_i, tangent_vertex_i_minus_1);
		let azimuth_of_tangent_i_plus_1 = azimuth_of_tangent(tangent_vertex_i_plus_1, tangent_vertex_i);

		let difference_in_azimuth_i =
			difference_in_azimuth(azimuth_of_tangent_i, azimuth_of_tangent_i_plus_1);
		let length_of_circular_section = circular_section_length(
			circular_arc_radius_i,
			circular_arc_angle_i,
			difference_in_azimuth_i,
		);

		let total_tangent_length_i = total_tangent_length(
			circular_arc_radius_i,
			circular_arc_angle_i,
			difference_in_azimuth_i,
			length_of_circular_section,
		);

		let ingoing_clothoid_start_point = tangent_vertex_i - total_tangent_length_i * unit_vector_i;

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

		let ingoing_beta = f64::from(unit_vector_i.z.atan2(unit_vector_i.x));
		let ingoing_clothoid = ClothoidParameters {
			endpoint: ingoing_clothoid_start_point,
			length: l_c_abs,
			beta: ingoing_beta,
			fresnel_scale,
			fresnel_scale_sign,
			s_multiplier: 1.0,
		};

		let circular_arc_start = circular_arc_start(
			ingoing_clothoid_start_point,
			l_c_abs,
			ingoing_beta,
			fresnel_scale,
			fresnel_scale_sign,
		);
		let clothoid_angle_change =
			lambda_i as f32 * (difference_in_azimuth_i - circular_arc_angle_i) / 2.0;
		let clothoid_end_tangent_angle = unit_vector_i.z.atan2(unit_vector_i.x) + clothoid_angle_change;

		let w_i_vector = w_i_vector(lambda_i as f32, clothoid_end_tangent_angle);
		let circular_arc_center =
			circular_arc_center(circular_arc_radius_i, circular_arc_start, w_i_vector);
		let start_vector = circular_arc_start - circular_arc_center;

		let arc_sweep = -lambda_i.signum() as f32 * circular_arc_angle_i;
		let arc_end_point = {
			let start_vector_from_center = start_vector;
			let rotation = Quat::from_axis_angle(Vec3::Y, arc_sweep);
			let xz_pos = circular_arc_center + rotation * start_vector_from_center;
			let y_pos = height_sampler.height_at(xz_pos);
			Vec3::new(xz_pos.x, y_pos, xz_pos.z)
		};

		let circular_arc = CircularArcGeometry {
			start_point: circular_arc_start,
			center: circular_arc_center,
			start_vector,
			arc_sweep,
			start_elevation: circular_arc_start.y,
			end_point: arc_end_point,
			end_elevation: arc_end_point.y,
		};

		let clothoid_transition_end = tangent_vertex_i + total_tangent_length_i * unit_vector_i_plus_1;

		let outgoing_beta = f64::from(unit_vector_i_plus_1.z.atan2(unit_vector_i_plus_1.x));
		let outgoing_clothoid = ClothoidParameters {
			endpoint: clothoid_transition_end,
			length: l_c_abs,
			beta: outgoing_beta,
			fresnel_scale,
			fresnel_scale_sign: -fresnel_scale_sign,
			s_multiplier: -1.0,
		};

		segments.push(CurveSegment {
			tangent_vertex_prev: tangent_vertex_i_minus_1,
			tangent_vertex: tangent_vertex_i,
			tangent_vertex_next: tangent_vertex_i_plus_1,
			ingoing_clothoid_start: ingoing_clothoid_start_point,
			ingoing_clothoid,
			circular_arc,
			outgoing_clothoid_end: clothoid_transition_end,
			outgoing_clothoid,
			azimuth_of_tangent: azimuth_of_tangent_i,
			difference_in_azimuth: difference_in_azimuth_i,
		});
	}

	AlignmentGeometry { segments }
}
