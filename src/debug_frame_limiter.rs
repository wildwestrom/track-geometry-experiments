use std::thread;
use std::time::{Duration, Instant};

use bevy::prelude::*;

pub(crate) struct FrameLimiterPlugin;

impl Plugin for FrameLimiterPlugin {
	fn build(&self, app: &mut App) {
		app
			.init_resource::<FrameLimiterState>()
			.add_systems(Last, enforce_frame_limit);
	}
}

#[derive(Resource, Debug, Clone)]
pub(crate) struct FrameLimiterState {
	pub enabled: bool,
	pub target_fps: u32,
	last_frame_end: Option<Instant>,
}

impl Default for FrameLimiterState {
	fn default() -> Self {
		Self {
			enabled: false,
			target_fps: 60,
			last_frame_end: None,
		}
	}
}

fn enforce_frame_limit(mut limiter: ResMut<FrameLimiterState>) {
	if !limiter.enabled {
		limiter.last_frame_end = None;
		return;
	}

	let fps = limiter.target_fps.max(1);
	let target_frame_time = Duration::from_secs_f64(1.0 / f64::from(fps));
	let now = Instant::now();

	if let Some(previous_frame_end) = limiter.last_frame_end {
		let elapsed = now.saturating_duration_since(previous_frame_end);
		if elapsed < target_frame_time {
			thread::sleep(target_frame_time - elapsed);
		}
	}

	limiter.last_frame_end = Some(Instant::now());
}
