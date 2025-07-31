use bevy::prelude::*;
use bevy::render::view::RenderLayers;
use bevy_panorbit_camera::{PanOrbitCamera, PanOrbitCameraPlugin};
use bevy_tweening::{Animator, Lens, Targetable, Tween, TweeningPlugin};
use std::f32::consts::PI;

use crate::spatial::world_size;

pub struct CameraPlugin;

impl Plugin for CameraPlugin {
	fn build(&self, app: &mut App) {
		app
			.add_plugins(PanOrbitCameraPlugin)
			.add_plugins(TweeningPlugin)
			// .add_plugins(crate::hud::CameraDebugHud)
			.insert_resource(CameraMode::default())
			.add_systems(Startup, setup)
			.add_systems(
				Update,
				(
					toggle_camera,
					cleanup_completed_tweens,
					disable_camera_during_transition,
					bevy_tweening::component_animator_system::<Projection>,
					bevy_tweening::component_animator_system::<PanOrbitCamera>,
				),
			);
	}
}

fn setup(mut commands: Commands, settings: Res<crate::terrain::Settings>) {
	let world_size = world_size(&settings);
	let (transform, perspective) = create_perspective_angled_state(world_size + 4206.9); // Just a random value to test its smooth

	commands.spawn((
		transform,
		Projection::from(perspective),
		//Camera3d::default(),
		Camera {
			order: 0,
			..default()
		},
		RenderLayers::layer(0),
		PanOrbitCamera::default(),
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
		RenderLayers::layer(1),
	));
}

#[derive(Resource, Clone)]
pub struct CameraMode {
	current_mode: CameraState,
	pub is_transitioning: bool,
	transition_timer: Timer,
	pub user_enabled: bool,
}

impl CameraMode {
	/// Disable camera movement (typically during drag operations)
	pub const fn disable_camera_movement(&mut self) {
		self.user_enabled = false;
	}

	/// Enable camera movement
	pub const fn enable_camera_movement(&mut self) {
		self.user_enabled = true;
	}

	/// Check if camera is in transition
	pub const fn is_camera_transitioning(&self) -> bool {
		self.is_transitioning
	}

	/// Check if camera movement is enabled
	pub const fn is_camera_movement_enabled(&self) -> bool {
		self.user_enabled
	}
}

impl Default for CameraMode {
	fn default() -> Self {
		Self {
			current_mode: CameraState::Perspective,
			is_transitioning: false,
			transition_timer: Timer::from_seconds(0.0, TimerMode::Repeating),
			user_enabled: true,
		}
	}
}

#[derive(PartialEq, Debug, Clone, Copy, Default)]
enum CameraState {
	#[default]
	Perspective,
	Orthographic,
}

impl CameraState {
	const fn next(self) -> Self {
		match self {
			Self::Perspective => Self::Orthographic,
			Self::Orthographic => Self::Perspective,
		}
	}
}

#[derive(Debug)]
struct DollyZoomLens {
	start_fov: f32,
	end_fov: f32,
	start_rot: Quat,
	end_rot: Quat,
	start_size: f32,
	end_size: f32,
}

const PADDING: f32 = 500.0;

impl Lens<Transform> for DollyZoomLens {
	fn lerp(&mut self, target: &mut dyn Targetable<Transform>, ratio: f32) {
		let fov = (self.end_fov - self.start_fov).mul_add(ratio, self.start_fov);
		let size = (self.end_size - self.start_size).mul_add(ratio, self.start_size);
		let distance = dolly_zoom_distance(size, fov);
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

#[derive(Debug)]
struct PanOrbitCameraLens {
	start_fov: f32,
	end_fov: f32,
	start_rot: Quat,
	end_rot: Quat,
	start_size: f32,
	end_size: f32,
}

impl Lens<PanOrbitCamera> for PanOrbitCameraLens {
	fn lerp(&mut self, target: &mut dyn Targetable<PanOrbitCamera>, ratio: f32) {
		let fov = (self.end_fov - self.start_fov).mul_add(ratio, self.start_fov);
		let size = (self.end_size - self.start_size).mul_add(ratio, self.start_size);
		let distance = dolly_zoom_distance(size, fov);

		// Calculate start and end spherical coordinates directly from rotations
		let start_direction = self.start_rot * Vec3::Z;
		let end_direction = self.end_rot * Vec3::Z;

		let (start_yaw, start_pitch) = direction_to_spherical(start_direction);
		let (end_yaw, end_pitch) = direction_to_spherical(end_direction);

		// Interpolate spherical coordinates directly
		let yaw = (end_yaw - start_yaw).mul_add(ratio, start_yaw);
		let pitch = (end_pitch - start_pitch).mul_add(ratio, start_pitch);
		let radius = distance;

		if let Some(pan_orbit_camera) = target.as_any_mut().downcast_mut::<PanOrbitCamera>() {
			pan_orbit_camera.yaw = Some(yaw);
			pan_orbit_camera.pitch = Some(pitch);
			pan_orbit_camera.radius = Some(radius);
			pan_orbit_camera.focus = Vec3::ZERO;
			pan_orbit_camera.target_yaw = yaw;
			pan_orbit_camera.target_pitch = pitch;
			pan_orbit_camera.target_radius = radius;
			pan_orbit_camera.target_focus = Vec3::ZERO;
		}
	}
}

// Helper function to convert direction vector to spherical coordinates
fn direction_to_spherical(direction: Vec3) -> (f32, f32) {
	let yaw = direction.x.atan2(direction.z);
	let pitch = (direction.y / direction.length()).asin();
	(yaw, pitch)
}

#[derive(Debug)]
struct ProjectionFovLens {
	start: f32,
	end: f32,
}

impl Lens<Projection> for ProjectionFovLens {
	fn lerp(&mut self, target: &mut dyn Targetable<Projection>, ratio: f32) {
		if let Some(projection) = target.as_any_mut().downcast_mut::<Projection>()
			&& let Projection::Perspective(persp) = projection
		{
			persp.fov = (self.end - self.start).mul_add(ratio, self.start);
		}
	}
}

// Here's how the state transition works:
// Whenever the user presses the toggle key, the camera will transition to the next state.
// Perspective → Orthographic: Animate FOV to a very small value and move the camera to a top-down
// position in a single tween. Orthographic → Perspective: Animate FOV to the angled value and move
// the camera to the angled position in a single tween. There is no explicit orthographic projection
// anymore; everything is handled with perspective projection and FOV/transform tweening.

// Transition timing constants
const TOTAL_TRANSITION_TIME: f32 = 1.0;
const CLOSE_TO_ORTHOGRAPHIC_FOV: f32 = 1e-3;

fn toggle_camera(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut camera_mode: ResMut<CameraMode>,
	mut commands: Commands,
	camera_query: Query<(Entity, &Transform, &Projection, &PanOrbitCamera)>,
	settings: Res<crate::terrain::Settings>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyT)
		&& !camera_mode.is_transitioning
		&& let Ok((camera_entity, current_transform, current_projection, _)) = camera_query.single()
	{
		let new_mode = camera_mode.current_mode.next();
		camera_mode.is_transitioning = true;

		camera_mode.transition_timer = Timer::from_seconds(TOTAL_TRANSITION_TIME, TimerMode::Once);

		let world_size = world_size(&settings);

		match (camera_mode.current_mode, new_mode) {
			// Perspective → Orthographic: 1-stage transition
			(CameraState::Perspective, CameraState::Orthographic) => {
				let end_size = world_size + PADDING; // TODO: Use settings.terrain.world_length().max(settings.terrain.world_width()) instead of hardcoded value
				let end_fov = CLOSE_TO_ORTHOGRAPHIC_FOV;
				let start_fov = if let Projection::Perspective(p) = current_projection {
					p.fov
				} else {
					panic!("Expected perspective projection");
				};
				let start_rot = current_transform.rotation;
				let end_rot = Quat::from_axis_angle(Vec3::Y, 90.0_f32.to_radians())
					* Quat::from_axis_angle(Vec3::X, -89.9_f32.to_radians());
				// Calculate current camera's effective size from its position and FOV
				let current_distance = current_transform.translation.length();
				let current_size = dolly_zoom_width(current_distance, start_fov);
				let transform_tween = Tween::new(
					EaseFunction::SmoothStep,
					std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
					DollyZoomLens {
						start_fov,
						end_fov,
						start_rot,
						end_rot,
						start_size: current_size,
						end_size,
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
				let pan_orbit_tween = Tween::new(
					EaseFunction::SmoothStep,
					std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
					PanOrbitCameraLens {
						start_fov,
						end_fov,
						start_rot,
						end_rot,
						start_size: current_size,
						end_size,
					},
				);
				commands
					.entity(camera_entity)
					.insert(Animator::new(transform_tween))
					.insert(Animator::new(fov_tween))
					.insert(Animator::new(pan_orbit_tween));
			}
			// Orthographic → Perspective: 1-stage transition
			(CameraState::Orthographic, CameraState::Perspective) => {
				let end_size = world_size + PADDING; // TODO: Use settings.terrain.world_length().max(settings.terrain.world_width()) instead of hardcoded value
				let (angled_transform, angled_projection) = create_perspective_angled_state(end_size);
				let start_fov = if let Projection::Perspective(p) = current_projection {
					p.fov
				} else {
					panic!("Expected perspective projection");
				};
				let end_fov = angled_projection.fov;
				let start_rot = current_transform.rotation;
				let end_transform = angled_transform;
				let end_rot = end_transform.rotation;
				// Calculate current camera's effective size from its position and FOV
				let current_distance = current_transform.translation.length();
				let current_size = dolly_zoom_width(current_distance, start_fov);
				let transform_tween = Tween::new(
					EaseFunction::SmoothStep,
					std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
					DollyZoomLens {
						start_fov,
						end_fov,
						start_rot,
						end_rot,
						start_size: current_size,
						end_size,
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
				let pan_orbit_tween = Tween::new(
					EaseFunction::SmoothStep,
					std::time::Duration::from_secs_f32(TOTAL_TRANSITION_TIME),
					PanOrbitCameraLens {
						start_fov,
						end_fov,
						start_rot,
						end_rot,
						start_size: current_size,
						end_size,
					},
				);
				commands
					.entity(camera_entity)
					.insert(Animator::new(transform_tween))
					.insert(Animator::new(fov_tween))
					.insert(Animator::new(pan_orbit_tween));
			}
			_ => unreachable!(),
		}

		camera_mode.current_mode = new_mode;
	}
}

fn cleanup_completed_tweens(
	time: Res<Time>,
	mut commands: Commands,
	mut camera_mode: ResMut<CameraMode>,
	camera_query: Query<Entity, With<PanOrbitCamera>>,
) {
	if camera_mode.is_transitioning {
		camera_mode.transition_timer.tick(time.delta());
		if camera_mode.transition_timer.finished()
			&& let Ok(camera_entity) = camera_query.single()
		{
			// Remove the animator components
			commands
				.entity(camera_entity)
				.remove::<Animator<Transform>>();
			commands
				.entity(camera_entity)
				.remove::<Animator<Projection>>();
			commands
				.entity(camera_entity)
				.remove::<Animator<PanOrbitCamera>>();
			camera_mode.is_transitioning = false;
		}
	}
}

fn disable_camera_during_transition(
	camera_mode: Res<CameraMode>,
	mut camera_query: Query<&mut PanOrbitCamera>,
) {
	if let Ok(mut camera) = camera_query.single_mut() {
		if camera_mode.is_camera_transitioning() {
			camera.enabled = false;
		} else {
			camera.enabled = camera_mode.is_camera_movement_enabled();
		}
	}
}

fn create_perspective_angled_state(size: f32) -> (Transform, PerspectiveProjection) {
	let fov = 60.0_f32.to_radians();
	// Desired camera position at 60deg FOV, looking from a diagonal angle
	let distance = dolly_zoom_distance(size, fov);
	let initial_angle = Vec3::new(0., 1., 1.);
	let angled_pos = initial_angle.normalize() * distance;
	let transform = Transform::from_translation(angled_pos).looking_at(Vec3::ZERO, Vec3::Y);
	let projection = create_perspective_projection(fov);
	(transform, projection)
}

fn dolly_zoom_distance(width: f32, fov: f32) -> f32 {
	width / (2.0 * (0.5 * fov).tan())
}

fn dolly_zoom_width(distance: f32, fov: f32) -> f32 {
	distance * 2.0 * (0.5 * fov).tan()
}

fn create_perspective_projection(fov: f32) -> PerspectiveProjection {
	PerspectiveProjection {
		fov,
		far: 10000.0,
		..default()
	}
}
