use alignment_path::{Alignment, MAX_ARC_RADIUS, PathSegment};
use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::saveable::SaveableSettings;
use crate::terrain;
use terrain::spatial::world_size_for_height;

pub(crate) type AlignmentId = usize;

#[derive(Resource, Default)]
pub(crate) struct TrackBuildingMode {
	pub active: bool,
}

/// Tracks the state of a draft alignment being constructed.
/// When a user clicks to place points, this resource holds the in-progress data.
#[derive(Resource, Default)]
pub(crate) struct DraftAlignment {
	/// The starting point of the alignment being built
	pub start: Option<Vec3>,
	/// Tangent direction to preserve when chaining segments
	pub previous_tangent: Option<Vec3>,
	/// Alignment currently being built in chained mode
	pub active_alignment_id: Option<AlignmentId>,
}

#[derive(Resource, Serialize, Deserialize)]
pub(crate) struct AlignmentState {
	/// The currently selected/visible alignment
	pub current_alignment: AlignmentId,
	pub alignments: HashMap<AlignmentId, Alignment>,
	/// Counter for generating unique alignment IDs
	#[serde(skip)]
	pub next_alignment_id: AlignmentId,
	/// Number of turns for manually creating alignments via UI (separate from ID)
	#[serde(skip)]
	pub ui_new_alignment_turns: usize,
}

impl Default for AlignmentState {
	fn default() -> Self {
		Self {
			current_alignment: 0,
			alignments: HashMap::new(),
			next_alignment_id: 1,
			ui_new_alignment_turns: 1,
		}
	}
}

impl AlignmentState {
	/// Add a new alignment with the given ID, start/end points, and number of intermediate tangent
	/// points. For a straight segment with no curves, use n_tangents=0.
	pub(crate) fn add_alignment(
		&mut self,
		id: AlignmentId,
		start: Vec3,
		end: Vec3,
		n_tangents: usize,
	) {
		self
			.alignments
			.insert(id, Alignment::new(start, end, n_tangents));
	}
}

const SNAP_TO_TANGENT_DOT_THRESHOLD: f32 = 0.996_194_7; // cos(5deg)

pub(crate) fn snapped_tangent_direction(
	start: Vec3,
	end: Vec3,
	previous_tangent: Option<Vec3>,
) -> Option<Vec3> {
	let previous_tangent = normalize_xz(previous_tangent?)?;
	let cursor_direction = normalize_xz(end - start)?;
	if should_snap_to_previous_tangent(previous_tangent, cursor_direction) {
		Some(previous_tangent)
	} else {
		None
	}
}

pub(crate) fn snapped_segment_end(start: Vec3, end: Vec3, previous_tangent: Option<Vec3>) -> Vec3 {
	let Some(direction) = snapped_tangent_direction(start, end, previous_tangent) else {
		return end;
	};
	let to_end = Vec3::new(end.x - start.x, 0.0, end.z - start.z);
	let forward_distance = to_end.dot(direction).max(0.0);
	Vec3::new(
		start.x + direction.x * forward_distance,
		end.y,
		start.z + direction.z * forward_distance,
	)
}

pub(crate) fn build_preview_alignment(
	start: Vec3,
	end: Vec3,
	previous_tangent: Option<Vec3>,
) -> Alignment {
	let snapped_end = snapped_segment_end(start, end, previous_tangent);
	if snapped_end.x != end.x || snapped_end.z != end.z {
		return Alignment::new(start, snapped_end, 0);
	}

	let Some(previous_tangent) = normalize_xz(previous_tangent.unwrap_or(Vec3::ZERO)) else {
		return Alignment::new(start, end, 0);
	};
	let Some(cursor_direction) = normalize_xz(end - start) else {
		return Alignment::new(start, end, 0);
	};

	if should_snap_to_previous_tangent(previous_tangent, cursor_direction) {
		return Alignment::new(start, end, 0);
	}

	let mut alignment = Alignment::new(start, end, 0);
	let distance = start.distance(end);
	let max_forward = (distance * 0.8).max(1.0);
	let forward = (distance * 0.45).clamp(1.0, max_forward);
	let mut tangent_vertex = start + previous_tangent * forward;
	tangent_vertex.y = start.lerp(end, 0.45).y;
	alignment.append_turn(tangent_vertex);
	configure_preview_turn_tangent_consumption(
		alignment
			.segments
			.last_mut()
			.and_then(PathSegment::as_turn_mut),
	);
	alignment
}

pub(crate) fn alignment_end_tangent(start: Vec3, end: Vec3, alignment: &Alignment) -> Option<Vec3> {
	let tangent = alignment
		.segments
		.len()
		.checked_sub(1)
		.and_then(|segment_index| alignment.segment_control_point(segment_index))
		.map(|control_point| end - control_point)
		.unwrap_or(end - start);
	normalize_xz(tangent)
}

pub(crate) fn extend_alignment_with_preview(
	alignment: &mut Alignment,
	segment_start: Vec3,
	segment_end: Vec3,
	previous_tangent: Option<Vec3>,
) {
	let preview = build_preview_alignment(segment_start, segment_end, previous_tangent);
	let preview_turn = preview.segments.first().and_then(|preview_segment| {
		if let PathSegment::Turn(turn) = preview_segment {
			Some(*turn)
		} else {
			None
		}
	});
	let next_anchor = preview_turn
		.map(|turn| turn.tangent_vertex)
		.unwrap_or(segment_end);
	alignment.append_segment_boundary(segment_start, next_anchor);
	if let Some(turn) = preview_turn {
		alignment.segments.push(PathSegment::Turn(turn));
	}
	alignment.end = segment_end;
	alignment_path::constraints::enforce_alignment_constraints(alignment);
}

fn normalize_xz(vector: Vec3) -> Option<Vec3> {
	let xz = Vec3::new(vector.x, 0.0, vector.z);
	let length = xz.length();
	if !length.is_finite() || length <= f32::EPSILON {
		return None;
	}
	Some(xz / length)
}

fn should_snap_to_previous_tangent(previous_tangent: Vec3, cursor_direction: Vec3) -> bool {
	previous_tangent.dot(cursor_direction) >= SNAP_TO_TANGENT_DOT_THRESHOLD
}

fn configure_preview_turn_tangent_consumption(turn: Option<&mut alignment_path::TurnSegment>) {
	let Some(turn) = turn else {
		return;
	};

	// Bias preview turns toward consuming the full incoming tangent so the curve
	// starts right after the previous straight section.
	turn.circular_section_radius = MAX_ARC_RADIUS;
	turn.circular_section_angle = 0.0;
}

#[cfg(test)]
mod tests {
	use super::*;
	use alignment_path::{GeometrySegment, HeightSampler, calculate_alignment_geometry};

	struct FlatSampler;

	impl HeightSampler for FlatSampler {
		fn height_at(&self, position: Vec3) -> f32 {
			position.y
		}
	}

	fn assert_vec3_approx_eq(actual: Vec3, expected: Vec3) {
		let delta = actual.distance(expected);
		assert!(
			delta <= 1.0e-3,
			"expected {expected:?}, got {actual:?} (|delta|={delta})",
		);
	}

	#[test]
	fn extend_alignment_preserves_preview_tangent_vertex() {
		let segment_start = Vec3::new(0.0, 0.0, 0.0);
		let segment_end = Vec3::new(20.0, 0.0, 10.0);
		let previous_tangent = Some(Vec3::X);
		let preview = build_preview_alignment(segment_start, segment_end, previous_tangent);
		let preview_vertex = preview
			.segments
			.first()
			.and_then(PathSegment::as_turn)
			.map(|turn| turn.tangent_vertex)
			.expect("preview should contain one turn segment");

		let mut alignment = Alignment::new(Vec3::new(-15.0, 0.0, 0.0), segment_start, 0);
		extend_alignment_with_preview(&mut alignment, segment_start, segment_end, previous_tangent);

		assert_eq!(alignment.end, segment_end);
		assert_eq!(alignment.segments.len(), 2);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("first control point should exist"),
			segment_start,
		);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(1)
				.expect("second control point should exist"),
			preview_vertex,
		);
	}

	#[test]
	fn extend_alignment_keeps_straight_segment_without_new_vertex() {
		let segment_start = Vec3::new(0.0, 0.0, 0.0);
		let segment_end = Vec3::new(25.0, 0.0, 0.0);
		let previous_tangent = Some(Vec3::X);
		let mut alignment = Alignment::new(Vec3::new(-10.0, 0.0, 0.0), segment_start, 0);

		extend_alignment_with_preview(&mut alignment, segment_start, segment_end, previous_tangent);

		assert_eq!(alignment.end, segment_end);
		assert_eq!(alignment.segments.len(), 1);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("straight boundary should exist"),
			segment_start,
		);
	}

	#[test]
	fn extend_alignment_allows_internal_straight_then_turn() {
		let mut alignment = Alignment::new(Vec3::new(-10.0, 0.0, 0.0), Vec3::ZERO, 0);

		let first_end = Vec3::new(20.0, 0.0, 0.0);
		extend_alignment_with_preview(&mut alignment, Vec3::ZERO, first_end, Some(Vec3::X));
		assert_eq!(alignment.end, first_end);
		assert_eq!(alignment.segments.len(), 1);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("first control point should exist"),
			Vec3::ZERO,
		);

		let second_end = Vec3::new(30.0, 0.0, 15.0);
		extend_alignment_with_preview(&mut alignment, first_end, second_end, Some(Vec3::X));

		assert_eq!(alignment.end, second_end);
		assert_eq!(alignment.segments.len(), 3);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("first control point should exist"),
			Vec3::ZERO,
		);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(1)
				.expect("second control point should exist"),
			first_end,
		);
		assert!(matches!(alignment.segments[2], PathSegment::Turn(_)));
	}

	#[test]
	fn tangent_snap_extension_keeps_previous_end_as_fractional_straight_node() {
		let initial_start = Vec3::new(-10.0, 0.0, 0.0);
		let initial_end = Vec3::new(20.0, 0.0, 0.0);
		let new_end = Vec3::new(40.0, 0.0, 0.0);
		let mut alignment = Alignment::new(initial_start, initial_end, 0);

		extend_alignment_with_preview(&mut alignment, initial_end, new_end, Some(Vec3::X));

		assert_eq!(alignment.end, new_end);
		assert_eq!(alignment.segments.len(), 1);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("straight node should resolve"),
			initial_end,
		);
		let straight = alignment.segments[0]
			.as_straight()
			.expect("segment should be straight");
		assert!((straight.fraction() - 0.6).abs() < 1.0e-4);
	}

	#[test]
	fn snapped_segment_end_projects_endpoint_onto_previous_tangent() {
		let start = Vec3::new(10.0, 0.0, 5.0);
		let raw_end = Vec3::new(35.0, 0.0, 6.0);
		let snapped_end = snapped_segment_end(start, raw_end, Some(Vec3::X));

		assert!((snapped_end.z - start.z).abs() <= 1.0e-4);
		assert!((snapped_end.x - 35.0).abs() <= 1.0e-4);
	}

	#[test]
	fn curve_after_snapped_straights_preserves_straight_nodes() {
		let initial_start = Vec3::new(0.0, 0.0, 0.0);
		let first_end = Vec3::new(10.0, 0.0, 0.0);
		let second_end = Vec3::new(20.0, 0.0, 0.0);
		let curve_end = Vec3::new(28.0, 0.0, 8.0);
		let mut alignment = Alignment::new(initial_start, first_end, 0);

		extend_alignment_with_preview(&mut alignment, first_end, second_end, Some(Vec3::X));
		extend_alignment_with_preview(&mut alignment, second_end, curve_end, Some(Vec3::X));

		assert_eq!(alignment.end, curve_end);
		assert_eq!(alignment.segments.len(), 3);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(0)
				.expect("first straight node should exist"),
			first_end,
		);
		assert_vec3_approx_eq(
			alignment
				.segment_control_point(1)
				.expect("second straight node should exist"),
			second_end,
		);
		assert!(matches!(alignment.segments[2], PathSegment::Turn(_)));
	}

	#[test]
	fn curve_after_snapped_straight_begins_at_last_straight_endpoint() {
		let initial_start = Vec3::new(0.0, 0.0, 0.0);
		let first_end = Vec3::new(10.0, 0.0, 0.0);
		let second_end = Vec3::new(20.0, 0.0, 0.0);
		let curve_end = Vec3::new(28.0, 0.0, 8.0);
		let mut alignment = Alignment::new(initial_start, first_end, 0);

		extend_alignment_with_preview(&mut alignment, first_end, second_end, Some(Vec3::X));
		extend_alignment_with_preview(&mut alignment, second_end, curve_end, Some(Vec3::X));

		let geometry = calculate_alignment_geometry(initial_start, curve_end, &alignment, &FlatSampler);
		let first_curve = geometry
			.segments
			.iter()
			.find_map(|segment| match segment {
				GeometrySegment::Turn(turn) => Some(*turn),
				GeometrySegment::Straight(_) => None,
			})
			.expect("alignment should contain a turn geometry segment");
		let distance_to_last_straight_end = first_curve.ingoing_clothoid_start.distance(second_end);
		assert!(
			distance_to_last_straight_end <= 0.05,
			"curve should begin at the last straight endpoint, got delta={distance_to_last_straight_end}",
		);
	}
}

impl SaveableSettings for AlignmentState {
	fn filename() -> &'static str {
		"alignments.json"
	}
}

pub(crate) fn load_alignment() -> AlignmentState {
	let mut settings = AlignmentState::load_or_default();
	// Ensure next_alignment_id is at least 1 (0 is reserved for the default alignment)
	settings.next_alignment_id = settings.next_alignment_id.max(1);
	// Initialize skipped fields
	settings.ui_new_alignment_turns = 1;
	for alignment in settings.alignments.values_mut() {
		alignment_path::constraints::enforce_alignment_constraints(alignment);
	}
	settings
}

pub(crate) fn startup(
	mut alignment_state: ResMut<AlignmentState>,
	settings: Res<terrain::Settings>,
) {
	let world_size = world_size_for_height(&settings);
	let start_world_pos = Vec3::new(0.45, 0.0, 0.0) * world_size;
	let end_world_pos = Vec3::new(-0.45, 0.0, 0.0) * world_size;
	if !alignment_state.alignments.contains_key(&0) {
		alignment_state
			.alignments
			.insert(0, Alignment::new(start_world_pos, end_world_pos, 0));
	}
	if !alignment_state
		.alignments
		.contains_key(&alignment_state.current_alignment)
	{
		alignment_state.current_alignment = 0;
	}
}
