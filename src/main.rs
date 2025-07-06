use bevy::{
	prelude::*,
	render::{
		RenderPlugin,
		camera::ScalingMode,
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

#[derive(Resource, Default)]
struct CameraMode {
	current_mode: CameraState,
	is_transitioning: bool,
	transition_timer: Timer,
}

#[derive(Clone, Copy, PartialEq)]
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

#[derive(Component)]
struct DelayedProjectionChange {
	timer: Timer,
	camera_entity: Entity,
	target_projection: Projection,
}

#[derive(Debug, Clone, Copy)]
struct TransformLerpLens {
	start: Transform,
	end: Transform,
}

impl Lens<Transform> for TransformLerpLens {
	fn lerp(&mut self, target: &mut dyn Targetable<Transform>, ratio: f32) {
		if let Some(transform) = target.as_any_mut().downcast_mut::<Transform>() {
			transform.translation = self.start.translation.lerp(self.end.translation, ratio);
			transform.rotation = self.start.rotation.slerp(self.end.rotation, ratio);
			transform.scale = self.start.scale.lerp(self.end.scale, ratio);
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
		.add_systems(
			Update,
			(
				toggle_camera,
				cleanup_completed_tweens,
				handle_projection_changes,
			),
		)
		.run();
}

fn setup(mut commands: Commands) {
	let (transform, projection) = create_perspective_camera_state();

	// Spawn single main camera
	commands.spawn((
		transform,
		projection,
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

fn toggle_camera(
	keyboard_input: Res<ButtonInput<KeyCode>>,
	mut camera_mode: ResMut<CameraMode>,
	mut commands: Commands,
	camera_query: Query<(Entity, &Transform, &Projection), With<MainCamera>>,
) {
	if keyboard_input.just_pressed(KeyCode::KeyT) && !camera_mode.is_transitioning {
		if let Ok((camera_entity, current_transform, _current_projection)) = camera_query.single() {
			let new_mode = camera_mode.current_mode.next();
			camera_mode.is_transitioning = true;
			camera_mode.transition_timer = Timer::from_seconds(1.0, TimerMode::Once);

			// Get target camera state
			let (target_transform, target_projection) = get_camera_state(new_mode);

			match (camera_mode.current_mode, new_mode) {
				// Perspective → Orthographic: Transform first, then change projection
				(CameraState::PerspectiveAngled, CameraState::OrthographicTopDown) => {
					// Create tween for transform transition
					let transform_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(0.8),
						TransformLerpLens {
							start: *current_transform,
							end: target_transform,
						},
					);

					// Add tween component to camera
					commands
						.entity(camera_entity)
						.insert(Animator::new(transform_tween));

					// Schedule projection change after transform completes
					commands.spawn(DelayedProjectionChange {
						timer: Timer::from_seconds(0.8, TimerMode::Once),
						camera_entity,
						target_projection,
					});
				}
				// Orthographic → Perspective: Change projection first, then transform
				(CameraState::OrthographicTopDown, CameraState::PerspectiveAngled) => {
					// Change projection immediately using commands
					commands.entity(camera_entity).insert(target_projection);

					// Create tween for transform transition
					let transform_tween = Tween::new(
						EaseFunction::SmoothStep,
						std::time::Duration::from_secs_f32(0.8),
						TransformLerpLens {
							start: *current_transform,
							end: target_transform,
						},
					);

					// Add tween component to camera
					commands
						.entity(camera_entity)
						.insert(Animator::new(transform_tween));
				}
				_ => unreachable!(),
			}

			// Update camera mode
			camera_mode.current_mode = new_mode;
		}
	}
}

fn cleanup_completed_tweens(
	time: Res<Time>,
	mut commands: Commands,
	mut camera_mode: ResMut<CameraMode>,
	camera_query: Query<(Entity, &mut Animator<Transform>), With<MainCamera>>,
) {
	if camera_mode.is_transitioning {
		camera_mode.transition_timer.tick(time.delta());
		if camera_mode.transition_timer.finished() {
			if let Ok((camera_entity, _animator)) = camera_query.single() {
				// Remove the animator component
				commands
					.entity(camera_entity)
					.remove::<Animator<Transform>>();
				camera_mode.is_transitioning = false;
			}
		}
	}
}

fn handle_projection_changes(
	time: Res<Time>,
	mut commands: Commands,
	mut delayed_changes: Query<(Entity, &mut DelayedProjectionChange)>,
) {
	for (entity, mut delayed_change) in delayed_changes.iter_mut() {
		delayed_change.timer.tick(time.delta());
		if delayed_change.timer.finished() {
			// Apply the projection change using commands
			commands
				.entity(delayed_change.camera_entity)
				.insert(delayed_change.target_projection.clone());
			// Remove the delayed change component
			commands.entity(entity).despawn();
		}
	}
}

fn get_camera_state(state: CameraState) -> (Transform, Projection) {
	match state {
		CameraState::PerspectiveAngled => create_perspective_camera_state(),
		CameraState::OrthographicTopDown => {
			let transform = Transform::from_translation(Vec3::new(0.0, 1200.0, 0.0))
				.looking_at(Vec3::ZERO, Vec3::Y);
			let projection = Projection::from(OrthographicProjection {
				scale: 1.0,
				near: 0.1,
				far: 10000.0,
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

fn create_perspective_camera_state() -> (Transform, Projection) {
	let transform =
		Transform::from_translation(Vec3::new(1000.0, 1000.0, 1200.0)).looking_at(Vec3::ZERO, Vec3::Y);
	let projection = Projection::from(PerspectiveProjection {
		fov: PI / 4.0,
		far: 10000.0,
		..default()
	});
	(transform, projection)
}
