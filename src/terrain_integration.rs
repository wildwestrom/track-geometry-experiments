use crate::saveable::SaveableSettings;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_procedural_terrain_gen as terrain;
use log::{error, info};
use terrain::{ElevationProfileSamples, PlotWidth, TerrainControlsUiExt};

use crate::alignment::AlignmentState;

/// Plugin to bridge track alignment elevations into the bevy_terrain_gen visualization.
pub struct TerrainIntegrationPlugin;

impl Plugin for TerrainIntegrationPlugin {
	fn build(&self, app: &mut App) {
		// Ensure the terrain plugin has a buffer to read elevation samples from.
		app
			.insert_resource(terrain::Settings::load_or_default())
			.init_resource::<ElevationProfileSamples>()
			.add_systems(Update, alignment_profile_adapter)
			.init_resource::<terrain::TerrainControlsUiExt>()
			.add_systems(Startup, register_terrain_controls_ext);
	}
}

/// Adapts the current AlignmentState into elevation profile samples consumed by bevy_terrain_gen.
///
/// Behavior:
/// - If there's a current alignment, we sample its elevation profile across the current plot width
///   (or a reasonable default) and write the results into `ElevationProfileSamples`.
/// - If no alignment is active/available, we clear the samples so the plot panel shows nothing.
///
/// Notes:
/// - The terrain plugin normalizes/plots the Y values and distributes X across the plot width, so
///   we only need to provide ordered (index, height) pairs here; the provided X values are not used
///   for pixel placement.
fn alignment_profile_adapter(
	alignment_state: Option<Res<AlignmentState>>,
	plot_width: Option<Res<PlotWidth>>,
	mut profile: ResMut<ElevationProfileSamples>,
) {
	// Decide the number of samples we want across the plot width
	let width = plot_width.as_ref().map(|w| w.0).unwrap_or(512).max(2);

	let Some(state) = alignment_state else {
		profile.0.clear();
		return;
	};

	let Some(alignment) = state.alignments.get(&state.turns) else {
		profile.0.clear();
		return;
	};

	// Use the alignment helper to produce (x, height) pairs. The terrain plugin only uses the
	// heights (and the order), so we pass them through directly.
	let samples = alignment.sample_elevation_profile(width);
	profile.0 = samples;
}

impl SaveableSettings for terrain::Settings {
	fn filename() -> &'static str {
		"terrain_settings.json"
	}
}

fn terrain_controls_save_load_buttons(ui: &mut egui::Ui, settings: &mut terrain::Settings) {
	settings.handle_save_operation_ui(ui, "Save Settings");

	if ui.button("Load Settings").clicked() {
		match terrain::Settings::load() {
			Ok(loaded) => {
				*settings = loaded;
				info!("Loaded terrain_settings.json");
			}
			Err(e) => {
				error!("Failed to load terrain settings: {}", e);
			}
		}
	}
}

fn register_terrain_controls_ext(mut ext: ResMut<TerrainControlsUiExt>) {
	ext.callbacks.push(terrain_controls_save_load_buttons);
}
