use bevy::prelude::*;
use bevy_egui::{EguiContexts, egui};

use crate::alignment::TrackBuildingMode;
use crate::terrain::ContourState;
#[cfg(debug_assertions)]
use crate::debug_frame_limiter::DebugFrameLimiterState;

pub struct UiShellPlugin;

impl Plugin for UiShellPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<UiShellState>()
			.add_systems(bevy_egui::EguiPrimaryContextPass, bottom_bar_ui);
		#[cfg(debug_assertions)]
		app.add_systems(bevy_egui::EguiPrimaryContextPass, frame_limiter_ui);
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
	#[cfg(debug_assertions)]
	FrameRateLimiter,
}

fn bottom_bar_ui(
	mut contexts: EguiContexts,
	mut shell_state: ResMut<UiShellState>,
	mut track_building_mode: ResMut<TrackBuildingMode>,
	mut contour_state: ResMut<ContourState>,
	#[cfg(debug_assertions)] mut frame_limiter: ResMut<DebugFrameLimiterState>,
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

			#[cfg(debug_assertions)]
			{
				let limiter_label = if frame_limiter.enabled {
					format!("FPS Limit: {} Hz", frame_limiter.target_fps)
				} else {
					"FPS Limit: Off".to_owned()
				};
				let button = egui::Button::new(limiter_label).selected(frame_limiter.enabled);
				if ui.add(button).clicked() {
					frame_limiter.enabled = !frame_limiter.enabled;
					shell_state.active_panel = ActivePanel::FrameRateLimiter;
				}
			}

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

#[cfg(debug_assertions)]
pub(crate) fn frame_limiter_ui(
	mut contexts: EguiContexts,
	mut frame_limiter: ResMut<DebugFrameLimiterState>,
	ui_shell_state: Res<UiShellState>,
) {
	if ui_shell_state.active_panel != ActivePanel::FrameRateLimiter {
		return;
	}

	let Ok(ctx) = contexts.ctx_mut() else {
		return;
	};

	egui::Window::new("Frame Rate Limiter")
		.fixed_pos(egui::pos2(8.0, 8.0))
		.movable(false)
		.resizable(false)
		.show(ctx, |ui| {
			ui.horizontal(|ui| {
				ui.label("Limiter:");
				let status = if frame_limiter.enabled {
					"Enabled"
				} else {
					"Disabled"
				};
				ui.label(status);
			});

			ui.separator();
			ui.label("Target FPS (debug mode only):");

			let mut fps_i32 = i32::try_from(frame_limiter.target_fps).unwrap_or(i32::MAX);
			if ui
				.add(
					egui::DragValue::new(&mut fps_i32)
						.range(1..=i32::MAX)
						.speed(1.0),
				)
				.changed()
			{
				let clamped = fps_i32.max(1);
				frame_limiter.target_fps = u32::try_from(clamped).unwrap_or(u32::MAX);
			}
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
