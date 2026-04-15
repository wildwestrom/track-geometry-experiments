use anyhow::Result;
use bevy_egui::egui;
use log::{debug, error};
use serde::{Deserialize, Serialize};

/// Trait for structs that can be saved to and loaded from JSON files
/// with consistent error handling and UI integration
pub trait SaveableSettings: Serialize + for<'de> Deserialize<'de> + Default {
	/// The filename where this struct should be saved/loaded from
	fn filename() -> &'static str;

	/// Save the struct to its JSON file
	fn save(&self) -> Result<()> {
		#[cfg(not(target_arch = "wasm32"))]
		{
			let json = serde_json::to_string_pretty(self)?;
			std::fs::write(Self::filename(), json)?;
		}
		Ok(())
	}

	/// Load the struct from its JSON file, returning an error if it fails
	fn load() -> Result<Self> {
		#[cfg(not(target_arch = "wasm32"))]
		{
			let filename = Self::filename();
			if std::path::Path::new(filename).exists() {
				let json = std::fs::read_to_string(filename)?;
				return Ok(serde_json::from_str(&json)?);
			}
		}
		Ok(Self::default())
	}

	/// Load the struct from its JSON file with error handling and logging
	/// Returns default values if loading fails
	fn load_or_default() -> Self {
		match Self::load() {
			Ok(settings) => {
				debug!("Loaded {} from file", Self::filename());
				settings
			}
			Err(e) => {
				error!(
					"Failed to load {}: {}. Using defaults.",
					Self::filename(),
					e
				);
				Self::default()
			}
		}
	}

	/// Handle save operation with UI button and consistent error handling
	fn handle_save_operation_ui(&self, ui: &mut egui::Ui, button_label: &str) {
		if ui.button(button_label).clicked() {
			match self.save() {
				Ok(()) => {
					debug!("{} saved successfully", button_label.replace(' ', ""));
				}
				Err(e) => {
					error!("Failed to save: {e}");
				}
			}
		}
	}
}
