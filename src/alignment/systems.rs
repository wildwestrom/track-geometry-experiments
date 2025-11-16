use bevy::prelude::*;

use crate::pin::create_pin;
use crate::terrain;
use terrain::spatial::world_size_for_height;

use super::components::{AlignmentPoint, PointType};
use super::state::AlignmentState;

pub(crate) fn update_alignment_from_pins(
	alignment_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	let mut start_pos = None;
	let mut end_pos = None;

	for (transform, alignment_point) in alignment_pins.iter() {
		if alignment_point.alignment_id == alignment_state.turns {
			match alignment_point.point_type {
				PointType::Start => start_pos = Some(transform.translation),
				PointType::End => end_pos = Some(transform.translation),
				PointType::Intermediate { .. } => {}
			}
		}
	}

	let (Some(new_start), Some(new_end)) = (start_pos, end_pos) else {
		return;
	};

	for alignment in alignment_state.alignments.values_mut() {
		if alignment.start != new_start || alignment.end != new_end {
			alignment.start = new_start;
			alignment.end = new_end;
		}
	}
}

pub(crate) fn update_alignment_pins(
	mut commands: Commands,
	alignment_state: Res<AlignmentState>,
	existing_pins: Query<Entity, With<AlignmentPoint>>,
	settings: Res<terrain::Settings>,
	mut last_current_alignment: Local<Option<usize>>,
) {
	let current_alignment = alignment_state.turns;
	if *last_current_alignment == Some(current_alignment) {
		return;
	}
	*last_current_alignment = Some(current_alignment);

	for entity in existing_pins.iter() {
		commands.entity(entity).despawn();
	}

	if let Some(alignment) = alignment_state.alignments.get(&current_alignment) {
		let world_size = world_size_for_height(&settings);

		let start_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::Start,
		};
		let start_color = start_point.get_color();
		commands.queue(create_pin(
			alignment.start / world_size,
			world_size,
			start_point,
			start_color,
		));

		let end_point = AlignmentPoint {
			alignment_id: current_alignment,
			point_type: PointType::End,
		};
		let end_color = end_point.get_color();
		commands.queue(create_pin(
			alignment.end / world_size,
			world_size,
			end_point,
			end_color,
		));

		for (i, segment) in alignment.segments.iter().enumerate() {
			let normalized_pos = segment.tangent_vertex / world_size;
			let alignment_point = AlignmentPoint {
				alignment_id: current_alignment,
				point_type: PointType::Intermediate { segment_index: i },
			};
			let point_color = alignment_point.get_color();
			commands.queue(create_pin(
				normalized_pos,
				world_size,
				alignment_point,
				point_color,
			));
		}
	}
}

pub(crate) fn update_alignment_from_intermediate_pins(
	intermediate_pins: Query<(&Transform, &AlignmentPoint), Changed<Transform>>,
	mut alignment_state: ResMut<AlignmentState>,
) {
	for (transform, intermediate_point) in intermediate_pins.iter() {
		if let PointType::Intermediate { segment_index } = intermediate_point.point_type {
			if let Some(alignment) = alignment_state
				.alignments
				.get_mut(&intermediate_point.alignment_id)
			{
				if let Some(segment) = alignment.segments.get_mut(segment_index) {
					segment.tangent_vertex = transform.translation;
				}
			}
		}
	}
}

pub(crate) fn update_pins_from_alignment_state(
	alignment_state: Res<AlignmentState>,
	mut alignment_pins: Query<(&mut Transform, &AlignmentPoint)>,
) {
	if let Some(alignment) = alignment_state.alignments.values().next() {
		if alignment.start != Vec3::ZERO || alignment.end != Vec3::ZERO {
			for (mut transform, alignment_point) in &mut alignment_pins {
				if alignment_point.alignment_id == alignment_state.turns {
					match alignment_point.point_type {
						PointType::Start => {
							transform.translation = alignment.start;
						}
						PointType::End => {
							transform.translation = alignment.end;
						}
						PointType::Intermediate { .. } => {}
					}
				}
			}
		}
	}
}
