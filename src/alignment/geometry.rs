use std::f64::consts::PI;

use bevy::math::ops::atan2;
use bevy::prelude::*;
use spec_math::Fresnel;

// Compute azimuth of the tangent from previous point to current point
pub(crate) fn azimuth_of_tangent(current: Vec3, previous: Vec3) -> f32 {
	let delta_x = current.x - previous.x;
	let delta_z = current.z - previous.z;
	let angle = atan2(delta_z, delta_x);
	-angle
}

// Compute the minimal absolute difference between two azimuths in [0, PI]
pub(crate) fn difference_in_azimuth(azimuth_i: f32, azimuth_ip1: f32) -> f32 {
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

pub(crate) fn circular_section_length(
	circular_section_radius_i: f32,
	circular_section_angle_i: f32,
	difference_in_azimuth_i: f32,
) -> f32 {
	circular_section_radius_i * (difference_in_azimuth_i - circular_section_angle_i)
}

pub(crate) fn total_tangent_length(
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

pub(crate) fn alpha_i(start_vector: Vec3) -> f32 {
	start_vector.z.atan2(start_vector.x)
}

pub(crate) fn o_i(circular_section_radius_i: f32, f_i: Vec3, w_i: Vec3) -> Vec3 {
	f_i + circular_section_radius_i * w_i
}

pub(crate) fn w_i(lambda_i: f32, clothoid_end_tangent_angle: f32) -> Vec3 {
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

pub(crate) fn f_i(t_i: Vec3, l_c_abs: f64, beta_i: f64, fresnel_scale: f64, fresnel_scale_sign: f64) -> Vec3 {
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

pub(crate) fn unit_vector(tangent_vertex_i: Vec3, tangent_vertex_i_minus_1: Vec3) -> Vec3 {
	(tangent_vertex_i - tangent_vertex_i_minus_1).normalize()
}

pub(crate) fn clothoid_point(
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