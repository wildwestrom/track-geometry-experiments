use bevy::prelude::*;

use crate::alignment::geometry::total_tangent_length;

use super::geometry::{azimuth_of_tangent, circular_section_length, difference_in_azimuth};
use super::state::{AlignmentState, PathSegment};
use super::{MAX_ARC_RADIUS, MIN_ARC_RADIUS};

const TANGENT_EPSILON: f32 = 1.0e-3;

// Convenience: compute the max allowable circular angle at a vertex, given its neighbors
pub(crate) fn compute_max_angle(previous: Vec3, vertex: Vec3, next: Vec3) -> f32 {
	let az_i = azimuth_of_tangent(vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, vertex);
	let max_angle = difference_in_azimuth(az_i, az_ip1);
	max_angle
}

// Clamp a segment's parameters to valid ranges based on geometry
pub(crate) fn clamp_segment_parameters(segment: &mut PathSegment, previous: Vec3, next: Vec3) {
	// Radius: keep finite and >= MIN_ARC_RADIUS
	if !segment.circular_section_radius.is_finite() || segment.circular_section_radius <= 0.0 {
		segment.circular_section_radius = MIN_ARC_RADIUS;
	} else if segment.circular_section_radius < MIN_ARC_RADIUS {
		segment.circular_section_radius = MIN_ARC_RADIUS;
	}

	// Also clamp to a global maximum to avoid absurd values
	if segment.circular_section_radius > MAX_ARC_RADIUS {
		segment.circular_section_radius = MAX_ARC_RADIUS;
	}

	// Angle: keep finite and within [0, max_angle]
	if !segment.circular_section_angle.is_finite() || segment.circular_section_angle < 0.0 {
		segment.circular_section_angle = 0.0;
	}
	let max_angle = compute_max_angle(previous, segment.tangent_vertex, next);
	if segment.circular_section_angle > max_angle {
		segment.circular_section_angle = max_angle;
	}

	// Constrain radius so that the transition tangents do not extend beyond
	// the available straight-line distance to the neighboring vertices.
	let az_i = azimuth_of_tangent(segment.tangent_vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, segment.tangent_vertex);
	let diff_az = difference_in_azimuth(az_i, az_ip1);
	let available_prev = previous.distance(segment.tangent_vertex);
	let available_next = segment.tangent_vertex.distance(next);
	let allowed = (available_prev.min(available_next) - TANGENT_EPSILON).max(0.0);

	ensure_tangent_within_limit(segment, max_angle, diff_az, allowed);
}

// System: enforce constraints across the entire AlignmentState every frame
pub(crate) fn enforce_alignment_constraints(mut alignment_state: ResMut<AlignmentState>) {
	for alignment in alignment_state.alignments.values_mut() {
		if alignment.segments.is_empty() {
			continue;
		}

		// Build neighbor list: [start] + segment vertices + [end]
		let mut neighbor_positions: Vec<Vec3> = Vec::with_capacity(alignment.segments.len() + 2);
		neighbor_positions.push(alignment.start);
		for s in alignment.segments.iter() {
			neighbor_positions.push(s.tangent_vertex);
		}
		neighbor_positions.push(alignment.end);

		for (i, segment) in alignment.segments.iter_mut().enumerate() {
			let previous = neighbor_positions[i];
			let next = neighbor_positions[i + 2];
			clamp_segment_parameters(segment, previous, next);
		}

		clamp_pairwise_tangent_gaps(&mut alignment.segments, &neighbor_positions);
	}
}

fn clamp_pairwise_tangent_gaps(segments: &mut [PathSegment], neighbors: &[Vec3]) {
	if segments.len() < 2 {
		return;
	}

	for i in 0..segments.len().saturating_sub(1) {
		let left_prev = neighbors[i];
		let left_vertex = neighbors[i + 1];
		let shared_vertex = neighbors[i + 2];
		let right_next = neighbors.get(i + 3).copied().unwrap_or(shared_vertex);

		let distance_between_vertices = left_vertex.distance(shared_vertex);
		let allowed_sum = (distance_between_vertices - TANGENT_EPSILON).max(0.0);

		let diff_left = segment_turn_delta(left_prev, left_vertex, shared_vertex);
		let diff_right = segment_turn_delta(left_vertex, shared_vertex, right_next);

		let left_total = tangent_length_for_segment(&segments[i], diff_left);
		let right_total = tangent_length_for_segment(&segments[i + 1], diff_right);

		let total_sum = left_total + right_total;
		if total_sum <= allowed_sum || allowed_sum <= 0.0 {
			continue;
		}

		let scale = allowed_sum / total_sum;
		let target_left = left_total * scale;
		let target_right = right_total * scale;

		let left_max_angle = compute_max_angle(left_prev, left_vertex, shared_vertex);
		let right_max_angle = compute_max_angle(left_vertex, shared_vertex, right_next);

		ensure_tangent_within_limit(&mut segments[i], left_max_angle, diff_left, target_left);
		ensure_tangent_within_limit(
			&mut segments[i + 1],
			right_max_angle,
			diff_right,
			target_right,
		);
	}
}

fn segment_turn_delta(previous: Vec3, vertex: Vec3, next: Vec3) -> f32 {
	let az_i = azimuth_of_tangent(vertex, previous);
	let az_ip1 = azimuth_of_tangent(next, vertex);
	difference_in_azimuth(az_i, az_ip1)
}

fn tangent_length_for_segment(segment: &PathSegment, diff_az: f32) -> f32 {
	let l_c = circular_section_length(
		segment.circular_section_radius,
		segment.circular_section_angle,
		diff_az,
	);
	total_tangent_length(
		segment.circular_section_radius,
		segment.circular_section_angle,
		diff_az,
		l_c,
	)
}

fn ensure_tangent_within_limit(
	segment: &mut PathSegment,
	max_angle: f32,
	diff_az: f32,
	limit: f32,
) -> f32 {
	if limit <= 0.0 {
		segment.circular_section_angle = max_angle;
		return 0.0;
	}

	let mut total_tangent = tangent_length_for_segment(segment, diff_az);
	if total_tangent <= limit {
		return total_tangent;
	}

	// Reduce radius using binary search until the tangent fits.
	let mut lo = MIN_ARC_RADIUS;
	let mut hi = segment.circular_section_radius.min(MAX_ARC_RADIUS);
	for _ in 0..32 {
		let mid = 0.5 * (lo + hi);
		let tlen = {
			let l_c = circular_section_length(mid, segment.circular_section_angle, diff_az);
			total_tangent_length(mid, segment.circular_section_angle, diff_az, l_c)
		};
		if tlen > limit {
			hi = mid;
		} else {
			lo = mid;
		}
	}
	segment.circular_section_radius = lo.clamp(MIN_ARC_RADIUS, MAX_ARC_RADIUS);
	total_tangent = tangent_length_for_segment(segment, diff_az);
	if total_tangent <= limit {
		return total_tangent;
	}

	// If we still overshoot even at the shrunken radius, try increasing the
	// circular section angle up to the geometric max. Larger omega shortens
	// tangents.
	let mut lo_angle = segment.circular_section_angle;
	let mut hi_angle = max_angle;
	for _ in 0..32 {
		let mid = 0.5 * (lo_angle + hi_angle);
		let l_c = circular_section_length(segment.circular_section_radius, mid, diff_az);
		let tlen = total_tangent_length(segment.circular_section_radius, mid, diff_az, l_c);
		if tlen > limit {
			lo_angle = mid;
		} else {
			hi_angle = mid;
		}
	}
	segment.circular_section_angle = hi_angle.min(max_angle);
	tangent_length_for_segment(segment, diff_az)
}
