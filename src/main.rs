use bevy::pbr::wireframe::{WireframeConfig, WireframePlugin};
#[cfg(not(target_arch = "wasm32"))]
use bevy::render::settings::WgpuFeatures;
use bevy::{
	prelude::*,
	render::{RenderPlugin, settings::WgpuSettings},
};
use bevy_egui::EguiPlugin;

mod alignment;
mod camera;
#[cfg(debug_assertions)]
mod debug_frame_limiter;
mod hud;
mod pin;
mod saveable;
mod terrain;
mod ui_shell;

use crate::alignment::AlignmentPlugin;
use crate::camera::CameraPlugin;
#[cfg(debug_assertions)]
use crate::debug_frame_limiter::DebugFrameLimiterPlugin;
use crate::pin::PinPlugin;
use crate::terrain::TerrainPlugin;
use crate::ui_shell::UiShellPlugin;

const HUD: bool = true;

fn main() {
	#[cfg(not(target_arch = "wasm32"))]
	let wgpu_settings = WgpuSettings {
		features: WgpuFeatures::POLYGON_MODE_LINE,
		..default()
	};
	#[cfg(target_arch = "wasm32")]
	let wgpu_settings = WgpuSettings::default();

	let mut app = App::new();
	app
		.add_plugins(
			DefaultPlugins
				.set(RenderPlugin {
					render_creation: bevy::render::settings::RenderCreation::Automatic(
						wgpu_settings,
					),
					..default()
				})
				.set(WindowPlugin {
					primary_window: Some(Window {
						fit_canvas_to_parent: true,
						..default()
					}),
					..default()
				})
				// On WASM, the asset server fetches `.meta` files for every asset.
				// Since we don't generate them, the server returns an HTML response
				// that Bevy can't parse, causing assets to silently fail to load.
				.set(AssetPlugin {
					meta_check: bevy::asset::AssetMetaCheck::Never,
					..default()
				}),
		)
		.add_plugins(EguiPlugin::default())
		.add_plugins(UiShellPlugin)
		.add_plugins(CameraPlugin)
		.add_plugins(TerrainPlugin)
		.add_plugins(PinPlugin)
		.add_plugins(AlignmentPlugin)
		.add_plugins(WireframePlugin::default())
		.insert_resource(WireframeConfig {
			global: false,
			default_color: Color::srgb(1.0, 1.0, 1.0),
		})
		.add_systems(Update, toggle_wireframe_system);

	#[cfg(debug_assertions)]
	app.add_plugins(DebugFrameLimiterPlugin);

	if HUD {
		app.add_plugins(hud::CameraDebugHud);
	}

	app.run();
}

fn toggle_wireframe_system(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut config: ResMut<WireframeConfig>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyW) {
		config.global = !config.global;
		debug!(
			"Wireframe mode: {}",
			if config.global { "ON" } else { "OFF" }
		);
	}
}
