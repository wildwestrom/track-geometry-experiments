use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		camera::ScalingMode,
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::EguiPlugin;
use std::f32::consts::PI;

mod hud;
use hud::CameraDebugHud;
mod terrain;
use terrain::TerrainPlugin;

#[derive(Resource, Default)]
struct CameraMode {
	current_mode: CameraState,
	transition_progress: f32,
	is_transitioning: bool,
	transition_stage: TransitionStage,
	transition_direction: TransitionDirection,
}

#[derive(Clone, Copy, PartialEq)]
enum CameraState {
	PerspectiveAngled,
	OrthographicTopDown,
}

#[derive(Clone, Copy, PartialEq)]
enum TransitionStage {
	None,
	PositionTransition,   // Moving from angled to top-down position (perspective)
	ProjectionTransition, // Switching from perspective to orthographic (top-down position)
}

#[derive(Clone, Copy, PartialEq)]
enum TransitionDirection {
	ToOrthographic, // Going from perspective angled to orthographic top-down
	ToPerspective,  // Going from orthographic top-down to perspective angled
}

impl Default for CameraState {
	fn default() -> Self {
		CameraState::PerspectiveAngled
	}
}

impl Default for TransitionStage {
	fn default() -> Self {
		TransitionStage::None
	}
}

impl Default for TransitionDirection {
	fn default() -> Self {
		TransitionDirection::ToOrthographic
	}
}

impl CameraState {
	fn next(self) -> Self {
		match self {
			CameraState::PerspectiveAngled => CameraState::OrthographicTopDown,
			CameraState::OrthographicTopDown => CameraState::PerspectiveAngled,
		}
	}
}

#[derive(Component)]
struct MainCamera;

fn main() {
	App::new()
		.add_plugins(DefaultPlugins.set(RenderPlugin {
			render_creation: bevy::render::settings::RenderCreation::Automatic(WgpuSettings {
				features: WgpuFeatures::POLYGON_MODE_LINE,
				..default()
			}),
			..default()
		}))
		//.add_plugins(PanOrbitCameraPlugin)
		.add_plugins(EguiPlugin::default())
		.add_plugins(CameraDebugHud)
		.add_plugins(TerrainPlugin)
		.insert_resource(CameraMode::default())
		.add_systems(Startup, setup)
		.add_systems(Update, (toggle_camera, update_camera_transition))
		.run();
}

fn setup(mut commands: Commands) {
	let persp_angled = Projection::from(PerspectiveProjection {
		fov: PI / 4.0,
		far: 10000.0,
		..default()
	});

	// Spawn single main camera
	commands.spawn((
		Transform::from_translation(Vec3::new(0.0, 1000.0, 1200.0)).looking_at(Vec3::ZERO, Vec3::Y),
		persp_angled,
		Camera3d::default(),
		Camera {
			order: 0,
			..default()
		},
		MainCamera,
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

fn toggle_camera(keyboard_input: Res<ButtonInput<KeyCode>>, mut camera_mode: ResMut<CameraMode>) {
	if keyboard_input.just_pressed(KeyCode::KeyT) {
		// Always allow toggling, even during transitions
		let new_mode = camera_mode.current_mode.next();
		camera_mode.transition_direction = if new_mode == CameraState::OrthographicTopDown {
			TransitionDirection::ToOrthographic
		} else {
			TransitionDirection::ToPerspective
		};
		camera_mode.current_mode = new_mode;
		camera_mode.transition_progress = 0.0;
		camera_mode.is_transitioning = true;
		camera_mode.transition_stage = TransitionStage::PositionTransition;
	}
}

fn update_camera_transition(
	time: Res<Time>,
	mut camera_mode: ResMut<CameraMode>,
	mut camera_query: Query<(&mut Transform, &mut Projection), With<MainCamera>>,
) {
	if !camera_mode.is_transitioning {
		return;
	}

	let transition_duration = 1.0; // seconds
	camera_mode.transition_progress += time.delta().as_secs_f32() / transition_duration;

	if camera_mode.transition_progress >= 1.0 {
		// Move to next stage or finish transition
		match camera_mode.transition_stage {
			TransitionStage::PositionTransition => {
				// Position transition complete, start projection transition
				camera_mode.transition_progress = 0.0;
				camera_mode.transition_stage = TransitionStage::ProjectionTransition;
			}
			TransitionStage::ProjectionTransition => {
				// Projection transition complete, finish
				camera_mode.transition_progress = 1.0;
				camera_mode.is_transitioning = false;
				camera_mode.transition_stage = TransitionStage::None;
			}
			TransitionStage::None => {
				camera_mode.is_transitioning = false;
			}
		}
		return;
	}

	let t = camera_mode.transition_progress;
	let t_smooth = EaseFunction::SmoothStep.sample(t).unwrap(); // Apply easing

	// Handle different transition stages
	match camera_mode.transition_stage {
		TransitionStage::PositionTransition => {
			match camera_mode.transition_direction {
				TransitionDirection::ToOrthographic => {
					// Going from angled to top-down position (keeping perspective)
					let (from_transform, from_projection) =
						get_camera_state(CameraState::PerspectiveAngled);
					let (to_transform, to_projection) = get_perspective_top_down_state();

					if let Ok((mut transform, mut projection)) = camera_query.single_mut() {
						transform.translation = from_transform
							.translation
							.lerp(to_transform.translation, t_smooth);
						transform.rotation = from_transform
							.rotation
							.slerp(to_transform.rotation, t_smooth);
						// Smoothly interpolate the perspective projection during position transition
						*projection =
							interpolate_projection(&from_projection, &to_projection, t_smooth);
					}
				}
				TransitionDirection::ToPerspective => {
					// Going from top-down to angled position (keeping perspective)
					let (from_transform, from_projection) = get_perspective_top_down_state();
					let (to_transform, to_projection) =
						get_camera_state(CameraState::PerspectiveAngled);

					if let Ok((mut transform, mut projection)) = camera_query.single_mut() {
						transform.translation = from_transform
							.translation
							.lerp(to_transform.translation, t_smooth);
						transform.rotation = from_transform
							.rotation
							.slerp(to_transform.rotation, t_smooth);
						// Smoothly interpolate the perspective projection during position transition
						*projection =
							interpolate_projection(&from_projection, &to_projection, t_smooth);
					}
				}
			}
		}
		TransitionStage::ProjectionTransition => {
			match camera_mode.transition_direction {
				TransitionDirection::ToOrthographic => {
					// Transition from perspective to orthographic (keeping top-down position)
					let (_, from_projection) = get_perspective_top_down_state();
					let (_, to_projection) = get_camera_state(CameraState::OrthographicTopDown);

					if let Ok((_, mut projection)) = camera_query.single_mut() {
						*projection =
							interpolate_projection(&from_projection, &to_projection, t_smooth);
					}
				}
				TransitionDirection::ToPerspective => {
					// Transition from orthographic to perspective (keeping top-down position)
					let (_, from_projection) = get_camera_state(CameraState::OrthographicTopDown);
					let (_, to_projection) = get_perspective_top_down_state();

					if let Ok((_, mut projection)) = camera_query.single_mut() {
						*projection =
							interpolate_projection(&from_projection, &to_projection, t_smooth);
					}
				}
			}
		}
		TransitionStage::None => {}
	}
}

fn get_camera_state(state: CameraState) -> (Transform, Projection) {
	match state {
		CameraState::PerspectiveAngled => {
			let transform = Transform::from_translation(Vec3::new(0.0, 1000.0, 1200.0))
				.looking_at(Vec3::ZERO, Vec3::Y);
			let projection = Projection::from(PerspectiveProjection {
				fov: PI / 4.0,
				far: 10000.0,
				..default()
			});
			(transform, projection)
		}
		CameraState::OrthographicTopDown => {
			let transform = Transform::from_translation(Vec3::new(0.0, 1200.0, 0.0))
				.looking_at(Vec3::ZERO, Vec3::Y);
			let projection = Projection::from(OrthographicProjection {
				scale: 1.0,
				near: 0.1,
				far: 3000.0,
				viewport_origin: Vec2::new(0.5, 0.5),
				scaling_mode: ScalingMode::FixedVertical {
					viewport_height: 1000.0,
				},
				area: Rect::new(-1.0, -1.0, 1.0, 1.0),
			});
			(transform, projection)
		}
	}
}

// Helper function to get perspective top-down state (intermediate state)
fn get_perspective_top_down_state() -> (Transform, Projection) {
	let transform =
		Transform::from_translation(Vec3::new(0.0, 1200.0, 0.0)).looking_at(Vec3::ZERO, Vec3::Y);
	let projection = Projection::from(PerspectiveProjection {
		fov: PI / 4.0,
		far: 10000.0,
		..default()
	});
	(transform, projection)
}

fn interpolate_projection(from: &Projection, to: &Projection, t: f32) -> Projection {
	match (from, to) {
		(Projection::Perspective(from_persp), Projection::Perspective(to_persp)) => {
			Projection::from(PerspectiveProjection {
				fov: from_persp.fov.lerp(to_persp.fov, t),
				near: from_persp.near.lerp(to_persp.near, t),
				far: from_persp.far.lerp(to_persp.far, t),
				..default()
			})
		}
		(Projection::Orthographic(from_ortho), Projection::Orthographic(to_ortho)) => {
			Projection::from(OrthographicProjection {
				scale: from_ortho.scale.lerp(to_ortho.scale, t),
				near: from_ortho.near.lerp(to_ortho.near, t),
				far: from_ortho.far.lerp(to_ortho.far, t),
				viewport_origin: from_ortho.viewport_origin.lerp(to_ortho.viewport_origin, t),
				scaling_mode: to_ortho.scaling_mode, // Keep target scaling mode
				area: Rect::new(
					from_ortho.area.min.x.lerp(to_ortho.area.min.x, t),
					from_ortho.area.min.y.lerp(to_ortho.area.min.y, t),
					from_ortho.area.max.x.lerp(to_ortho.area.max.x, t),
					from_ortho.area.max.y.lerp(to_ortho.area.max.y, t),
				),
			})
		}
		// For mixed transitions, use the target projection type
		(_, Projection::Perspective(to_persp)) => Projection::from(PerspectiveProjection {
			fov: to_persp.fov,
			near: to_persp.near,
			far: to_persp.far,
			..default()
		}),
		(_, Projection::Orthographic(to_ortho)) => Projection::from(OrthographicProjection {
			scale: to_ortho.scale,
			near: to_ortho.near,
			far: to_ortho.far,
			viewport_origin: to_ortho.viewport_origin,
			scaling_mode: to_ortho.scaling_mode,
			area: to_ortho.area,
		}),
		// Handle Custom projection by using the target projection
		(_, Projection::Custom(_)) => to.clone(),
	}
}
