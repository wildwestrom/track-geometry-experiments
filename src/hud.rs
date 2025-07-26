use bevy::diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy_panorbit_camera::PanOrbitCamera;

#[derive(Resource, Default)]
struct HudText(String);

pub(crate) struct CameraDebugHud;

impl Plugin for CameraDebugHud {
    fn build(&self, app: &mut App) {
        app.init_resource::<HudText>()
            .add_systems(Startup, setup_hud)
            .add_systems(Update, (update_hud, update_ui_text))
            .add_plugins(FrameTimeDiagnosticsPlugin::default());
    }
}

fn setup_hud(mut commands: Commands) {
    commands
        .spawn(Node {
            padding: UiRect::all(Val::Px(10.0)),
            justify_self: JustifySelf::Start,
            align_self: AlignSelf::Start,
            ..default()
        })
        .with_child((
            Text::new("Loading..."),
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
    camera_query: Query<(&Transform, &Projection), With<PanOrbitCamera>>,
    mut hud_text: ResMut<HudText>,
    diagnostics: Res<DiagnosticsStore>,
) {
    let Ok(camera_transform) = camera_query.single() else {
        return;
    };

    let translation = camera_transform.0.translation;
    let (tr_x, tr_y, tr_z) = (translation.x, translation.y, translation.z);
    let rotation = camera_transform.0.rotation;
    let (rot_x, rot_y, rot_z, rot_w) = (rotation.x, rotation.y, rotation.z, rotation.w);
    let (euler_y, euler_x, euler_z) = {
        let (r1, r2, r3) = rotation.to_euler(EulerRot::YXZ);
        (r1.to_degrees(), r2.to_degrees(), r3.to_degrees())
    };

    let mut text = String::from("");
    text.push_str("Camera Transform:\n");
    text.push_str(&format!("\tPosition: ({tr_x:.2}, {tr_y:.2}, {tr_z:.2})\n"));
    text.push_str(&format!(
        "\tRotation YXZ: ({euler_y:.2}, {euler_x:.2}, {euler_z:.2}) deg\n"
    ));
    text.push_str(&format!(
        "\tRotation Quat: ({rot_x:.2}, {rot_y:.2}, {rot_z:.2}, {rot_w:.2})\n"
    ));

    if let Projection::Perspective(persp) = camera_transform.1 {
        let fov = persp.fov.to_degrees();
        text.push_str(&format!("\tFOV: {fov:.2} deg\n"));
    }

    if let Some(fps) = diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|fps_diag| fps_diag.smoothed())
    {
        text.push_str(&format!("\nFPS: {fps:.1}"));
    };

    hud_text.0 = text;
}

fn update_ui_text(hud_text: Res<HudText>, mut text_query: Query<&mut Text>) {
    if let Ok(mut text) = text_query.single_mut() {
        *text = Text::new(&hud_text.0);
    }
}
