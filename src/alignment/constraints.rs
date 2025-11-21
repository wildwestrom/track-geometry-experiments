use alignment_path::constraints as path_constraints;
use bevy::prelude::*;

use super::state::AlignmentState;

pub(crate) use alignment_path::constraints::compute_max_angle;

pub(crate) fn enforce_alignment_constraints(mut alignment_state: ResMut<AlignmentState>) {
	for alignment in alignment_state.alignments.values_mut() {
		path_constraints::enforce_alignment_constraints(alignment);
	}
}
