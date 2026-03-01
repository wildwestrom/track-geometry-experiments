use alignment_path::{Alignment, PathSegment};
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

pub(crate) fn build_preview_alignment(
	start: Vec3,
	end: Vec3,
	previous_tangent: Option<Vec3>,
) -> Alignment {
	let Some(previous_tangent) = normalize_xz(previous_tangent.unwrap_or(Vec3::ZERO)) else {
		return Alignment::new(start, end, 0);
	};
	let Some(cursor_direction) = normalize_xz(end - start) else {
		return Alignment::new(start, end, 0);
	};

	if previous_tangent.dot(cursor_direction) >= SNAP_TO_TANGENT_DOT_THRESHOLD {
		return Alignment::new(start, end, 0);
	}

	let mut alignment = Alignment::new(start, end, 1);
	let distance = start.distance(end);
	let max_forward = (distance * 0.8).max(1.0);
	let forward = (distance * 0.45).clamp(1.0, max_forward);
	let mut tangent_vertex = start + previous_tangent * forward;
	tangent_vertex.y = start.lerp(end, 0.45).y;
	if let Some(segment) = alignment.segments.first_mut() {
		segment.tangent_vertex = tangent_vertex;
	}
	alignment
}

pub(crate) fn alignment_end_tangent(start: Vec3, end: Vec3, alignment: &Alignment) -> Option<Vec3> {
	let tangent = alignment
		.segments
		.last()
		.map(|segment| end - segment.tangent_vertex)
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
	alignment.end = segment_end;

	if let Some(preview_segment) = preview.segments.first() {
		let mut junction_segment = PathSegment::new(segment_start);
		junction_segment.circular_section_radius = preview_segment.circular_section_radius;
		junction_segment.circular_section_angle = preview_segment.circular_section_angle;
		alignment.segments.push(junction_segment);
	}

	alignment.n_tangents = alignment.segments.len();
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
