use crate::saveable::SaveableSettings;
use bevy::prelude::*;
use bevy_egui::egui;
use bevy_procedural_terrain_gen as terrain;
use log::{error, info};
use terrain::TerrainControlsUiExt;

/// Plugin to bridge track alignment elevations into the bevy_terrain_gen visualization.
pub struct TerrainIntegrationPlugin;

impl Plugin for TerrainIntegrationPlugin {
	fn build(&self, app: &mut App) {
		// Ensure the terrain plugin has a buffer to read elevation samples from.
		app
			.insert_resource(terrain::Settings::load_or_default())
			.init_resource::<terrain::TerrainControlsUiExt>()
			.add_systems(Startup, register_terrain_controls_ext);
	}
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
