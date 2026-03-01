use alignment_path::Alignment;
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
	/// Add a new alignment with the given ID, start/end points, and number of intermediate tangent points.
	/// For a straight segment with no curves, use n_tangents=0.
	pub(crate) fn add_alignment(&mut self, id: AlignmentId, start: Vec3, end: Vec3, n_tangents: usize) {
		self.alignments.insert(id, Alignment::new(start, end, n_tangents));
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
