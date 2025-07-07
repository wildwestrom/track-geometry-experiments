use bevy::{
	prelude::*,
	render::{
		settings::{WgpuFeatures, WgpuSettings},
		RenderPlugin,
	},
};
use bevy_egui::EguiPlugin;

mod camera;
mod hud;
mod terrain;
use camera::CameraPlugin;

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
		.run();
}
