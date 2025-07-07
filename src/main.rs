use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		settings::{WgpuFeatures, WgpuSettings},
	},
};
use bevy_egui::EguiPlugin;
use bevy_tweening::*;
use std::f32::consts::PI;

mod hud;
use hud::CameraDebugHud;
mod terrain;
use terrain::TerrainPlugin;
use terrain::WORLD_SIZE;

#[derive(Resource, Default)]
struct CameraMode {
	current_mode: CameraState,
	is_transitioning: bool,
	transition_timer: Timer,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum CameraState {
	PerspectiveAngled,
	OrthographicTopDown,
}

impl Default for CameraState {
	fn default() -> Self {
		CameraState::PerspectiveAngled
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

#[derive(Debug, Clone, Copy)]
struct DollyZoomLens {
	start_fov: f32,
	end_fov: f32,
	start_rot: Quat,
	end_rot: Quat,
}

const PADDING: f32 = 500.0;

impl Lens<Transform> for DollyZoomLens {
	fn lerp(&mut self, target: &mut dyn Targetable<Transform>, ratio: f32) {
		let fov = self.start_fov + (self.end_fov - self.start_fov) * ratio;
		let distance = dolly_zoom_distance(WORLD_SIZE + PADDING, fov);
		// let distance = self.start_distance + (self.end_distance - self.start_distance) * ratio;
		let rot = self.start_rot.slerp(self.end_rot, ratio);
		let direction = rot * Vec3::Z;
		if let Some(transform) = target.as_any_mut().downcast_mut::<Transform>() {
			*transform = Transform {
				translation: direction * distance,
				rotation: rot,
				scale: Vec3::ONE,
			};
		}
	}
}

#[derive(Debug, Clone, Copy)]
struct ProjectionFovLens {
	start: f32,
	end: f32,
}

impl Lens<Projection> for ProjectionFovLens {
	fn lerp(&mut self, target: &mut dyn Targetable<Projection>, ratio: f32) {
		if let Some(projection) = target.as_any_mut().downcast_mut::<Projection>() {
			if let Projection::Perspective(persp) = projection {
				persp.fov = self.start + (self.end - self.start) * ratio;
			}
		}
	}
}

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
		.add_plugins(TweeningPlugin)
		.add_plugins(CameraDebugHud)
		.add_plugins(TerrainPlugin)
		.insert_resource(CameraMode::default())
		.add_systems(Startup, setup)
		.add_systems(Update, (toggle_camera, cleanup_completed_tweens))
		.add_systems(
			Update,
			bevy_tweening::component_animator_system::<Projection>,
		)
		.run();
}

fn setup(mut commands: Commands) {
	let (transform, perspective) = create_perspective_angled_state();

	commands.spawn((
		transform,
		Projection::from(perspective),
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

// Here's how the state transition works:
// Whenever the user presses the toggle key, the camera will transition to the next state.
// Perspective → Orthographic: Animate FOV to a very small value and move the camera to a top-down position in a single tween.
// Orthographic → Perspective: Animate FOV to the angled value and move the camera to the angled position in a single tween.
// There is no explicit orthographic projection anymore; everything is handled with perspective projection and FOV/transform tweening.

// Transition timing constants
const TOTAL_TRANSITION_TIME: f32 = 1.0;
const CLOSE_TO_ORTHOGRAPHIC_FOV: f32 = 1e-3;

fn toggle_camera(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut camera_mode: ResMut<CameraMode>,
	mut commands: Commands,
	camera_query: Query<(Entity, &Transform, &Projection), With<MainCamera>>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyT) && !camera_mode.is_transitioning {
		if let Ok((camera_entity, current_transform, current_projection)) = camera_query.single() {
			let new_mode = camera_mode.current_mode.next();
			camera_mode.is_transitioning = true;

			camera_mode.transition_timer =
				Timer::from_seconds(TOTAL_TRANSITION_TIME, TimerMode::Once);

			match (camera_mode.current_mode, new_mode) {
				// Perspective → Orthographic: 1-stage transition
				(CameraState::PerspectiveAngled, CameraState::OrthographicTopDown) => {
					let end_fov = CLOSE_TO_ORTHOGRAPHIC_FOV;
					let start_fov = if let Projection::Perspective(p) = current_projection {
						p.fov
					} else {
						panic!("Expected perspective projection");
					};
					let start_rot = current_transform.rotation;
					let end_rot = Quat::from_euler(
						EulerRot::XYZ,
						-90.0_f32.to_radians(),
						0.0_f32.to_radians(),
						90.0_f32.to_radians(),
					);
					let transform_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
						DollyZoomLens {
							start_fov,
							end_fov,
							start_rot,
							end_rot,
						},
					);
					let fov_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
						ProjectionFovLens {
							start: start_fov,
							end: end_fov,
						},
					);
					commands
						.entity(camera_entity)
						.insert(Animator::new(transform_tween))
						.insert(Animator::new(fov_tween));
				}
				// Orthographic → Perspective: 1-stage transition
				(CameraState::OrthographicTopDown, CameraState::PerspectiveAngled) => {
					let (_, angled_projection) = create_perspective_angled_state();
					let start_fov = if let Projection::Perspective(p) = current_projection {
						p.fov
					} else {
						panic!("Expected perspective projection");
					};
					let end_fov = angled_projection.fov;
					let start_rot = current_transform.rotation;
					let end_transform = create_perspective_angled_state().0;
					let end_rot = end_transform.rotation;
					let transform_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
						DollyZoomLens {
							start_fov,
							end_fov,
							start_rot,
							end_rot,
						},
					);
					let fov_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
						ProjectionFovLens {
							start: start_fov,
							end: end_fov,
						},
					);
					commands
						.entity(camera_entity)
						.insert(Animator::new(transform_tween))
						.insert(Animator::new(fov_tween));
				}
				_ => unreachable!(),
			}

			camera_mode.current_mode = new_mode;
		}
	}
}

fn cleanup_completed_tweens(
	time: Res<Time>,
	mut commands: Commands,
	mut camera_mode: ResMut<CameraMode>,
	camera_query: Query<Entity, With<MainCamera>>,
) {
	if camera_mode.is_transitioning {
		camera_mode.transition_timer.tick(time.delta());
		if camera_mode.transition_timer.finished() {
			if let Ok(camera_entity) = camera_query.single() {
				// Remove the animator components
				commands
					.entity(camera_entity)
					.remove::<Animator<Transform>>();
				commands
					.entity(camera_entity)
					.remove::<Animator<Projection>>();
				camera_mode.is_transitioning = false;
			}
		}
	}
}

fn create_perspective_angled_state() -> (Transform, PerspectiveProjection) {
	let fov = 60.0_f32.to_radians();
	// Desired camera position at 60deg FOV, looking from a diagonal angle
	let distance = dolly_zoom_distance(WORLD_SIZE + PADDING, fov);
	let initial_angle = Vec3::ONE;
	let angled_pos = initial_angle.normalize() * distance;
	let transform = Transform::from_translation(angled_pos).looking_at(Vec3::ZERO, Vec3::Y);
	let projection = create_perspective_projection(fov);
	(transform, projection)
}

fn dolly_zoom_distance(width: f32, fov: f32) -> f32 {
	width / (2.0 * (0.5 * fov).tan())
}

fn create_perspective_projection(fov: f32) -> PerspectiveProjection {
	PerspectiveProjection {
		fov,
		far: 10000.0,
		..default()
	}
}
