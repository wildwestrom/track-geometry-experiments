use glam::Vec3;

use crate::geometry::{
	azimuth_of_tangent, circular_section_length, difference_in_azimuth, total_tangent_length,
};
use crate::path::{Alignment, PathSegment, TurnSegment, project_fraction_onto_span};

pub const MIN_ARC_RADIUS: f32 = 1.0;
pub const MAX_ARC_RADIUS: f32 = 2000.0;
const TANGENT_EPSILON: f32 = 1.0e-3;
const STRAIGHT_BOUNDARY_EPSILON: f32 = 1.0e-4;

// Convenience: compute the max allowable circular angle at a vertex, given its neighbors
pub fn compute_max_angle(previous: Vec3, vertex: Vec3, next: Vec3) -> f32 {
	let az_i = azimuth_of_tangent(vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, vertex);
	difference_in_azimuth(az_i, az_ip1)
}

// Clamp a turn's parameters to valid ranges based on geometry
pub fn clamp_turn_parameters(turn: &mut TurnSegment, previous: Vec3, next: Vec3) {
	if !turn.circular_section_radius.is_finite() || turn.circular_section_radius <= 0.0 {
		turn.circular_section_radius = MIN_ARC_RADIUS;
	} else if turn.circular_section_radius < MIN_ARC_RADIUS {
		turn.circular_section_radius = MIN_ARC_RADIUS;
	}

	if turn.circular_section_radius > MAX_ARC_RADIUS {
		turn.circular_section_radius = MAX_ARC_RADIUS;
	}

	if !turn.circular_section_angle.is_finite() || turn.circular_section_angle < 0.0 {
		turn.circular_section_angle = 0.0;
	}
	let max_angle = compute_max_angle(previous, turn.tangent_vertex, next);
	if turn.circular_section_angle > max_angle {
		turn.circular_section_angle = max_angle;
	}

	let az_i = azimuth_of_tangent(turn.tangent_vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, turn.tangent_vertex);
	let diff_az = difference_in_azimuth(az_i, az_ip1);
	let available_prev = previous.distance(turn.tangent_vertex);
	let available_next = turn.tangent_vertex.distance(next);
	let allowed = (available_prev.min(available_next) - TANGENT_EPSILON).max(0.0);

	ensure_tangent_within_limit(turn, max_angle, diff_az, allowed);
}

pub fn enforce_alignment_constraints(alignment: &mut Alignment) {
	if alignment.segments.is_empty() {
		return;
	}

	enforce_straight_boundary_fractions(alignment);
	let control_points = alignment.control_points();

	for (i, segment) in alignment.segments.iter_mut().enumerate() {
		let Some(turn) = segment.as_turn_mut() else {
			continue;
		};
		let previous = control_points[i];
		let next = control_points[i + 2];
		clamp_turn_parameters(turn, previous, next);
	}

	clamp_shared_edge_tangents(&mut alignment.segments, &control_points);
}

fn enforce_straight_boundary_fractions(alignment: &mut Alignment) {
	let control_points = alignment.control_points();

	let mut run_start = 0;
	while run_start < alignment.segments.len() {
		if !matches!(alignment.segments[run_start], PathSegment::Straight(_)) {
			run_start += 1;
			continue;
		}

		let mut run_end = run_start;
		while run_end + 1 < alignment.segments.len()
			&& matches!(alignment.segments[run_end + 1], PathSegment::Straight(_))
		{
			run_end += 1;
		}

		let section_start = control_points[run_start];
		let section_end = control_points[run_end + 2];
		let run_len = run_end - run_start + 1;
		let max_fraction = 1.0 - STRAIGHT_BOUNDARY_EPSILON;
		let mut min_fraction = STRAIGHT_BOUNDARY_EPSILON;

		for offset in 0..run_len {
			let segment_index = run_start + offset;
			let current = control_points[segment_index + 1];
			let projected_fraction = project_fraction_onto_span(current, section_start, section_end);
			let remaining = run_len - offset - 1;
			let max_for_current =
				(max_fraction - STRAIGHT_BOUNDARY_EPSILON * remaining as f32).max(min_fraction);
			let normalized_fraction = projected_fraction.clamp(min_fraction, max_for_current);
			if let Some(straight) = alignment.segments[segment_index].as_straight_mut() {
				straight.set_fraction(normalized_fraction);
			}
			min_fraction = (normalized_fraction + STRAIGHT_BOUNDARY_EPSILON).min(max_fraction);
		}

		run_start = run_end + 1;
	}
}

fn clamp_shared_edge_tangents(segments: &mut [PathSegment], control_points: &[Vec3]) {
	if segments.len() < 2 {
		return;
	}

	for edge_idx in 1..segments.len() {
		let left_idx = edge_idx - 1;
		let right_idx = edge_idx;

		let left_cp = control_points[edge_idx];
		let right_cp = control_points[edge_idx + 1];
		let distance_between_control_points = left_cp.distance(right_cp);
		let allowed_sum = (distance_between_control_points - TANGENT_EPSILON).max(0.0);
		if allowed_sum <= 0.0 {
			continue;
		}

		let left_prev = control_points[edge_idx - 1];
		let left_vertex = left_cp;
		let left_next = right_cp;

		let right_prev = left_cp;
		let right_vertex = right_cp;
		let right_next = control_points[edge_idx + 2];

		let diff_left = segment_turn_delta(left_prev, left_vertex, left_next);
		let diff_right = segment_turn_delta(right_prev, right_vertex, right_next);

		let (left_segments, right_segments) = segments.split_at_mut(right_idx);
		let Some(left_turn) = left_segments[left_idx].as_turn_mut() else {
			continue;
		};
		let Some(right_turn) = right_segments[0].as_turn_mut() else {
			continue;
		};

		let left_total = tangent_length_for_turn(left_turn, diff_left);
		let right_total = tangent_length_for_turn(right_turn, diff_right);
		let total_sum = left_total + right_total;
		if total_sum <= allowed_sum {
			continue;
		}

		let scale = allowed_sum / total_sum;
		let target_left = left_total * scale;
		let target_right = right_total * scale;

		let left_max_angle = compute_max_angle(left_prev, left_vertex, left_next);
		let right_max_angle = compute_max_angle(right_prev, right_vertex, right_next);

		ensure_tangent_within_limit(left_turn, left_max_angle, diff_left, target_left);
		ensure_tangent_within_limit(right_turn, right_max_angle, diff_right, target_right);
	}
}

fn segment_turn_delta(previous: Vec3, vertex: Vec3, next: Vec3) -> f32 {
	let az_i = azimuth_of_tangent(vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, vertex);
	difference_in_azimuth(az_i, az_ip1)
}

fn tangent_length_for_turn(turn: &TurnSegment, diff_az: f32) -> f32 {
	let l_c = circular_section_length(
		turn.circular_section_radius,
		turn.circular_section_angle,
		diff_az,
	);
	total_tangent_length(
		turn.circular_section_radius,
		turn.circular_section_angle,
		diff_az,
		l_c,
	)
}

fn ensure_tangent_within_limit(
	turn: &mut TurnSegment,
	max_angle: f32,
	diff_az: f32,
	limit: f32,
) -> f32 {
	if limit <= 0.0 {
		turn.circular_section_angle = max_angle;
		return 0.0;
	}

	let mut total_tangent = tangent_length_for_turn(turn, diff_az);
	if total_tangent <= limit {
		return total_tangent;
	}

	let mut lo = MIN_ARC_RADIUS;
	let mut hi = turn.circular_section_radius.min(MAX_ARC_RADIUS);
	for _ in 0..32 {
		let mid = 0.5 * (lo + hi);
		let tlen = {
			let l_c = circular_section_length(mid, turn.circular_section_angle, diff_az);
			total_tangent_length(mid, turn.circular_section_angle, diff_az, l_c)
		};
		if tlen > limit {
			hi = mid;
		} else {
			lo = mid;
		}
	}
	turn.circular_section_radius = lo.clamp(MIN_ARC_RADIUS, MAX_ARC_RADIUS);
	total_tangent = tangent_length_for_turn(turn, diff_az);
	if total_tangent <= limit {
		return total_tangent;
	}

	let mut lo_angle = turn.circular_section_angle;
	let mut hi_angle = max_angle;
	for _ in 0..32 {
		let mid = 0.5 * (lo_angle + hi_angle);
		let l_c = circular_section_length(turn.circular_section_radius, mid, diff_az);
		let tlen = total_tangent_length(turn.circular_section_radius, mid, diff_az, l_c);
		if tlen > limit {
			lo_angle = mid;
		} else {
			hi_angle = mid;
		}
	}
	turn.circular_section_angle = hi_angle.min(max_angle);
	tangent_length_for_turn(turn, diff_az)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::path::StraightSegment;

	#[test]
	fn straight_boundary_fraction_is_clamped_to_tangent_span() {
		let mut alignment = Alignment {
			start: Vec3::new(0.0, 0.0, 0.0),
			end: Vec3::new(10.0, 0.0, 0.0),
			segments: vec![PathSegment::Straight(StraightSegment::from_fraction(0.5))],
		};
		if let Some(straight) = alignment.segments[0].as_straight_mut() {
			straight.fraction = 1.25;
		}

		enforce_alignment_constraints(&mut alignment);

		let boundary = alignment
			.segment_control_point(0)
			.expect("straight segment must resolve to a control point");
		assert!(boundary.z.abs() < 1.0e-4);
		assert!(boundary.x > 0.0);
		assert!(boundary.x < 10.0);
	}

	#[test]
	fn consecutive_straight_boundaries_remain_ordered() {
		let mut alignment = Alignment {
			start: Vec3::ZERO,
			end: Vec3::new(10.0, 0.0, 0.0),
			segments: vec![
				PathSegment::Straight(StraightSegment::from_fraction(0.8)),
				PathSegment::Straight(StraightSegment::from_fraction(0.2)),
			],
		};

		enforce_alignment_constraints(&mut alignment);

		let first = alignment
			.segment_control_point(0)
			.expect("first straight control point should resolve");
		let second = alignment
			.segment_control_point(1)
			.expect("second straight control point should resolve");
		assert!(first.x < second.x);
	}
}
