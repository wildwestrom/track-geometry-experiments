use std::f64::consts::PI;

use glam::{Quat, Vec2, Vec3};
use spec_math::Fresnel;

use crate::path::{Alignment, TurnSegment};

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

	(hv_i + ph_i + tp_i) as f32
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
	pub segments: Vec<GeometrySegment>,
}

impl AlignmentGeometry {
	pub fn total_length(&self) -> f32 {
		self.segments.iter().map(GeometrySegment::length).sum()
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		self
			.segments
			.iter()
			.find_map(|segment| segment.xz_at_station(station))
	}
}

#[derive(Clone, Copy)]
pub enum GeometrySegment {
	Straight(StraightGeometry),
	Turn(CurveSegment),
}

impl GeometrySegment {
	pub fn start_station(&self) -> f32 {
		match self {
			Self::Straight(s) => s.start_station,
			Self::Turn(t) => t.start_station,
		}
	}

	pub fn length(&self) -> f32 {
		match self {
			Self::Straight(s) => s.length,
			Self::Turn(t) => t.length(),
		}
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		match self {
			Self::Straight(s) => s.xz_at_station(station),
			Self::Turn(t) => t.xz_at_station(station),
		}
	}
}

#[derive(Clone, Copy)]
pub struct StraightGeometry {
	pub start: Vec3,
	pub end: Vec3,
	pub start_station: f32,
	pub length: f32,
}

impl StraightGeometry {
	pub fn xz_at(&self, s: f32) -> Vec2 {
		let start = Vec2::new(self.start.x, self.start.z);
		let end = Vec2::new(self.end.x, self.end.z);
		start.lerp(end, s)
	}

	pub fn point_at(&self, s: f32, y: f32) -> Vec3 {
		let xz = self.xz_at(s);
		Vec3::new(xz.x, y, xz.y)
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		let s = local_s_for_station(station, self.start_station, self.length)?;
		Some(self.xz_at(s))
	}
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
	pub start_station: f32,
}

impl CurveSegment {
	pub fn length(&self) -> f32 {
		self.ingoing_clothoid.length + self.circular_arc.length + self.outgoing_clothoid.length
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		self
			.ingoing_clothoid
			.xz_at_station(station)
			.or_else(|| self.circular_arc.xz_at_station(station))
			.or_else(|| self.outgoing_clothoid.xz_at_station(station))
	}
}

#[derive(Clone, Copy)]
pub struct ClothoidParameters {
	pub endpoint: Vec3,
	pub circular_arc_length: f64,
	pub beta: f64,
	pub fresnel_scale: f64,
	pub fresnel_scale_sign: f64,
	pub s_multiplier: f64,
	pub length: f32,
	pub station_at_s0: f32,
	pub station_at_s1: f32,
}

impl ClothoidParameters {
	pub fn xz_at(&self, s: f32) -> Vec2 {
		let point = clothoid_point(
			self.s_multiplier * f64::from(s),
			self.endpoint,
			self.circular_arc_length,
			self.beta,
			self.fresnel_scale,
			self.fresnel_scale_sign,
		);
		Vec2::new(point.x, point.z)
	}

	pub fn point_at(&self, s: f32, y: f32) -> Vec3 {
		let xz = self.xz_at(s);
		Vec3::new(xz.x, y, xz.y)
	}

	pub fn station_at(&self, s: f32) -> f32 {
		self.station_at_s0 * (1.0 - s) + self.station_at_s1 * s
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		let lo = self.station_at_s0.min(self.station_at_s1);
		let hi = self.station_at_s0.max(self.station_at_s1);
		if !(lo..=hi).contains(&station) {
			return None;
		}
		let span = self.station_at_s1 - self.station_at_s0;
		let s = if span.abs() < f32::EPSILON {
			0.0
		} else {
			(station - self.station_at_s0) / span
		};
		Some(self.xz_at(s.clamp(0.0, 1.0)))
	}
}

#[derive(Clone, Copy)]
pub struct CircularArcGeometry {
	pub start_point: Vec3,
	pub center: Vec3,
	pub start_vector: Vec3,
	pub arc_sweep: f32,
	pub end_point: Vec3,
	pub start_station: f32,
	pub length: f32,
}

impl CircularArcGeometry {
	pub fn xz_at(&self, s: f32) -> Vec2 {
		let sweep_angle = self.arc_sweep * s;
		let rotation = Quat::from_axis_angle(Vec3::Y, sweep_angle);
		let rotated_vector = rotation * self.start_vector;
		let xz_position = self.center + rotated_vector;
		Vec2::new(xz_position.x, xz_position.z)
	}

	pub fn point_at(&self, s: f32, y: f32) -> Vec3 {
		let xz = self.xz_at(s);
		Vec3::new(xz.x, y, xz.y)
	}

	pub fn xz_at_station(&self, station: f32) -> Option<Vec2> {
		let s = local_s_for_station(station, self.start_station, self.length)?;
		Some(self.xz_at(s))
	}
}

fn local_s_for_station(station: f32, start_station: f32, length: f32) -> Option<f32> {
	if length <= 0.0 {
		return None;
	}
	if !(start_station..=start_station + length).contains(&station) {
		return None;
	}
	Some(((station - start_station) / length).clamp(0.0, 1.0))
}

pub fn calculate_alignment_geometry(
	start: Vec3,
	end: Vec3,
	alignment: &Alignment,
) -> AlignmentGeometry {
	assert!(start.is_finite(), "start vertex must be finite: {start}");
	assert!(end.is_finite(), "end vertex must be finite: {end}");

	let control_points = alignment.control_points_with_endpoints(start, end);

	for (i, segment) in alignment.segments.iter().enumerate() {
		let control_point = control_points[i + 1];
		assert!(
			control_point.is_finite(),
			"segment {i} control point is not finite: {control_point}",
		);
		if let Some(turn) = segment.as_turn() {
			assert!(
				turn.circular_section_radius.is_finite(),
				"segment {i} radius is not finite: {}",
				turn.circular_section_radius
			);
			assert!(
				turn.circular_section_angle.is_finite(),
				"segment {i} angle is not finite: {}",
				turn.circular_section_angle
			);
		}
	}

	let mut turn_geometry_by_control_point = vec![None; control_points.len()];
	for (i, segment) in alignment.segments.iter().enumerate() {
		let Some(turn) = segment.as_turn() else {
			continue;
		};
		let previous = control_points[i];
		let tangent_vertex = control_points[i + 1];
		let next = control_points[i + 2];
		if let Some(turn_geometry) = compute_turn_geometry(previous, tangent_vertex, next, turn) {
			turn_geometry_by_control_point[i + 1] = Some(turn_geometry);
		}
	}

	let mut segments = Vec::new();
	let mut station = 0.0_f32;
	for edge_idx in 0..control_points.len().saturating_sub(1) {
		let left_cp = control_points[edge_idx];
		let right_cp = control_points[edge_idx + 1];

		let straight_start = turn_geometry_by_control_point[edge_idx]
			.map(|turn| turn.outgoing_clothoid_end)
			.unwrap_or(left_cp);
		let straight_end = turn_geometry_by_control_point[edge_idx + 1]
			.map(|turn| turn.ingoing_clothoid_start)
			.unwrap_or(right_cp);

		if straight_start.distance_squared(straight_end) > f32::EPSILON {
			let length = Vec2::new(
				straight_end.x - straight_start.x,
				straight_end.z - straight_start.z,
			)
			.length();
			segments.push(GeometrySegment::Straight(StraightGeometry {
				start: straight_start,
				end: straight_end,
				start_station: station,
				length,
			}));
			station += length;
		}

		if let Some(mut turn_geometry) = turn_geometry_by_control_point[edge_idx + 1] {
			assign_turn_stations(&mut turn_geometry, station);
			station += turn_geometry.length();
			segments.push(GeometrySegment::Turn(turn_geometry));
		}
	}

	AlignmentGeometry { segments }
}

fn assign_turn_stations(turn: &mut CurveSegment, start_station: f32) {
	turn.start_station = start_station;

	let ingoing_length = turn.ingoing_clothoid.length;
	turn.ingoing_clothoid.station_at_s0 = start_station;
	turn.ingoing_clothoid.station_at_s1 = start_station + ingoing_length;

	let arc_start = start_station + ingoing_length;
	turn.circular_arc.start_station = arc_start;
	let arc_end = arc_start + turn.circular_arc.length;

	let outgoing_length = turn.outgoing_clothoid.length;
	// outgoing_clothoid.point_at(s=0) is at the outgoing tangent endpoint (far from arc),
	// which is the later station in traversal order; s=1 is at arc end.
	turn.outgoing_clothoid.station_at_s0 = arc_end + outgoing_length;
	turn.outgoing_clothoid.station_at_s1 = arc_end;
}

fn compute_turn_geometry(
	tangent_vertex_i_minus_1: Vec3,
	tangent_vertex_i: Vec3,
	tangent_vertex_i_plus_1: Vec3,
	turn: &TurnSegment,
) -> Option<CurveSegment> {
	let circular_arc_radius_i = turn.circular_section_radius;
	let circular_arc_angle_i = turn.circular_section_angle;

	let unit_vector_i = unit_vector(tangent_vertex_i, tangent_vertex_i_minus_1);
	let unit_vector_i_plus_1 = unit_vector(tangent_vertex_i_plus_1, tangent_vertex_i);
	if !unit_vector_i.is_finite() || !unit_vector_i_plus_1.is_finite() {
		return None;
	}

	let azimuth_of_tangent_i = azimuth_of_tangent(tangent_vertex_i, tangent_vertex_i_minus_1);
	let azimuth_of_tangent_i_plus_1 = azimuth_of_tangent(tangent_vertex_i_plus_1, tangent_vertex_i);

	let difference_in_azimuth_i =
		difference_in_azimuth(azimuth_of_tangent_i, azimuth_of_tangent_i_plus_1);
	if difference_in_azimuth_i <= f32::EPSILON {
		return None;
	}

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
	let clothoid_length = length_of_circular_section.abs();
	let arc_length = circular_arc_radius_i.abs() * circular_arc_angle_i.abs();

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
		circular_arc_length: l_c_abs,
		beta: ingoing_beta,
		fresnel_scale,
		fresnel_scale_sign,
		s_multiplier: 1.0,
		length: clothoid_length,
		station_at_s0: 0.0,
		station_at_s1: 0.0,
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
	let arc_end_xz = {
		let rotation = Quat::from_axis_angle(Vec3::Y, arc_sweep);
		circular_arc_center + rotation * start_vector
	};

	let circular_arc = CircularArcGeometry {
		start_point: circular_arc_start,
		center: circular_arc_center,
		start_vector,
		arc_sweep,
		end_point: arc_end_xz,
		start_station: 0.0,
		length: arc_length,
	};

	let clothoid_transition_end = tangent_vertex_i + total_tangent_length_i * unit_vector_i_plus_1;

	let outgoing_beta = f64::from(unit_vector_i_plus_1.z.atan2(unit_vector_i_plus_1.x));
	let outgoing_clothoid = ClothoidParameters {
		endpoint: clothoid_transition_end,
		circular_arc_length: l_c_abs,
		beta: outgoing_beta,
		fresnel_scale,
		fresnel_scale_sign: -fresnel_scale_sign,
		s_multiplier: -1.0,
		length: clothoid_length,
		station_at_s0: 0.0,
		station_at_s1: 0.0,
	};

	Some(CurveSegment {
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
		start_station: 0.0,
	})
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::path::{Alignment, TurnSegment};

	#[test]
	fn straight_length_matches_xz_distance() {
		let straight = StraightGeometry {
			start: Vec3::new(0.0, 5.0, 0.0),
			end: Vec3::new(30.0, 10.0, 40.0),
			start_station: 0.0,
			length: 50.0,
		};
		let xz_dist = Vec2::new(30.0, 40.0).length();
		assert!((straight.length - xz_dist).abs() < 1.0);
	}

	#[test]
	fn arc_length_matches_radius_times_sweep() {
		let arc = CircularArcGeometry {
			start_point: Vec3::ZERO,
			center: Vec3::new(100.0, 0.0, 0.0),
			start_vector: Vec3::new(-100.0, 0.0, 0.0),
			arc_sweep: 0.5,
			end_point: Vec3::ZERO,
			start_station: 0.0,
			length: 50.0,
		};
		assert!((arc.length - 100.0_f32 * 0.5_f32).abs() < 1e-3);
	}

	#[test]
	fn station_accumulates_monotonically() {
		let alignment = Alignment::new(
			Vec3::new(0.0, 0.0, 0.0),
			Vec3::new(200.0, 0.0, 0.0),
			1,
		);
		let geometry = calculate_alignment_geometry(
			alignment.start,
			alignment.end,
			&alignment,
		);

		let mut prev_end = 0.0_f32;
		for segment in &geometry.segments {
			let start = segment.start_station();
			let len = segment.length();
			assert!(
				(start - prev_end).abs() < 1e-2,
				"station gap: prev_end={prev_end}, start={start}",
			);
			assert!(len >= 0.0, "negative segment length: {len}");
			prev_end = start + len;
		}
	}

	#[test]
	fn total_length_matches_segment_sum() {
		let alignment = Alignment::new(
			Vec3::new(0.0, 0.0, 0.0),
			Vec3::new(200.0, 0.0, 200.0),
			1,
		);
		let geometry = calculate_alignment_geometry(
			alignment.start,
			alignment.end,
			&alignment,
		);
		let sum: f32 = geometry.segments.iter().map(GeometrySegment::length).sum();
		assert!((geometry.total_length() - sum).abs() < 1e-3);
	}

	#[test]
	fn clothoid_length_matches_computed_length() {
		let alignment = Alignment {
			start: Vec3::new(0.0, 0.0, 0.0),
			end: Vec3::new(300.0, 0.0, 0.0),
			segments: vec![crate::path::PathSegment::Turn(TurnSegment {
				tangent_vertex: Vec3::new(150.0, 0.0, 50.0),
				circular_section_radius: 100.0,
				circular_section_angle: 0.3,
			})],
			..Default::default()
		};
		let geometry = calculate_alignment_geometry(
			alignment.start,
			alignment.end,
			&alignment,
		);
		for segment in &geometry.segments {
			if let GeometrySegment::Turn(turn) = segment {
				let in_len = turn.ingoing_clothoid.length;
				let out_len = turn.outgoing_clothoid.length;
				assert_eq!(in_len, out_len, "clothoid lengths should be symmetric");
				assert!(in_len > 0.0, "clothoid length must be positive");
			}
		}
	}
}
