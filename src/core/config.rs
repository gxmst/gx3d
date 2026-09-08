//! User configuration persisted across sessions in `config.json` next to the
//! working directory. Loaded once at startup, saved whenever the pause menu
//! changes a setting. Missing or corrupt files fall back to defaults.

use serde::{Deserialize, Serialize};

pub const CONFIG_PATH: &str = "config.json";

/// Mouse-sensitivity multipliers applied on top of the scene's base value.
pub const SENSITIVITY_OPTIONS: [f32; 5] = [0.5, 0.75, 1.0, 1.5, 2.0];
pub const DEFAULT_SENSITIVITY_INDEX: usize = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UserConfig {
    /// Index into `game::menu::RESOLUTION_OPTIONS`.
    pub resolution_index: usize,
    /// Index into [`SENSITIVITY_OPTIONS`].
    pub sensitivity_index: usize,
    pub god_mode_enabled: bool,
    /// Whether the player has opened the F help overlay at least once. Gates
    /// the highlighted first-time hint on the HUD.
    pub help_seen: bool,
    /// Selected weather mode (clear/rain/snow/random).
    pub weather: crate::game::systems::weather::WeatherKind,
}

impl Default for UserConfig {
    fn default() -> Self {
        Self {
            resolution_index: 2, // 1920x1080
            sensitivity_index: DEFAULT_SENSITIVITY_INDEX,
            god_mode_enabled: crate::game::menu::DEFAULT_GOD_MODE_ENABLED,
            help_seen: false,
            weather: Default::default(),
        }
    }
}

impl UserConfig {
    pub fn load() -> Self {
        match std::fs::read_to_string(CONFIG_PATH) {
            Ok(text) => match serde_json::from_str::<UserConfig>(&text) {
                Ok(config) => config.sanitized(),
                Err(error) => {
                    log::warn!("config.json is invalid ({error}); using defaults");
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        }
    }

    /// Write to disk; failures are logged, never fatal (a read-only working
    /// directory should not break the game).
    pub fn save(&self) {
        match serde_json::to_string_pretty(self) {
            Ok(text) => {
                if let Err(error) = std::fs::write(CONFIG_PATH, text) {
                    log::warn!("Failed to save config.json: {error}");
                }
            }
            Err(error) => log::warn!("Failed to serialize config: {error}"),
        }
    }

    pub fn sensitivity_multiplier(&self) -> f32 {
        SENSITIVITY_OPTIONS
            .get(self.sensitivity_index)
            .copied()
            .unwrap_or(1.0)
    }

    fn sanitized(mut self) -> Self {
        if self.resolution_index >= crate::game::menu::RESOLUTION_OPTIONS.len() {
            self.resolution_index = 2;
        }
        if self.sensitivity_index >= SENSITIVITY_OPTIONS.len() {
            self.sensitivity_index = DEFAULT_SENSITIVITY_INDEX;
        }
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_indices_are_sanitized_on_load() {
        let config = UserConfig {
            resolution_index: 99,
            sensitivity_index: 99,
            god_mode_enabled: true,
            help_seen: true,
            weather: Default::default(),
        }
        .sanitized();
        assert_eq!(config.resolution_index, 2);
        assert_eq!(config.sensitivity_index, DEFAULT_SENSITIVITY_INDEX);
        assert!(config.help_seen);
    }

    #[test]
    fn config_roundtrips_through_json() {
        let config = UserConfig {
            resolution_index: 1,
            sensitivity_index: 3,
            god_mode_enabled: false,
            help_seen: true,
            weather: crate::game::systems::weather::WeatherKind::Rain,
        };
        let text = serde_json::to_string(&config).unwrap();
        let parsed: UserConfig = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed.resolution_index, 1);
        assert_eq!(parsed.sensitivity_index, 3);
        assert!(!parsed.god_mode_enabled);
        assert!(parsed.help_seen);
    }

    #[test]
    fn unknown_or_missing_fields_fall_back_to_defaults() {
        let parsed: UserConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed.resolution_index, 2);
        let parsed: UserConfig =
            serde_json::from_str(r#"{"help_seen": true, "future_field": 1}"#).unwrap();
        assert!(parsed.help_seen);
    }
}
