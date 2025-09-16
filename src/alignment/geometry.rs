use bevy::math::ops::atan2;
use bevy::prelude::*;

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
