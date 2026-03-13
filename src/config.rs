use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::env::{EnvValueSource, ResolvedEnvVar, resolve_env_var};
use crate::error::CommandError;

const DEFAULT_API_BASE_URL: &str = "https://api.buffer.com";
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
const DEFAULT_MEDIA_PROVIDER: &str = "cloudflare-r2";
const DEFAULT_MEDIA_KEY_PREFIX: &str = "tmp/buf";

#[derive(Debug, Clone, Default)]
pub struct PathOverrides {
    pub home: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub env_file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BufPaths {
    pub home: PathBuf,
    pub config_file: PathBuf,
    pub env_file: PathBuf,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileConfig {
    #[serde(alias = "api_base_url")]
    #[serde(alias = "api_url")]
    pub api_base_url: Option<String>,
    #[serde(alias = "request_timeout_ms")]
    pub request_timeout_ms: Option<u64>,
    #[serde(alias = "organization_id")]
    #[serde(alias = "default_organization_id")]
    pub default_organization_id: Option<String>,
    #[serde(alias = "default_channels")]
    pub default_channels: DefaultChannels,
    pub media: FileMediaConfig,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct DefaultChannels {
    pub instagram: Option<String>,
    pub linkedin: Option<String>,
    pub threads: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct FileMediaConfig {
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub base_url: Option<String>,
    pub key_prefix: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretInfo {
    pub present: bool,
    pub key: Option<String>,
    pub source: Option<String>,
    pub masked: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSettings {
    pub provider: String,
    pub ready: bool,
    pub endpoint: Option<String>,
    pub bucket: Option<String>,
    pub base_url: Option<String>,
    pub key_prefix: String,
    pub access_key_id: SecretInfo,
    pub secret_access_key: SecretInfo,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedSettings {
    pub api_base_url: String,
    pub request_timeout_ms: u64,
    pub default_organization_id: Option<String>,
    pub default_channels: DefaultChannels,
    pub token: SecretInfo,
    pub media: MediaSettings,
}

#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub paths: BufPaths,
    pub file_exists: bool,
    pub env_exists: bool,
    pub file_config: Option<FileConfig>,
    pub settings: ResolvedSettings,
    pub token_value: Option<String>,
    pub media_credentials: Option<MediaCredentials>,
}

#[derive(Debug, Clone)]
pub struct ConfigInspection {
    pub paths: BufPaths,
    pub file_exists: bool,
    pub env_exists: bool,
    pub file_config: Option<FileConfig>,
    pub parse_error: Option<String>,
    pub settings: ResolvedSettings,
    pub token_value: Option<String>,
    pub media_credentials: Option<MediaCredentials>,
}

#[derive(Debug, Clone)]
pub struct MediaCredentials {
    pub endpoint: String,
    pub bucket: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub base_url: String,
    pub key_prefix: String,
}

pub fn inspect_runtime(overrides: &PathOverrides, api_base_url: Option<&str>) -> ConfigInspection {
    let paths = resolve_paths(overrides);
    let file_exists = paths.config_file.exists();
    let env_exists = paths.env_file.exists();

    let (file_config, parse_error) = match load_file_config(&paths.config_file) {
        Ok(config) => (config, None),
        Err(error) => (None, Some(error)),
    };

    let token = resolve_env_var(&["BUF_API_TOKEN"], &paths.env_file);
    let api_base = api_base_url
        .map(ToOwned::to_owned)
        .or_else(|| resolve_env_var(&["BUF_API_BASE_URL"], &paths.env_file).map(|item| item.value))
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.api_base_url.clone())
        })
        .unwrap_or_else(|| DEFAULT_API_BASE_URL.to_owned());

    let timeout_ms = resolve_env_var(&["BUF_REQUEST_TIMEOUT_MS"], &paths.env_file)
        .and_then(|item| item.value.parse::<u64>().ok())
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.request_timeout_ms)
        })
        .unwrap_or(DEFAULT_TIMEOUT_MS);

    let default_organization_id = resolve_env_var(&["BUF_ORGANIZATION_ID"], &paths.env_file)
        .map(|item| item.value)
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.default_organization_id.clone())
        });

    let default_channels = DefaultChannels {
        instagram: resolve_env_var(&["BUF_DEFAULT_CHANNEL_INSTAGRAM"], &paths.env_file)
            .map(|item| item.value)
            .or_else(|| {
                file_config
                    .as_ref()
                    .and_then(|config| config.default_channels.instagram.clone())
            }),
        linkedin: resolve_env_var(&["BUF_DEFAULT_CHANNEL_LINKEDIN"], &paths.env_file)
            .map(|item| item.value)
            .or_else(|| {
                file_config
                    .as_ref()
                    .and_then(|config| config.default_channels.linkedin.clone())
            }),
        threads: resolve_env_var(&["BUF_DEFAULT_CHANNEL_THREADS"], &paths.env_file)
            .map(|item| item.value)
            .or_else(|| {
                file_config
                    .as_ref()
                    .and_then(|config| config.default_channels.threads.clone())
            }),
    };

    let media_endpoint = resolve_env_var(&["BUF_MEDIA_ENDPOINT"], &paths.env_file)
        .map(|item| item.value)
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.media.endpoint.clone())
        });
    let media_bucket = resolve_env_var(&["BUF_MEDIA_BUCKET"], &paths.env_file)
        .map(|item| item.value)
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.media.bucket.clone())
        });
    let media_base_url = resolve_env_var(&["BUF_MEDIA_BASE_URL"], &paths.env_file)
        .map(|item| item.value)
        .or_else(|| {
            file_config
                .as_ref()
                .and_then(|config| config.media.base_url.clone())
        });
    let media_key_prefix = file_config
        .as_ref()
        .and_then(|config| config.media.key_prefix.clone())
        .unwrap_or_else(|| DEFAULT_MEDIA_KEY_PREFIX.to_owned());
    let media_access_key_id = resolve_env_var(&["BUF_MEDIA_ACCESS_KEY_ID"], &paths.env_file);
    let media_secret_access_key =
        resolve_env_var(&["BUF_MEDIA_SECRET_ACCESS_KEY"], &paths.env_file);

    let media = MediaSettings {
        provider: DEFAULT_MEDIA_PROVIDER.to_owned(),
        ready: media_endpoint.is_some()
            && media_bucket.is_some()
            && media_base_url.is_some()
            && media_access_key_id.is_some()
            && media_secret_access_key.is_some(),
        endpoint: media_endpoint.clone(),
        bucket: media_bucket.clone(),
        base_url: media_base_url.clone(),
        key_prefix: media_key_prefix.clone(),
        access_key_id: media_access_key_id
            .as_ref()
            .map_or_else(SecretInfo::missing, SecretInfo::from_env_var),
        secret_access_key: media_secret_access_key
            .as_ref()
            .map_or_else(SecretInfo::missing, SecretInfo::from_env_var),
    };

    let media_credentials = match (
        media_endpoint,
        media_bucket,
        media_access_key_id,
        media_secret_access_key,
        media_base_url,
    ) {
        (
            Some(endpoint),
            Some(bucket),
            Some(access_key_id),
            Some(secret_access_key),
            Some(base_url),
        ) => Some(MediaCredentials {
            endpoint,
            bucket,
            access_key_id: access_key_id.value,
            secret_access_key: secret_access_key.value,
            base_url,
            key_prefix: media_key_prefix,
        }),
        _ => None,
    };

    let token_info = token
        .as_ref()
        .map_or_else(SecretInfo::missing, SecretInfo::from_env_var);

    ConfigInspection {
        paths,
        file_exists,
        env_exists,
        file_config,
        parse_error,
        settings: ResolvedSettings {
            api_base_url: api_base,
            request_timeout_ms: timeout_ms,
            default_organization_id,
            default_channels,
            token: token_info,
            media,
        },
        token_value: token.map(|value| value.value),
        media_credentials,
    }
}

pub fn load_runtime(
    overrides: &PathOverrides,
    api_base_url: Option<&str>,
) -> Result<RuntimeContext, CommandError> {
    let inspection = inspect_runtime(overrides, api_base_url);
    if let Some(error) = inspection.parse_error.clone() {
        return Err(CommandError::blocked(
            "CONFIG_INVALID",
            format!(
                "failed to parse config file `{}`: {error}",
                inspection.paths.config_file.display()
            ),
            format!(
                "Fix the TOML syntax in `{}` or remove the file to use defaults",
                inspection.paths.config_file.display()
            ),
        ));
    }

    Ok(RuntimeContext {
        paths: inspection.paths,
        file_exists: inspection.file_exists,
        env_exists: inspection.env_exists,
        file_config: inspection.file_config,
        settings: inspection.settings,
        token_value: inspection.token_value,
        media_credentials: inspection.media_credentials,
    })
}

pub fn resolve_paths(overrides: &PathOverrides) -> BufPaths {
    let home = overrides.home.clone().unwrap_or_else(default_home);
    let home = make_absolute(home);
    let config_file = overrides
        .config_file
        .clone()
        .map(make_absolute)
        .unwrap_or_else(|| home.join("buf.config.toml"));
    let env_file = overrides
        .env_file
        .clone()
        .map(make_absolute)
        .unwrap_or_else(|| home.join(".env"));
    let temp_dir = home.join("tmp");

    BufPaths {
        home,
        config_file,
        env_file,
        temp_dir,
    }
}

fn load_file_config(path: &Path) -> Result<Option<FileConfig>, String> {
    match fs::read_to_string(path) {
        Ok(content) => toml::from_str::<FileConfig>(&content)
            .map(Some)
            .map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn default_home() -> PathBuf {
    if let Some(path) = std::env::var_os("BUF_HOME") {
        return PathBuf::from(path);
    }
    if let Some(path) = std::env::var_os("TOOLS_HOME") {
        return PathBuf::from(path).join("buf");
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".tools")
        .join("buf")
}

fn make_absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(path)
}

impl SecretInfo {
    fn missing() -> Self {
        Self {
            present: false,
            key: None,
            source: None,
            masked: None,
        }
    }

    fn from_env_var(resolved: &ResolvedEnvVar) -> Self {
        Self {
            present: true,
            key: Some(resolved.key.clone()),
            source: Some(match resolved.source {
                EnvValueSource::Process => "process".to_owned(),
                EnvValueSource::EnvFile => "envFile".to_owned(),
            }),
            masked: Some(mask_secret(&resolved.value)),
        }
    }
}

fn mask_secret(value: &str) -> String {
    let visible = value.chars().take(4).collect::<String>();
    if value.len() <= 4 {
        format!("{visible}***")
    } else {
        format!("{visible}...{}", value.len())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PathOverrides, inspect_runtime, resolve_paths};

    #[test]
    fn override_paths_win() {
        let paths = resolve_paths(&PathOverrides {
            home: Some(PathBuf::from("/tmp/buf-home")),
            config_file: Some(PathBuf::from("/tmp/custom/buf.config.toml")),
            env_file: Some(PathBuf::from("/tmp/custom/.env")),
        });
        assert_eq!(paths.home, Path::new("/tmp/buf-home"));
        assert_eq!(paths.config_file, Path::new("/tmp/custom/buf.config.toml"));
        assert_eq!(paths.env_file, Path::new("/tmp/custom/.env"));
        assert_eq!(paths.temp_dir, Path::new("/tmp/buf-home/tmp"));
    }

    #[test]
    fn threads_default_channel_resolves_from_env() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let home = std::env::temp_dir().join(format!("buf-config-test-{unique}"));
        fs::create_dir_all(&home).expect("create temp home");
        let env_file = home.join(".env");
        fs::write(
            &env_file,
            [
                "BUF_API_TOKEN=test-token",
                "BUF_DEFAULT_CHANNEL_INSTAGRAM=ig-id",
                "BUF_DEFAULT_CHANNEL_LINKEDIN=li-id",
                "BUF_DEFAULT_CHANNEL_THREADS=th-id",
            ]
            .join("\n"),
        )
        .expect("write env file");

        let inspection = inspect_runtime(
            &PathOverrides {
                home: Some(home.clone()),
                config_file: Some(home.join("buf.config.toml")),
                env_file: Some(env_file),
            },
            None,
        );

        assert_eq!(
            inspection.settings.default_channels.threads.as_deref(),
            Some("th-id")
        );

        fs::remove_dir_all(home).expect("cleanup temp home");
    }
}
