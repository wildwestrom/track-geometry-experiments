use crate::saveable::SaveableSettings;
use crate::ui_shell::{ActivePanel, UiShellState};
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
	ui_shell_state: Res<UiShellState>,
	alignment_pins: Query<(&Transform, &AlignmentPoint)>,
) {
	if ui_shell_state.active_panel != ActivePanel::AlignmentProperties {
		return;
	}

	let path_debug_level = &mut path_debug_level.0;
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Alignment Properties")
			.fixed_pos(egui::pos2(8.0, 8.0))
			.movable(false)
			.resizable(false)
			.show(ctx, |ui| {
				ui.label(format!("Path Debug Level: {path_debug_level}"));
				ui.horizontal(|ui| {
					for i in 0..=MAX_GEOMETRY_DEBUG_LEVEL {
						if ui.button(i.to_string()).clicked() {
							*path_debug_level = i;
						};
					}
				});

				ui.label(format!(
					"Current alignment: {}",
					alignment_state.current_alignment
				));
				ui.label(format!(
					"Total alignments: {}",
					alignment_state.alignments.len()
				));
				ui.label(format!("Total pins: {}", alignment_pins.iter().count()));

				let mut start_pos = Vec3::ZERO;
				let mut end_pos = Vec3::ZERO;

				for (transform, alignment_point) in alignment_pins.iter() {
					if alignment_point.alignment_id == alignment_state.current_alignment {
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
	let mut alignment_entries: Vec<_> = alignment_state.alignments.iter().collect();
	alignment_entries.sort_by_key(|(id, _)| *id);

	let mut id_to_delete: Option<usize> = None;

	for (&id, alignment) in alignment_entries {
		let n_turns = alignment.turn_count();
		let label = match n_turns {
			0 => format!("Alignment {} (Straight)", id),
			1 => format!("Alignment {} (1 Turn)", id),
			n => format!("Alignment {} ({} Turns)", id, n),
		};
		ui.horizontal(|ui| {
			ui.radio_value(&mut alignment_state.current_alignment, id, label);
			if ui.small_button("X").clicked() {
				id_to_delete = Some(id);
			}
		});
	}

	// Delete alignment if requested
	if let Some(id) = id_to_delete {
		alignment_state.alignments.remove(&id);
		// If we deleted the current alignment, switch to another one
		if alignment_state.current_alignment == id {
			alignment_state.current_alignment = alignment_state
				.alignments
				.keys()
				.next()
				.copied()
				.unwrap_or(0);
		}
	}
}

fn vertex_properties_ui(ui: &mut egui::Ui, alignment_state: &mut AlignmentState) {
	// Only show vertex properties if the alignment has intermediate tangent points
	if let Some(alignment) = &mut alignment_state
		.alignments
		.get_mut(&alignment_state.current_alignment)
		&& alignment.turn_count() > 0
	{
		let control_points = alignment.control_points();
		let segments: &mut [PathSegment] = &mut alignment.segments;

		let mut turn_index = 0;
		for (i, segment) in segments.iter_mut().enumerate() {
			let Some(turn) = segment.as_turn_mut() else {
				continue;
			};
			turn_index += 1;
			let vertex = turn.tangent_vertex;
			egui::Grid::new(format!("turn_{i}"))
				.num_columns(2)
				.spacing(egui::Vec2::splat(2.0))
				.show(ui, |ui| {
					ui.label(format!("Turn {}:", turn_index));
					ui.label(format!(
						"({:.2}, {:.2}, {:.2})",
						vertex.x, vertex.y, vertex.z,
					));
					ui.end_row();
					ui.label("Angle:");
					// Use shared constraints helper to determine slider max
					let prev = control_points[i];
					let next = control_points[i + 2];
					let max_angle = compute_max_angle(prev, vertex, next);
					if !turn.circular_section_angle.is_finite() || turn.circular_section_angle < 0.0 {
						turn.circular_section_angle = 0.0;
					}
					if turn.circular_section_angle > max_angle {
						turn.circular_section_angle = max_angle;
					}
					ui.add(
						egui::Slider::new(&mut turn.circular_section_angle, 0.0..=max_angle)
							.step_by(FRAC_PI_180)
							.custom_parser(|s| s.parse::<f64>().ok().map(|f| f.to_radians()))
							.custom_formatter(|val, _| format!("{:.0?}°", val.to_degrees())),
					);
					ui.end_row();
					ui.label("Radius:");
					// Enforce a minimum positive radius to avoid degenerate cases
					if !turn.circular_section_radius.is_finite() || turn.circular_section_radius <= 0.0 {
						turn.circular_section_radius = MIN_ARC_RADIUS;
					}
					ui.add(egui::Slider::new(
						&mut turn.circular_section_radius,
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

		let mut n_turns = alignment_state.ui_new_alignment_turns;

		if ui
			.add_enabled(n_turns > 1, egui::Button::new("-"))
			.clicked()
		{
			n_turns = (n_turns - 1).max(1);
		}

		ui.label(format!("{n_turns}"));

		if ui
			.add_enabled(n_turns < MAX_TURNS, egui::Button::new("+"))
			.clicked()
		{
			n_turns += 1;
		}

		alignment_state.ui_new_alignment_turns = n_turns;

		if ui.button("Add Alignment").clicked() {
			let new_id = alignment_state.next_alignment_id;
			alignment_state.add_alignment(new_id, start_pos, end_pos, n_turns);
			alignment_state.next_alignment_id += 1;
			alignment_state.current_alignment = new_id;
		}
	});
}
