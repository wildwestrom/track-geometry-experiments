use crate::saveable::SaveableSettings;
use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use alignment_path::PathSegment;

use super::components::{AlignmentPoint, PointType};
use super::constraints::compute_max_angle;
use super::state::AlignmentState;
use super::{
	FRAC_PI_180, GeometryDebugLevel, MAX_ARC_RADIUS, MAX_GEOMETRY_DEBUG_LEVEL, MAX_TURNS,
	MIN_ARC_RADIUS,
};

pub(crate) fn ui(
	mut contexts: EguiContexts,
	mut alignment_state: ResMut<AlignmentState>,
	mut path_debug_level: ResMut<GeometryDebugLevel>,
	alignment_pins: Query<(&Transform, &AlignmentPoint)>,
) {
	let path_debug_level = &mut path_debug_level.0;
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Alignment Properties")
			.default_pos((00.0, 35.0))
			.default_open(false)
			.show(ctx, |ui| {
				ui.label(format!("Path Debug Level: {path_debug_level}"));
				ui.horizontal(|ui| {
					for i in 0..=MAX_GEOMETRY_DEBUG_LEVEL {
						if ui.button(i.to_string()).clicked() {
							*path_debug_level = i;
						};
					}
				});

				ui.label(format!("Total turns: {}", alignment_state.turns));
				ui.label(format!("Total pins: {}", alignment_pins.iter().count()));

				let mut start_pos = Vec3::ZERO;
				let mut end_pos = Vec3::ZERO;

				for (transform, alignment_point) in alignment_pins.iter() {
					if alignment_point.alignment_id == alignment_state.turns {
						match alignment_point.point_type {
							PointType::Start => {
								start_pos = transform.translation;
							}
							PointType::End => {
								end_pos = transform.translation;
							}
							PointType::Intermediate { .. } => {}
						}
					}
				}

				ui.separator();

				display_position(ui, "Start (Red)", start_pos);
				display_position(ui, "End (Blue)", end_pos);
				ui.separator();

				ui.label("Select Alignment:");
				alignment_selection_ui(ui, &mut alignment_state);
				ui.separator();

				ui.label("Vertices:");
				vertex_properties_ui(ui, &mut alignment_state);
				ui.separator();

				ui.label("Create New Alignment:");
				alignment_creation_ui(ui, &mut alignment_state, start_pos, end_pos);
				ui.separator();

				let alignment_state: &AlignmentState = &alignment_state;
				alignment_state.handle_save_operation_ui(ui, "Save Alignments");
			});
	}
}

fn display_position(ui: &mut egui::Ui, label: &str, position: Vec3) {
	ui.label(format!(
		"{}: ({:.2},{:.2},{:.2})",
		label, position.x, position.y, position.z
	));
}

fn alignment_selection_ui(ui: &mut egui::Ui, alignment_state: &mut AlignmentState) {
	let mut alignment_keys: Vec<&usize> = alignment_state.alignments.keys().collect();
	alignment_keys.sort();

	for &turns in alignment_keys {
		let mut turns_str = turns.to_string();
		ui.radio_value(
			&mut alignment_state.turns,
			turns,
			match turns {
				0 => "Linear Alignment",
				1 => {
					turns_str.push_str(" Turn");
					&turns_str
				}
				_ => {
					turns_str.push_str(" Turns");
					&turns_str
				}
			},
		);
	}
}

fn vertex_properties_ui(ui: &mut egui::Ui, alignment_state: &mut AlignmentState) {
	if alignment_state.turns > 0
		&& let Some(alignment) = &mut alignment_state.alignments.get_mut(&alignment_state.turns)
	{
		let segments: &mut [PathSegment] = &mut alignment.segments;
		// Precompute neighbor positions to avoid borrowing conflicts
		let mut neighbor_positions: Vec<Vec3> = Vec::with_capacity(segments.len() + 2);
		neighbor_positions.push(alignment.start);
		for s in segments.iter() {
			neighbor_positions.push(s.tangent_vertex);
		}
		neighbor_positions.push(alignment.end);

		for (i, segment) in segments.iter_mut().enumerate() {
			let vertex = segment.tangent_vertex;
			egui::Grid::new(format!("turn_{i}"))
				.num_columns(2)
				.spacing(egui::Vec2::splat(2.0))
				.show(ui, |ui| {
					ui.label(format!("Vertex {:.2}:", i + 1));
					ui.label(format!(
						"({:.2}, {:.2}, {:.2})",
						vertex.x, vertex.y, vertex.z,
					));
					ui.end_row();
					ui.label("Angle:");
					// Use shared constraints helper to determine slider max
					let prev = neighbor_positions[i];
					let next = neighbor_positions[i + 2];
					let max_angle = compute_max_angle(prev, vertex, next);
					if !segment.circular_section_angle.is_finite() || segment.circular_section_angle < 0.0 {
						segment.circular_section_angle = 0.0;
					}
					if segment.circular_section_angle > max_angle {
						segment.circular_section_angle = max_angle;
					}
					ui.add(
						egui::Slider::new(&mut segment.circular_section_angle, 0.0..=max_angle)
							.step_by(FRAC_PI_180)
							.custom_parser(|s| s.parse::<f64>().ok().map(|f| f.to_radians()))
							.custom_formatter(|val, _| format!("{:.0?}°", val.to_degrees())),
					);
					ui.end_row();
					ui.label("Radius:");
					// Enforce a minimum positive radius to avoid degenerate cases
					if !segment.circular_section_radius.is_finite() || segment.circular_section_radius <= 0.0
					{
						segment.circular_section_radius = MIN_ARC_RADIUS;
					}
					ui.add(egui::Slider::new(
						&mut segment.circular_section_radius,
						MIN_ARC_RADIUS..=MAX_ARC_RADIUS,
					));
				});
		}
	}
}

fn alignment_creation_ui(
	ui: &mut egui::Ui,
	alignment_state: &mut AlignmentState,
	start_pos: Vec3,
	end_pos: Vec3,
) {
	ui.horizontal(|ui| {
		ui.label("Turns:");

		let mut draft_turns = alignment_state.draft_turns;

		if ui
			.add_enabled(draft_turns > 1, egui::Button::new("-"))
			.clicked()
		{
			draft_turns = (draft_turns - 1).max(1);
		}

		ui.label(format!("{draft_turns}"));

		if ui
			.add_enabled(draft_turns < MAX_TURNS, egui::Button::new("+"))
			.clicked()
		{
			draft_turns += 1;
		}

		alignment_state.draft_turns = draft_turns;

		if !alignment_state.alignments.contains_key(&draft_turns)
			&& ui.button("Add Alignment").clicked()
		{
			alignment_state.add_alignment(draft_turns, start_pos, end_pos);
			alignment_state.turns = draft_turns;
		}
	});
}
