use super::Settings;
use bevy::prelude::*;

/// Convert grid coordinates to world coordinates
pub fn grid_to_world(grid_x: u32, grid_z: u32, settings: &Settings) -> Vec3 {
	let world_x = settings.world_x();
	let world_z = settings.world_z();
	let grid_x_count = settings.grid_x();
	let grid_z_count = settings.grid_z();

	let x_step = world_x / grid_x_count as f32;
	let z_step = world_z / grid_z_count as f32;

	let x_pos = (grid_x as f32).mul_add(x_step, -(world_x / 2.0));
	let z_pos = (grid_z as f32).mul_add(z_step, -(world_z / 2.0));

	Vec3::new(x_pos, 0.0, z_pos)
}

/// Calculate terrain height at given world coordinates using bilinear interpolation.
pub fn calculate_terrain_height(
	world_pos: Vec3,
	heightmap: &super::HeightMap,
	settings: &Settings,
) -> f32 {
	let world_x = settings.world_x();
	let world_z = settings.world_z();
	let grid_x = settings.grid_x();
	let grid_z = settings.grid_z();

	let gx_f = (world_pos.x + world_x / 2.0) / world_x * grid_x as f32;
	let gz_f = (world_pos.z + world_z / 2.0) / world_z * grid_z as f32;

	let x0 = (gx_f.floor() as u32).min(grid_x.saturating_sub(1));
	let z0 = (gz_f.floor() as u32).min(grid_z.saturating_sub(1));
	let x1 = (x0 + 1).min(grid_x.saturating_sub(1));
	let z1 = (z0 + 1).min(grid_z.saturating_sub(1));

	let tx = gx_f.fract();
	let tz = gz_f.fract();

	let h00 = heightmap.get(x0, z0);
	let h10 = heightmap.get(x1, z0);
	let h01 = heightmap.get(x0, z1);
	let h11 = heightmap.get(x1, z1);

	let base_height = (h00 * (1.0 - tx) + h10 * tx) * (1.0 - tz) + (h01 * (1.0 - tx) + h11 * tx) * tz;

	base_height * world_size_for_height(settings) * settings.height_multiplier
}

/// Get the world size (commonly used calculation)
pub fn world_size(settings: &Settings) -> f32 {
	settings.world_x().max(settings.world_z())
}

/// Get the world size for height calculations
pub fn world_size_for_height(settings: &Settings) -> f32 {
	settings.world_x().min(settings.world_z())
}

/*
/// Clamp world position to terrain bounds
pub fn clamp_to_terrain_bounds(world_pos: Vec3, settings: &Settings) -> Vec3 {
	let half_world_x = settings.world_x() / 2.0;
	let half_world_z = settings.world_z() / 2.0;

	Vec3::new(
		world_pos.x.clamp(-half_world_x, half_world_x),
		world_pos.y,
		world_pos.z.clamp(-half_world_z, half_world_z),
	)
}
*/
