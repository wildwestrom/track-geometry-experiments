use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		render_asset::RenderAssetUsages,
		render_resource::{Extent3d, TextureDimension, TextureFormat},
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::{EguiContexts, EguiPlugin, egui};
use bevy_panorbit_camera::PanOrbitCameraPlugin;
use noise::{NoiseFn, OpenSimplex};
use std::f32::consts::PI;

mod hud;

#[derive(Resource)]
struct NoiseParameters {
	seed: u32,
	offset_x: f32,
	offset_y: f32,
	scale: f32,
	octaves: u8,
	persistence: f32,
	lacunarity: f32,
	valley_exponent: f32,
	fudge_factor: f32,
}

impl Default for NoiseParameters {
	fn default() -> Self {
		Self {
			seed: 42,
			offset_x: 0.0,
			offset_y: 0.0,
			scale: 0.15,
			octaves: 4,
			persistence: 0.5,
			lacunarity: 2.0,
			valley_exponent: 1.0,
			fudge_factor: 1.2,
		}
	}
}

#[derive(Component)]
struct NoiseTexture;

const NOISE_MAX: f64 = 0.544;

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
		//.add_plugins(CameraDebugHud)
		.insert_resource(NoiseParameters::default())
		.add_systems(Startup, setup)
		.add_systems(Update, update_noise_texture)
		.add_systems(bevy_egui::EguiPrimaryContextPass, ui_system)
		.run();
}

fn setup(
	mut commands: Commands,
	mut meshes: ResMut<Assets<Mesh>>,
	mut materials: ResMut<Assets<StandardMaterial>>,
	mut images: ResMut<Assets<Image>>,
	noise_params: Res<NoiseParameters>,
) {
	commands.spawn((
		Mesh3d(meshes.add(Plane3d::default().mesh().size(5.0, 5.0).subdivisions(8))),
		MeshMaterial3d(materials.add(Color::srgb(0.3, 0.5, 0.3))),
	));

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

	let noise_texture = generate_noise_texture(128, 128, &noise_params);
	let noise_handle = images.add(noise_texture);

	commands.spawn((
		Camera2d,
		Camera {
			order: 1,
			..default()
		},
	));

	commands
		.spawn((
			ImageNode::new(noise_handle),
			NoiseTexture,
			Node {
				justify_self: JustifySelf::End,
				align_self: AlignSelf::Start,
				width: Val::Px(256.0),
				height: Val::Px(256.0),
				padding: UiRect::all(Val::Px(10.0)),
				..default()
			},
		))
		.with_child((
			Text::new("Noise Preview"),
			TextFont {
				font_size: 24.0,
				..default()
			},
			TextColor(Color::WHITE),
			Node { ..default() },
		));
}

fn ui_system(mut contexts: EguiContexts, mut noise_params: ResMut<NoiseParameters>) {
	if let Ok(ctx) = contexts.ctx_mut() {
		egui::Window::new("Noise Controls").show(ctx, |ui| {
			ui.label("Seed:");
			ui.add(egui::DragValue::new(&mut noise_params.seed).speed(1.0));

			ui.label("Offset X:");
			ui.add(
				egui::Slider::new(&mut noise_params.offset_x, -1000.0..=1000.0)
					.step_by(1.0)
					.text("Offset X"),
			);

			ui.label("Offset Y:");
			ui.add(
				egui::Slider::new(&mut noise_params.offset_y, -1000.0..=1000.0)
					.step_by(1.0)
					.text("Offset Y"),
			);

			ui.label("Scale:");
			ui.add(egui::Slider::new(&mut noise_params.scale, 0.01..=0.5).text("Scale"));

			ui.label("Octaves:");
			ui.add(
				egui::Slider::new(&mut noise_params.octaves, 1..=8)
					.text("Octaves")
					.step_by(1.0),
			);

			ui.label("Persistence:");
			ui.add(egui::Slider::new(&mut noise_params.persistence, 0.1..=0.6).text("Persistence"));

			ui.label("Lacunarity:");
			ui.add(egui::Slider::new(&mut noise_params.lacunarity, 1.01..=4.0).text("Lacunarity"));

			ui.separator();
			ui.label("Valley Controls:");

			ui.label("Valley Exponent:");
			ui.add(
				egui::Slider::new(&mut noise_params.valley_exponent, 0.0..=10.0)
					.text("Valley Exponent"),
			);

			ui.label("Fudge Factor:");
			ui.add(
				egui::Slider::new(&mut noise_params.fudge_factor, 0.5..=2.0).text("Fudge Factor"),
			);
		});
	}
}

fn update_noise_texture(
	mut noise_texture_query: Query<&mut ImageNode, With<NoiseTexture>>,
	mut images: ResMut<Assets<Image>>,
	noise_params: Res<NoiseParameters>,
) {
	if let Ok(mut image_node) = noise_texture_query.single_mut() {
		let new_texture = generate_noise_texture(128, 128, &noise_params);
		let new_handle = images.add(new_texture);
		*image_node = ImageNode::new(new_handle);
	}
}

fn generate_noise_texture(width: u32, height: u32, params: &NoiseParameters) -> Image {
	let noise = OpenSimplex::new(params.seed);
	let mut texture_data = Vec::with_capacity((width * height * 4) as usize);

	for y in 0..height {
		for x in 0..width {
			let mut amplitude = 1.0;
			let mut frequency = 1.0;
			let mut noise_value = 0.0;
			let mut max_value = 0.0;

			// Generate fractal noise with multiple octaves
			for _ in 0..params.octaves {
				let sample_x =
					(x as f64 + params.offset_x as f64) * params.scale as f64 * frequency;
				let sample_y =
					(y as f64 + params.offset_y as f64) * params.scale as f64 * frequency;

				let raw_noise_sample =
					(noise.get([sample_x, sample_y]) / NOISE_MAX).clamp(-1.0, 1.0);
				noise_value += (raw_noise_sample) * amplitude;
				max_value += amplitude;

				amplitude *= params.persistence as f64;
				frequency *= params.lacunarity as f64;
			}

			// Normalize the noise value
			noise_value /= max_value;

			// Apply valley transformation using power function
			// Convert from [-1, 1] to [0, 1] range first
			let normalized_value = (noise_value + 1.0) * 0.5;
			// Apply fudge factor and power function
			let valley_value =
				(normalized_value * params.fudge_factor as f64).powf(params.valley_exponent as f64);
			// Convert back to [0, 255] range
			let pixel_value = (valley_value * 255.0) as u8;

			texture_data.push(pixel_value); // R
			texture_data.push(pixel_value); // G
			texture_data.push(pixel_value); // B
			texture_data.push(255); // A
		}
	}

	Image::new_fill(
		Extent3d {
			width,
			height,
			depth_or_array_layers: 1,
		},
		TextureDimension::D2,
		&texture_data,
		TextureFormat::Rgba8UnormSrgb,
		RenderAssetUsages::all(),
	)
}
