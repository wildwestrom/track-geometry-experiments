use bevy::platform::collections::HashMap;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

use crate::saveable::SaveableSettings;
use bevy_procedural_terrain_gen as terrain;
use terrain::spatial::world_size_for_height;

pub(crate) type Turns = usize;

#[derive(Resource, Serialize, Deserialize)]
pub(crate) struct AlignmentState {
	pub turns: Turns,
	pub alignments: HashMap<Turns, Alignment>,
	#[serde(skip)]
	pub draft_turns: Turns,
}

impl Default for AlignmentState {
	fn default() -> Self {
		Self {
			turns: 0,
			alignments: HashMap::new(),
			draft_turns: 1,
		}
	}
}

impl AlignmentState {
	pub(crate) fn add_alignment(&mut self, turns: usize, start: Vec3, end: Vec3) {
		self
			.alignments
			.insert(turns, Alignment::new(start, end, turns));
	}
}

#[derive(Serialize, Deserialize, Default)]
pub(crate) struct Alignment {
	pub start: Vec3,
	pub end: Vec3,
	pub n_tangents: usize,
	pub segments: Vec<PathSegment>,
}

impl Alignment {
	pub fn new(start: Vec3, end: Vec3, n_tangents: usize) -> Self {
		let mut sections = Vec::with_capacity(n_tangents);
		if n_tangents > 0 {
			for i in 1..=n_tangents {
				let s = i as f32 / (n_tangents + 1) as f32;
				let vertex = start.lerp(end, s);
				sections.push(PathSegment::new(vertex));
			}
		}
		Self {
			start,
			end,
			n_tangents,
			segments: sections,
		}
	}
}

#[derive(Debug, Serialize, Deserialize, Default, Clone, Copy)]
pub(crate) struct PathSegment {
	pub tangent_vertex: Vec3,
	pub circular_section_radius: f32,
	pub circular_section_angle: f32,
}

impl PathSegment {
	pub const fn new(tangent_vertex: Vec3) -> Self {
		Self {
			tangent_vertex,
			circular_section_radius: 50.0,
			circular_section_angle: 0.5,
		}
	}
}

impl SaveableSettings for AlignmentState {
	fn filename() -> &'static str {
		"alignments.json"
	}
}

pub(crate) fn load_alignment() -> AlignmentState {
	let mut settings = AlignmentState::load_or_default();
	settings.draft_turns = settings.draft_turns.max(1);
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
		.contains_key(&alignment_state.turns)
	{
		alignment_state.turns = 0;
	}
}
