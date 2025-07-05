use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		settings::{WgpuFeatures, WgpuSettings},
	},
};

fn main() {
	App::new()
		.add_plugins(DefaultPlugins.set(RenderPlugin {
			render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
				features: WgpuFeatures::POLYGON_MODE_LINE,
				..default()
			}),
			..default()
		}))
		.add_systems(Startup, || {})
		.run();
}
