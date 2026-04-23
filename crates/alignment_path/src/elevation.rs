use serde::{Deserialize, Serialize};

use crate::geometry::{AlignmentGeometry, HeightSampler};

pub trait ElevationProfile {
	fn elevation_at(&self, station: f32) -> f32;
}

/// Samples terrain at every station by locating the horizontal XZ position along
/// the geometry and querying the height sampler. Produces "follows terrain" behavior.
pub struct TerrainSampledProfile<'a, H: HeightSampler> {
	pub sampler: &'a H,
	pub horizontal: &'a AlignmentGeometry,
}

impl<H: HeightSampler> ElevationProfile for TerrainSampledProfile<'_, H> {
	fn elevation_at(&self, station: f32) -> f32 {
		let Some(xz) = self.horizontal.xz_at_station(station) else {
			return 0.0;
		};
		let probe = glam::Vec3::new(xz.x, 0.0, xz.y);
		self.sampler.height_at(probe)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pvi {
	pub station: f32,
	pub elevation: f32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PviProfile {
	pub points: Vec<Pvi>,
}

impl ElevationProfile for PviProfile {
	fn elevation_at(&self, station: f32) -> f32 {
		let points = &self.points;
		if points.is_empty() {
			return 0.0;
		}
		if station <= points[0].station {
			return points[0].elevation;
		}
		let last = &points[points.len() - 1];
		if station >= last.station {
			return last.elevation;
		}
		// Piecewise-linear interpolation between adjacent PVIs.
		// Parabolic VCs will replace this in a future pass.
		let i = points
			.partition_point(|p| p.station <= station)
			.saturating_sub(1);
		let lo = &points[i];
		let hi = &points[i + 1];
		let span = hi.station - lo.station;
		if span <= 0.0 {
			return lo.elevation;
		}
		let t = (station - lo.station) / span;
		lo.elevation * (1.0 - t) + hi.elevation * t
	}
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum VerticalProfileData {
	TerrainSampled,
	Pvi(PviProfile),
}

impl Default for VerticalProfileData {
	fn default() -> Self {
		Self::TerrainSampled
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn pvi_profile_empty_returns_zero() {
		let profile = PviProfile { points: vec![] };
		assert_eq!(profile.elevation_at(100.0), 0.0);
	}

	#[test]
	fn pvi_profile_single_point() {
		let profile = PviProfile {
			points: vec![Pvi {
				station: 50.0,
				elevation: 10.0,
			}],
		};
		assert_eq!(profile.elevation_at(0.0), 10.0);
		assert_eq!(profile.elevation_at(50.0), 10.0);
		assert_eq!(profile.elevation_at(200.0), 10.0);
	}

	#[test]
	fn pvi_profile_interpolates_linearly() {
		let profile = PviProfile {
			points: vec![
				Pvi {
					station: 0.0,
					elevation: 0.0,
				},
				Pvi {
					station: 100.0,
					elevation: 10.0,
				},
			],
		};
		assert!((profile.elevation_at(50.0) - 5.0).abs() < 1e-4);
		assert!((profile.elevation_at(75.0) - 7.5).abs() < 1e-4);
	}

	#[test]
	fn pvi_profile_clamps_at_extents() {
		let profile = PviProfile {
			points: vec![
				Pvi {
					station: 10.0,
					elevation: 5.0,
				},
				Pvi {
					station: 90.0,
					elevation: 15.0,
				},
			],
		};
		assert_eq!(profile.elevation_at(0.0), 5.0);
		assert_eq!(profile.elevation_at(100.0), 15.0);
	}

	#[test]
	fn vertical_profile_data_default_is_terrain_sampled() {
		let data = VerticalProfileData::default();
		assert!(matches!(data, VerticalProfileData::TerrainSampled));
	}

	#[test]
	fn vertical_profile_data_roundtrips_pvi_json() {
		let original = VerticalProfileData::Pvi(PviProfile {
			points: vec![
				Pvi {
					station: 0.0,
					elevation: 5.0,
				},
				Pvi {
					station: 500.0,
					elevation: 20.0,
				},
			],
		});
		let json = serde_json::to_string(&original).unwrap();
		let restored: VerticalProfileData = serde_json::from_str(&json).unwrap();
		assert!(matches!(restored, VerticalProfileData::Pvi(_)));
	}

	#[test]
	fn vertical_profile_data_terrain_sampled_json() {
		let json = r#"{"kind":"terrain_sampled"}"#;
		let data: VerticalProfileData = serde_json::from_str(json).unwrap();
		assert!(matches!(data, VerticalProfileData::TerrainSampled));
	}

	#[test]
	fn alignment_without_vertical_profile_deserializes_to_default() {
		let json = r#"{"start":[0,0,0],"end":[100,0,0],"segments":[]}"#;
		let alignment: crate::path::Alignment = serde_json::from_str(json).unwrap();
		assert!(matches!(
			alignment.vertical_profile,
			VerticalProfileData::TerrainSampled
		));
	}

	struct ConstantSampler(f32);
	impl HeightSampler for ConstantSampler {
		fn height_at(&self, _position: glam::Vec3) -> f32 {
			self.0
		}
	}

	#[test]
	fn terrain_sampled_profile_returns_sampler_height() {
		use crate::geometry::{GeometrySegment, StraightGeometry};
		use glam::Vec3;

		let straight = StraightGeometry {
			start: Vec3::new(0.0, 0.0, 0.0),
			end: Vec3::new(100.0, 0.0, 0.0),
			start_station: 0.0,
			length: 100.0,
		};
		let geometry = AlignmentGeometry {
			segments: vec![GeometrySegment::Straight(straight)],
		};
		let sampler = ConstantSampler(42.0);
		let profile = TerrainSampledProfile {
			sampler: &sampler,
			horizontal: &geometry,
		};
		assert_eq!(profile.elevation_at(50.0), 42.0);
		assert_eq!(profile.elevation_at(0.0), 42.0);
		assert_eq!(profile.elevation_at(100.0), 42.0);
	}

	#[test]
	fn terrain_sampled_profile_out_of_range_returns_zero() {
		let geometry = AlignmentGeometry { segments: vec![] };
		let sampler = ConstantSampler(5.0);
		let profile = TerrainSampledProfile {
			sampler: &sampler,
			horizontal: &geometry,
		};
		assert_eq!(profile.elevation_at(9999.0), 0.0);
	}
}
