use bevy::prelude::*;
use bevy::window::{PresentMode, PrimaryWindow};
use bevy_egui::{EguiContexts, egui};

use crate::alignment::{MAX_SNAP_ANGLE_DEGREES, MIN_SNAP_ANGLE_DEGREES};
use crate::alignment::{TangentSnapSettings, TrackBuildingMode};
use crate::debug_frame_limiter::FrameLimiterState;
use crate::terrain::ContourState;

pub struct UiShellPlugin;

impl Plugin for UiShellPlugin {
	fn build(&self, app: &mut App) {
		app.init_resource::<UiShellState>().add_systems(
			bevy_egui::EguiPrimaryContextPass,
			(bottom_bar_ui, settings_ui),
		);
	}
}

#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UiShellState {
	pub active_panel: ActivePanel,
	pub alignment_tab: AlignmentTab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AlignmentTab {
	#[default]
	Horizontal,
	Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ActivePanel {
	#[default]
	None,
	TerrainControls,
	ContourLines,
	AlignmentProperties,
	Visualizations,
	Settings,
}

fn bottom_bar_ui(
	mut contexts: EguiContexts,
	mut shell_state: ResMut<UiShellState>,
	mut track_building_mode: ResMut<TrackBuildingMode>,
	mut contour_state: ResMut<ContourState>,
) {
	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	egui::TopBottomPanel::bottom("main_menu_bar").show(ctx, |ui| {
		ui.horizontal_centered(|ui| {
			ui.spacing_mut().item_spacing.x = 8.0;
			panel_button(
				ui,
				&mut shell_state,
				"Terrain",
				ActivePanel::TerrainControls,
			);
			let contour_enabled = contour_state.enabled();
			let contour_button = egui::Button::new("Contour").selected(contour_enabled);
			if ui.add(contour_button).clicked() {
				contour_state.set_enabled(!contour_enabled);
				shell_state.active_panel = ActivePanel::ContourLines;
			}
			panel_button(
				ui,
				&mut shell_state,
				"Alignment",
				ActivePanel::AlignmentProperties,
			);
			panel_button(
				ui,
				&mut shell_state,
				"Visualizations",
				ActivePanel::Visualizations,
			);

			let build_label = if track_building_mode.active {
				"Exit Build (Esc/F)"
			} else {
				"Build Track (F)"
			};
			let build_button = egui::Button::new(build_label).selected(track_building_mode.active);
			if ui.add(build_button).clicked() {
				track_building_mode.active = !track_building_mode.active;
			}

			panel_button(ui, &mut shell_state, "Settings", ActivePanel::Settings);
		});
	});
}

fn settings_ui(
	mut contexts: EguiContexts,
	ui_shell_state: Res<UiShellState>,
	mut snap_settings: ResMut<TangentSnapSettings>,
	mut windows: Query<&mut Window, With<PrimaryWindow>>,
	mut frame_limiter: ResMut<FrameLimiterState>,
) {
	if ui_shell_state.active_panel != ActivePanel::Settings {
		return;
	}

	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	let Ok(mut window) = windows.single_mut() else {
		warn!("No window found!");
		return;
	};

	egui::Window::new("Settings")
		.fixed_pos(egui::pos2(8.0, 8.0))
		.movable(false)
		.resizable(false)
		.show(ctx, |ui| {
			egui::Grid::new("settings_grid")
				.num_columns(2)
				.spacing(egui::vec2(8.0, 6.0))
				.show(ui, |ui| {
					ui.label("VSync");
					let mut vsync = window.present_mode == PresentMode::AutoVsync;
					if ui.checkbox(&mut vsync, "").changed() {
						window.present_mode = if vsync {
							PresentMode::AutoVsync
						} else {
							PresentMode::AutoNoVsync
						};
					}
					ui.end_row();
					{
						ui.label("FPS limit");
						ui.checkbox(&mut frame_limiter.enabled, "Enabled");
						ui.end_row();
						if frame_limiter.enabled {
							ui.label("Target FPS");
							let mut fps_i32 = i32::try_from(frame_limiter.target_fps).unwrap_or(i32::MAX);
							if ui
								.add(
									egui::DragValue::new(&mut fps_i32)
										.range(1..=i32::MAX)
										.speed(1.0),
								)
								.changed()
							{
								frame_limiter.target_fps = u32::try_from(fps_i32.max(1)).unwrap_or(u32::MAX);
							}
							ui.end_row();
						}
					}
					ui.label("Snap angle");
					ui.add(
						egui::Slider::new(
							&mut snap_settings.angle_degrees,
							MIN_SNAP_ANGLE_DEGREES..=MAX_SNAP_ANGLE_DEGREES,
						)
						.step_by(0.1)
						.suffix(" deg"),
					);
					ui.end_row();
					ui.label("Hysteresis");
					ui.add(
						egui::Slider::new(&mut snap_settings.hysteresis_degrees, 0.0..=5.0)
							.step_by(0.1)
							.suffix(" deg"),
					);
					ui.end_row();
				});
		});
}

fn panel_button(
	ui: &mut egui::Ui,
	shell_state: &mut UiShellState,
	label: &str,
	panel: ActivePanel,
) {
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
