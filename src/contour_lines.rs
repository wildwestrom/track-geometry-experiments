use bevy::prelude::*;
use bevy_procedural_terrain_gen::{HeightMap, Settings};

/// A line segment with start and end points in 2D space
#[derive(Debug, Clone, Copy)]
pub struct LineSegment {
	pub start: Vec2,
	pub end: Vec2,
}

/// Contour line data for a single elevation level
#[derive(Debug, Clone)]
pub struct ContourLevel {
	pub elevation: f32,
	pub segments: Vec<LineSegment>,
}

/// Complete set of contour lines at multiple elevations
#[derive(Debug, Clone, Resource)]
pub struct ContourLines {
	pub levels: Vec<ContourLevel>,
}

impl ContourLines {
	pub fn new() -> Self {
		Self { levels: Vec::new() }
	}

	/// Get total vertex count across all levels (2 per segment)
	pub fn vertex_count(&self) -> usize {
		self
			.levels
			.iter()
			.map(|level| level.segments.len() * 2)
			.sum()
	}

	/// Flatten all segments into a single vector of vertices for GPU rendering
	pub fn to_vertices(&self) -> Vec<Vec2> {
		let mut vertices = Vec::with_capacity(self.vertex_count());
		for level in &self.levels {
			for segment in &level.segments {
				vertices.push(segment.start);
				vertices.push(segment.end);
			}
		}
		vertices
	}

	/// Convert all segments to GPU-compatible format
	/// Returns a vector of segments with their start and end points in 2D (XZ plane)
	pub fn to_gpu_segments(&self) -> Vec<crate::terrain_contour::LineSegmentGPU> {
		use crate::terrain_contour::LineSegmentGPU;
		let mut gpu_segments = Vec::new();
		for level in &self.levels {
			for segment in &level.segments {
				gpu_segments.push(LineSegmentGPU {
					start: segment.start,
					end: segment.end,
				});
			}
		}
		gpu_segments
	}
}

impl Default for ContourLines {
	fn default() -> Self {
		Self::new()
	}
}

/// Generate contour lines from a height map using marching squares algorithm
///
/// # Arguments
/// * `height_map` - The terrain height map (values are normalized 0.0-1.0)
/// * `settings` - Terrain settings for coordinate conversion
/// * `min_elevation` - Minimum world elevation (Y coordinate) to generate contours for
/// * `max_elevation` - Maximum world elevation (Y coordinate) to generate contours for
/// * `interval` - Spacing between contour lines in world units
pub fn generate_contour_lines(
	height_map: &HeightMap,
	settings: &Settings,
	min_elevation: f32,
	max_elevation: f32,
	interval: f32,
) -> ContourLines {
	let grid_x = settings.grid_x();
	let grid_z = settings.grid_z();

	// Calculate world scale for coordinate conversion
	let world_x = settings.world_x();
	let world_z = settings.world_z();
	let height_scale = settings.world_x().min(settings.world_z()) * settings.height_multiplier;

	// Generate elevation levels in world space
	let mut elevation_levels = Vec::new();
	let mut current = min_elevation;
	while current <= max_elevation {
		elevation_levels.push(current);
		current += interval;
	}

	let mut contour_lines = ContourLines::new();

	// Generate contours for each elevation level
	for &elevation in &elevation_levels {
		// Convert world elevation to normalized height map space (0.0-1.0)
		let iso_value = elevation / height_scale;

		// Clamp iso_value to valid range
		let iso_value = iso_value.clamp(0.0, 1.0);

		let mut segments = Vec::new();

		// Process each cell in the grid
		// Note: We iterate up to grid_x - 1 and grid_z - 1 to avoid out-of-bounds
		for z in 0..grid_z {
			for x in 0..grid_x {
				// Get height values at the four corners of the cell (normalized 0.0-1.0)
				let height_bl = height_map.get(x, z); // Bottom-left
				let height_br = height_map.get(x + 1, z); // Bottom-right
				let height_tl = height_map.get(x, z + 1); // Top-left
				let height_tr = height_map.get(x + 1, z + 1); // Top-right

				// Determine which corners are above the iso level
				let above_bl = height_bl >= iso_value;
				let above_br = height_br >= iso_value;
				let above_tl = height_tl >= iso_value;
				let above_tr = height_tr >= iso_value;

				// Get the marching squares case (0-15)
				let contour_case = get_contour_case(above_tl, above_tr, above_bl, above_br);

				// Generate line segments for this cell
				get_contour_line_segments(
					contour_case,
					x,
					z,
					iso_value,
					height_bl,
					height_br,
					height_tl,
					height_tr,
					world_x,
					world_z,
					grid_x,
					grid_z,
					&mut segments,
				);
			}
		}

		contour_lines.levels.push(ContourLevel {
			elevation,
			segments,
		});
	}

	contour_lines
}

/// Get the marching squares case number (0-15) based on which corners are above iso level
/// Case encoding: BL (bit 0) + BR (bit 1) + TR (bit 2) + TL (bit 3)
fn get_contour_case(tl: bool, tr: bool, bl: bool, br: bool) -> u8 {
	let mut result = 0u8;
	if bl {
		result |= 1;
	}
	if br {
		result |= 2;
	}
	if tr {
		result |= 4;
	}
	if tl {
		result |= 8;
	}
	result
}

/// Generate line segments for a cell based on the marching squares case
fn get_contour_line_segments(
	case: u8,
	grid_x: u32,
	grid_z: u32,
	iso_value: f32,
	height_bl: f32,
	height_br: f32,
	height_tl: f32,
	height_tr: f32,
	world_x: f32,
	world_z: f32,
	grid_x_size: u32,
	grid_z_size: u32,
	segments: &mut Vec<LineSegment>,
) {
	// Convert grid coordinates to world coordinates
	// grid_to_world formula: x_pos = (grid_x as f32).mul_add(x_step, -(world_x / 2.0))
	let x_step = world_x / grid_x_size as f32;
	let z_step = world_z / grid_z_size as f32;
	let x_base = (grid_x as f32).mul_add(x_step, -(world_x / 2.0));
	let z_base = (grid_z as f32).mul_add(z_step, -(world_z / 2.0));

	match case {
		// No intersection
		0 | 15 => return,

		// Single edge cases
		1 | 14 => {
			// Bottom-left corner: line from left edge to bottom edge
			// Left edge: interpolate between BL (at grid_z) and TL (at grid_z+1)
			let left_z = lerp(
				height_bl,
				height_tl,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			// Bottom edge: interpolate between BL (at grid_x) and BR (at grid_x+1)
			let bottom_x = lerp(
				height_bl,
				height_br,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			// Convert to world coordinates
			let left_world_z = (left_z as f32).mul_add(z_step, -(world_z / 2.0));
			let bottom_world_x = (bottom_x as f32).mul_add(x_step, -(world_x / 2.0));
			let start = Vec2::new(x_base, left_world_z);
			let end = Vec2::new(bottom_world_x, z_base);
			segments.push(LineSegment { start, end });
		}

		2 | 13 => {
			// Bottom-right corner: line from bottom edge to right edge
			// Bottom edge: interpolate between BL (at grid_x) and BR (at grid_x+1)
			let bottom_x = lerp(
				height_bl,
				height_br,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			// Right edge: interpolate between BR (at grid_z) and TR (at grid_z+1)
			let right_z = lerp(
				height_br,
				height_tr,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			// Convert to world coordinates
			let bottom_world_x = (bottom_x as f32).mul_add(x_step, -(world_x / 2.0));
			let right_world_z = (right_z as f32).mul_add(z_step, -(world_z / 2.0));
			let start = Vec2::new(bottom_world_x, z_base);
			let end = Vec2::new(x_base + x_step, right_world_z);
			segments.push(LineSegment { start, end });
		}

		4 | 11 => {
			// Top-right corner: line from top edge to right edge
			// Top edge: interpolate between TL (at grid_x) and TR (at grid_x+1)
			let top_x = lerp(
				height_tl,
				height_tr,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			// Right edge: interpolate between BR (at grid_z) and TR (at grid_z+1)
			let right_z = lerp(
				height_br,
				height_tr,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			// Convert to world coordinates
			let top_world_x = (top_x as f32).mul_add(x_step, -(world_x / 2.0));
			let right_world_z = (right_z as f32).mul_add(z_step, -(world_z / 2.0));
			let start = Vec2::new(top_world_x, z_base + z_step);
			let end = Vec2::new(x_base + x_step, right_world_z);
			segments.push(LineSegment { start, end });
		}

		7 | 8 => {
			// Top-left corner: line from left edge to top edge
			// Left edge: interpolate between BL (at grid_z) and TL (at grid_z+1)
			let left_z = lerp(
				height_bl,
				height_tl,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			// Top edge: interpolate between TL (at grid_x) and TR (at grid_x+1)
			let top_x = lerp(
				height_tl,
				height_tr,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			// Convert to world coordinates
			let left_world_z = (left_z as f32).mul_add(z_step, -(world_z / 2.0));
			let top_world_x = (top_x as f32).mul_add(x_step, -(world_x / 2.0));
			let start = Vec2::new(x_base, left_world_z);
			let end = Vec2::new(top_world_x, z_base + z_step);
			segments.push(LineSegment { start, end });
		}

		// Horizontal edge cases
		3 | 12 => {
			// Horizontal line across the cell (left to right edge)
			let left_z = lerp(
				height_bl,
				height_tl,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			let right_z = lerp(
				height_br,
				height_tr,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			// Convert to world coordinates
			let left_world_z = (left_z as f32).mul_add(z_step, -(world_z / 2.0));
			let right_world_z = (right_z as f32).mul_add(z_step, -(world_z / 2.0));
			let start = Vec2::new(x_base, left_world_z);
			let end = Vec2::new(x_base + x_step, right_world_z);
			segments.push(LineSegment { start, end });
		}

		// Vertical edge cases
		6 | 9 => {
			// Vertical line across the cell (bottom to top edge)
			let bottom_x = lerp(
				height_bl,
				height_br,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			let top_x = lerp(
				height_tl,
				height_tr,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			// Convert to world coordinates
			let bottom_world_x = (bottom_x as f32).mul_add(x_step, -(world_x / 2.0));
			let top_world_x = (top_x as f32).mul_add(x_step, -(world_x / 2.0));
			let start = Vec2::new(bottom_world_x, z_base);
			let end = Vec2::new(top_world_x, z_base + z_step);
			segments.push(LineSegment { start, end });
		}

		// Saddle point cases (ambiguous)
		5 => {
			// Two possible lines - use average method (connect opposite corners)
			// First line: from top edge to right edge
			let top_x = lerp(
				height_tl,
				height_tr,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			let right_z = lerp(
				height_br,
				height_tr,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			let top_world_x = (top_x as f32).mul_add(x_step, -(world_x / 2.0));
			let right_world_z = (right_z as f32).mul_add(z_step, -(world_z / 2.0));
			let start = Vec2::new(top_world_x, z_base + z_step);
			let end = Vec2::new(x_base + x_step, right_world_z);
			segments.push(LineSegment { start, end });

			// Second line: from left edge to bottom edge
			let left_z = lerp(
				height_bl,
				height_tl,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			let bottom_x = lerp(
				height_bl,
				height_br,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			let left_world_z = (left_z as f32).mul_add(z_step, -(world_z / 2.0));
			let bottom_world_x = (bottom_x as f32).mul_add(x_step, -(world_x / 2.0));
			let start = Vec2::new(x_base, left_world_z);
			let end = Vec2::new(bottom_world_x, z_base);
			segments.push(LineSegment { start, end });
		}

		10 => {
			// Two possible lines - use average method (connect opposite corners)
			// First line: from left edge to top edge
			let left_z = lerp(
				height_bl,
				height_tl,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			let top_x = lerp(
				height_tl,
				height_tr,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			let left_world_z = (left_z as f32).mul_add(z_step, -(world_z / 2.0));
			let top_world_x = (top_x as f32).mul_add(x_step, -(world_x / 2.0));
			let start = Vec2::new(x_base, left_world_z);
			let end = Vec2::new(top_world_x, z_base + z_step);
			segments.push(LineSegment { start, end });

			// Second line: from bottom edge to right edge
			let bottom_x = lerp(
				height_bl,
				height_br,
				iso_value,
				grid_x as f32,
				(grid_x + 1) as f32,
			);
			let right_z = lerp(
				height_br,
				height_tr,
				iso_value,
				grid_z as f32,
				(grid_z + 1) as f32,
			);
			let bottom_world_x = (bottom_x as f32).mul_add(x_step, -(world_x / 2.0));
			let right_world_z = (right_z as f32).mul_add(z_step, -(world_z / 2.0));
			let start = Vec2::new(bottom_world_x, z_base);
			let end = Vec2::new(x_base + x_step, right_world_z);
			segments.push(LineSegment { start, end });
		}

		_ => {
			// Unhandled case - shouldn't happen but handle gracefully
			panic!("Unhandled marching squares case: {}", case);
		}
	}
}

/// Linear interpolation to find the exact point where iso_value crosses between two heights
/// Based on C++ implementation: lerp(v1, v2, iso, a1, a2)
/// Returns the coordinate value where iso crosses between heights v1 and v2
/// a1 and a2 are the coordinates at heights v1 and v2 respectively
fn lerp(v1: f32, v2: f32, iso: f32, a1: f32, a2: f32) -> f32 {
	if (v1 - v2).abs() < f32::EPSILON {
		// Edge case: heights are equal, return midpoint
		(a1 + a2) * 0.5
	} else {
		(iso - v2) * (a1 - a2) / (v1 - v2) + a2
	}
}
