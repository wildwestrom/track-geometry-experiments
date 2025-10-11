use bevy::prelude::*;

use crate::alignment::geometry::total_tangent_length;

use super::geometry::{azimuth_of_tangent, circular_section_length, difference_in_azimuth};
use super::state::{AlignmentState, PathSegment};
use super::{MAX_ARC_RADIUS, MIN_ARC_RADIUS};

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
	let length_of_circular = circular_section_length(
		segment.circular_section_radius,
		segment.circular_section_angle,
		diff_az,
	);
	let mut total_tangent = total_tangent_length(
		segment.circular_section_radius,
		segment.circular_section_angle,
		diff_az,
		length_of_circular,
	);

	let available_prev = previous.distance(segment.tangent_vertex);
	let available_next = segment.tangent_vertex.distance(next);
	let allowed = available_prev.min(available_next) - 1.0e-3;

	if total_tangent > allowed {
		// Reduce radius using binary search until the tangent fits.
		let mut lo = MIN_ARC_RADIUS;
		let mut hi = segment.circular_section_radius.min(MAX_ARC_RADIUS);
		for _ in 0..32 {
			let mid = 0.5 * (lo + hi);
			let l_c = circular_section_length(mid, segment.circular_section_angle, diff_az);
			let tlen = total_tangent_length(mid, segment.circular_section_angle, diff_az, l_c);
			if tlen > allowed {
				hi = mid;
			} else {
				lo = mid;
			}
		}
		segment.circular_section_radius = lo.clamp(MIN_ARC_RADIUS, MAX_ARC_RADIUS);

		// Recompute with the new radius
		let l_c = circular_section_length(
			segment.circular_section_radius,
			segment.circular_section_angle,
			diff_az,
		);
		total_tangent = total_tangent_length(
			segment.circular_section_radius,
			segment.circular_section_angle,
			diff_az,
			l_c,
		);
	}

	// If we still overshoot even at the shrunken radius, try increasing the
	// circular section angle up to the geometric max. Larger omega shortens
	// tangents.
	if total_tangent > allowed {
		let mut lo = segment.circular_section_angle;
		let mut hi = compute_max_angle(previous, segment.tangent_vertex, next);
		for _ in 0..32 {
			let mid = 0.5 * (lo + hi);
			let l_c = circular_section_length(segment.circular_section_radius, mid, diff_az);
			let tlen = total_tangent_length(segment.circular_section_radius, mid, diff_az, l_c);
			if tlen > allowed {
				// Need even larger omega to shorten tangents
				lo = mid;
			} else {
				hi = mid;
			}
		}
		segment.circular_section_angle = hi.min(max_angle);
	}
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
	}
}
