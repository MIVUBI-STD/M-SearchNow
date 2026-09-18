use crate::{
    error::{BackendError, BackendResult},
    storage::AtomicFileStore,
};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const CURRENT_SCHEMA_VERSION: u32 = 1;
const MAX_SETTINGS_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppSettings {
    #[serde(default = "current_schema_version")]
    pub schema_version: u32,
    #[serde(default)]
    pub minecraft: MinecraftSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MinecraftSettings {
    #[serde(default)]
    pub root_override: Option<PathBuf>,
    #[serde(default)]
    pub include_preview: bool,
    #[serde(default = "default_true")]
    pub include_legacy_uwp: bool,
    #[serde(default)]
    pub include_development_content: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            minecraft: MinecraftSettings::default(),
        }
    }
}

impl Default for MinecraftSettings {
    fn default() -> Self {
        Self {
            root_override: None,
            include_preview: false,
            include_legacy_uwp: true,
            include_development_content: false,
        }
    }
}

impl AppSettings {
    pub fn validate(&self) -> BackendResult<()> {
        if self.schema_version != CURRENT_SCHEMA_VERSION {
            return Err(BackendError::new(
                "settings_schema_unsupported",
                format!(
                    "Settings schema {} is not supported by this SearchNow build.",
                    self.schema_version
                ),
            ));
        }
        if self
            .minecraft
            .root_override
            .as_ref()
            .is_some_and(|path| path.as_os_str().is_empty())
        {
            return Err(BackendError::new(
                "settings_minecraft_root_invalid",
                "Minecraft root override cannot be empty.",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SettingsStore {
    file: AtomicFileStore,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            file: AtomicFileStore::new(path, ".settings.backup", ".settings"),
        }
    }

    pub fn path(&self) -> &Path {
        self.file.path()
    }

    pub fn load(&self) -> BackendResult<AppSettings> {
        let Some(text) = self.file.read_to_string(MAX_SETTINGS_BYTES)? else {
            return Ok(AppSettings::default());
        };
        let settings: AppSettings = serde_json::from_str(&text).map_err(|error| {
            BackendError::new(
                "settings_invalid_json",
                format!("SearchNow settings are invalid: {error}"),
            )
        })?;
        settings.validate()?;
        Ok(settings)
    }

    pub fn save(&self, settings: &AppSettings) -> BackendResult<()> {
        settings.validate()?;
        let bytes = serde_json::to_vec_pretty(settings).map_err(|error| {
            BackendError::new(
                "settings_serialize_failed",
                format!("SearchNow could not serialize settings: {error}"),
            )
        })?;
        self.file.replace(&bytes, MAX_SETTINGS_BYTES)
    }
}

fn current_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn defaults_are_local_safe() {
        let settings = AppSettings::default();
        assert!(!settings.minecraft.include_preview);
        assert!(settings.minecraft.include_legacy_uwp);
        assert!(!settings.minecraft.include_development_content);
        assert!(settings.minecraft.root_override.is_none());
    }

    #[test]
    fn settings_round_trip() {
        let directory = tempfile::tempdir().expect("tempdir");
        let store = SettingsStore::new(directory.path().join("settings.json"));
        let mut settings = AppSettings::default();
        settings.minecraft.include_preview = true;
        store.save(&settings).expect("save");
        assert_eq!(store.load().expect("load"), settings);
    }

    #[test]
    fn recovers_backup_when_primary_is_missing() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("settings.json");
        let store = SettingsStore::new(&path);
        let mut settings = AppSettings::default();
        settings.minecraft.include_preview = true;
        let backup = directory.path().join(".settings.backup");
        fs::write(&backup, serde_json::to_vec(&settings).expect("json")).expect("backup");
        assert_eq!(store.load().expect("load"), settings);
        assert!(path.exists());
        assert!(!backup.exists());
    }

    #[test]
    fn future_schema_fails_closed() {
        let settings = AppSettings {
            schema_version: CURRENT_SCHEMA_VERSION + 1,
            ..AppSettings::default()
        };
        let error = settings.validate().expect_err("future schema must fail");
        assert_eq!(error.code(), "settings_schema_unsupported");
    }

    #[test]
    fn unknown_settings_fields_fail_closed() {
        let json = r#"{
            "schemaVersion": 1,
            "minecraft": {
                "includePreview": false,
                "includeLegacyUwp": true,
                "includeDevelopmentContent": false,
                "unexpectedSetting": true
            }
        }"#;
        assert!(serde_json::from_str::<AppSettings>(json).is_err());
    }

    #[test]
    fn store_rejects_invalid_json() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("settings.json");
        fs::write(&path, b"{not-json").expect("corrupt settings");
        let store = SettingsStore::new(path);

        let error = store.load().expect_err("invalid JSON must fail closed");
        assert_eq!(error.code(), "settings_invalid_json");
    }

    #[test]
    fn store_rejects_unsupported_schema() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("settings.json");
        fs::write(
            &path,
            br#"{"schemaVersion":999,"minecraft":{"includePreview":false,"includeLegacyUwp":true,"includeDevelopmentContent":false}}"#,
        )
        .expect("future settings");
        let store = SettingsStore::new(path);

        let error = store
            .load()
            .expect_err("unsupported schema must fail closed");
        assert_eq!(error.code(), "settings_schema_unsupported");
    }

    #[test]
    fn persisted_empty_root_override_fails_closed() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("settings.json");
        fs::write(
            &path,
            br#"{"schemaVersion":1,"minecraft":{"rootOverride":"","includePreview":false,"includeLegacyUwp":true,"includeDevelopmentContent":false}}"#,
        )
        .expect("invalid root settings");
        let store = SettingsStore::new(path);

        let error = store
            .load()
            .expect_err("empty root override must fail closed");
        assert_eq!(error.code(), "settings_minecraft_root_invalid");
    }
}
