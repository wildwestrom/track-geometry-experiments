use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::EguiPlugin;
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use std::f32::consts::PI;

mod hud;
use hud::CameraDebugHud;
mod terrain;
use terrain::TerrainPlugin;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins.set(RenderPlugin {
			render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
				features: WgpuFeatures::POLYGON_MODE_LINE,
				..default()
			}),
			..default()
		}))
		.add_plugins(PanOrbitCameraPlugin)
		.add_plugins(EguiPlugin::default())
		.add_plugins(CameraDebugHud)
		.add_plugins(TerrainPlugin)
		.add_systems(Startup, setup)
		.run();
}

fn setup(mut commands: Commands) {
	commands.spawn((
		Transform::from_translation(Vec3::new(0.0, 1.5, 5.0)),
		bevy_panorbit_camera::PanOrbitCamera::default(),
	));
	commands.spawn((
		DirectionalLight {
			illuminance: light_consts::lux::OVERCAST_DAY,
			shadows_enabled: true,
			..default()
		},
		Transform {
			translation: Vec3::new(0.0, 2.0, 0.0),
			rotation: Quat::from_rotation_x(-PI / 4.),
			..default()
		},
	));

	commands.spawn((
		Camera2d,
		Camera {
			order: 1,
			..default()
		},
	));
}
