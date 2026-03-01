use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::alignment::TrackBuildingMode;

pub struct UiShellPlugin;

impl Plugin for UiShellPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<UiShellState>()
			.add_systems(bevy_egui::EguiPrimaryContextPass, bottom_bar_ui);
	}
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiShellState {
	pub active_panel: ActivePanel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
	#[default]
	None,
	TerrainControls,
	ContourLines,
	AlignmentProperties,
	Visualizations,
}

fn bottom_bar_ui(
	mut contexts: EguiContexts,
	mut shell_state: ResMut<UiShellState>,
	mut track_building_mode: ResMut<TrackBuildingMode>,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	egui::TopBottomPanel::bottom("main_menu_bar").show(ctx, |ui| {
		ui.horizontal_centered(|ui| {
			ui.spacing_mut().item_spacing.x = 8.0;
			panel_button(ui, &mut shell_state, "Terrain", ActivePanel::TerrainControls);
			panel_button(ui, &mut shell_state, "Contour", ActivePanel::ContourLines);
			panel_button(
				ui,
				&mut shell_state,
				"Alignment",
				ActivePanel::AlignmentProperties,
			);
			panel_button(ui, &mut shell_state, "Visualizations", ActivePanel::Visualizations);

			let build_label = if track_building_mode.active {
				"Exit Build (Esc/F)"
			} else {
				"Build Track (F)"
			};
			let build_button = egui::Button::new(build_label).selected(track_building_mode.active);
			if ui.add(build_button).clicked() {
				track_building_mode.active = !track_building_mode.active;
			}
		});
	});
}

fn panel_button(ui: &mut egui::Ui, shell_state: &mut UiShellState, label: &str, panel: ActivePanel) {
	let is_selected = shell_state.active_panel == panel;
	let button = egui::Button::new(label).selected(is_selected);

	if ui.add(button).clicked() {
		shell_state.active_panel = if is_selected {
			ActivePanel::None
		} else {
			panel
		};
	}
}
