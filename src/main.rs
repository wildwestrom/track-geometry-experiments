use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::EguiPlugin;

mod alignment;
mod camera;
mod hud;
mod pin;
mod saveable;
mod spatial;
mod terrain;

use crate::alignment::AlignmentPlugin;
use crate::camera::CameraPlugin;
use crate::pin::PinPlugin;
use crate::terrain::TerrainPlugin;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins.set(RenderPlugin {
			render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
				features: WgpuFeatures::POLYGON_MODE_LINE,
				..default()
			}),
			..default()
		}))
		.add_plugins(EguiPlugin::default())
		.add_plugins(CameraPlugin)
		.add_plugins(TerrainPlugin)
		.add_plugins(PinPlugin)
		.add_plugins(AlignmentPlugin)
		.add_plugins(WireframePlugin::default())
		.insert_resource(WireframeConfig {
			global: false,
			default_color: Color::srgb(1.0, 1.0, 1.0),
		})
		.add_systems(Update, (toggle_wireframe_system,))
		.run();
}

fn toggle_wireframe_system(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut config: ResMut<WireframeConfig>,
) {
	if keyboard_input.just_pressed(KeyCode::Space) {
		config.global = !config.global;
		info!(
			"Wireframe mode: {}",
			if config.global { "ON" } else { "OFF" }
		);
	}
}
