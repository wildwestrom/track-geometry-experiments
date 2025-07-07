use bevy::prelude::*;

use crate::MainCamera;

#[derive(Component)]
pub struct HudText;

pub struct CameraDebugHud;

impl Plugin for CameraDebugHud {
	fn build(&self, app: &mut App) {
		app.add_systems(Startup, setup_hud)
			.add_systems(Update, update_hud);
	}
}

fn setup_hud(mut commands: Commands) {
	commands
		.spawn((Node {
			padding: UiRect::all(Val::Px(10.0)),
			justify_self: JustifySelf::Start,
			align_self: AlignSelf::Start,
			..default()
		},))
		.with_child((
			HudText,
			Text::new("Camera Transform: Loading..."),
			TextFont {
				font_size: 10.0,
				..default()
			},
			TextColor(Color::WHITE),
			TextLayout::new_with_justify(JustifyText::Left),
			Node { ..default() },
		));
}

fn update_hud(
	camera_query: Query<(&Transform, &Projection), With<MainCamera>>,
	mut hud_query: Query<&mut Text, With<HudText>>,
) {
	if let Ok(camera_transform) = camera_query.single() {
		if let Ok(mut hud_text) = hud_query.single_mut() {
			let translation = camera_transform.0.translation;
			let rotation = camera_transform.0.rotation;

			// Convert rotation to euler angles for display
			let euler = rotation.to_euler(EulerRot::XYZ);

			*hud_text = if let Projection::Perspective(persp) = camera_transform.1 {
				Text::new(format!(
					"Camera Transform:\n\
            Position: ({:.2}, {:.2}, {:.2})\n\
            Rotation: ({:.2}, {:.2}, {:.2}) deg\n\
			FOV: {:.2} deg",
					translation.x,
					translation.y,
					translation.z,
					euler.0.to_degrees(),
					euler.1.to_degrees(),
					euler.2.to_degrees(),
					persp.fov.to_degrees()
				))
			} else {
				Text::new(format!(
					"Camera Transform:\n\
            Position: ({:.2}, {:.2}, {:.2})\n\
            Rotation: ({:.2}, {:.2}, {:.2}) deg",
					translation.x,
					translation.y,
					translation.z,
					euler.0.to_degrees(),
					euler.1.to_degrees(),
					euler.2.to_degrees()
				))
			};
		}
	}
}
