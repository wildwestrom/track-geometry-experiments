use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

#[derive(Component)]
struct HudText;

pub(crate) struct CameraDebugHud;

impl Plugin for CameraDebugHud {
	fn build(&self, app: &mut App) {
		app
			.add_systems(Startup, setup_hud)
			.add_systems(Update, update_hud)
			.add_plugins(FrameTimeDiagnosticsPlugin::default());
	}
}

fn setup_hud(mut commands: Commands) {
	commands
		.spawn(Node {
			padding: UiRect::all(Val::Px(10.0)),
			justify_self: JustifySelf::End,
			align_self: AlignSelf::Start,
			..default()
		})
		.with_child((
			Text::new("Loading..."),
			HudText,
			TextFont {
				font_size: 10.0,
				..default()
			},
			TextColor(Color::WHITE),
			TextLayout::new_with_justify(Justify::Left),
			Node { ..default() },
		));
}

fn update_hud(
	mut hud_text: Single<&mut Text, With<HudText>>,
	camera_query: Single<(&Transform, &Projection), With<PanOrbitCamera>>,
	diagnostics: Res<DiagnosticsStore>,
) {
	let (camera_transform, camera_projection) = *camera_query;
	let translation = camera_transform.translation;
	let (tr_x, tr_y, tr_z) = (translation.x, translation.y, translation.z);
	let rotation = camera_transform.rotation;
	let (rot_x, rot_y, rot_z, rot_w) = (rotation.x, rotation.y, rotation.z, rotation.w);
	let (euler_y, euler_x, euler_z) = {
		let (r1, r2, r3) = rotation.to_euler(EulerRot::YXZ);
		(r1.to_degrees(), r2.to_degrees(), r3.to_degrees())
	};

	let mut text = String::new();
	text.push_str("Camera Transform:");
	text.push_str(&format!("\n\tPosition: ({tr_x:.2}, {tr_y:.2}, {tr_z:.2})"));
	text.push_str(&format!(
		"\n\tRotation YXZ: ({euler_y:.2}, {euler_x:.2}, {euler_z:.2}) deg"
	));
	text.push_str(&format!(
		"\n\tRotation Quat: ({rot_x:.2}, {rot_y:.2}, {rot_z:.2}, {rot_w:.2})"
	));

	if let Projection::Perspective(persp) = camera_projection {
		let fov = persp.fov.to_degrees();
		text.push_str(&format!("\n\tFOV: {fov:.2} deg"));
	}

	if let Some(fps) = diagnostics
		.get(&FrameTimeDiagnosticsPlugin::FPS)
		.and_then(|fps_diag| fps_diag.smoothed())
	{
		text.push_str(&format!("\nFPS: {fps:.1}"));
	};

	hud_text.0 = text;
}
