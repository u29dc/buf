use serde_json::json;

use crate::commands::{CommandOutput, CommandResult, GlobalOptions};
use crate::config::{ConfigInspection, inspect_runtime, load_runtime};
use crate::error::ProcessExit;

pub fn show(options: &GlobalOptions) -> CommandResult {
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let data = present_config(&ConfigInspection {
        paths: runtime.paths.clone(),
        file_exists: runtime.file_exists,
        env_exists: runtime.env_exists,
        file_config: runtime.file_config.clone(),
        parse_error: None,
        settings: runtime.settings.clone(),
        token_value: runtime.token_value.clone(),
        media_credentials: runtime.media_credentials.clone(),
    });

    Ok(CommandOutput::new("config.show", data).with_text(format!(
        "config resolved from {}",
        runtime.paths.env_file.display()
    )))
}

pub fn validate(options: &GlobalOptions) -> CommandResult {
    let inspection = inspect_runtime(&options.path_overrides(), options.api_base_url.as_deref());
    let mut warnings = Vec::new();
    if !inspection.file_exists {
        warnings.push("config file is optional and currently absent".to_owned());
    }
    if !inspection.env_exists {
        warnings.push("env file is missing; buf is relying on process environment only".to_owned());
    }
    if !inspection.settings.token.present {
        warnings.push("BUF_API_TOKEN is missing; Buffer API commands will be blocked".to_owned());
    }
    if !inspection.settings.media.ready {
        warnings.push(
            "BUF_MEDIA_* settings are incomplete; local media staging will be unavailable"
                .to_owned(),
        );
    }
    if inspection.parse_error.is_some() {
        warnings.push("config file contains invalid TOML".to_owned());
    }

    let data = json!({
        "valid": inspection.parse_error.is_none(),
        "warnings": warnings,
        "config": present_config(&inspection),
    });

    let exit_status = if inspection.parse_error.is_some() {
        ProcessExit::Failure
    } else {
        ProcessExit::Success
    };

    Ok(CommandOutput::new("config.validate", data)
        .with_text("config validate complete")
        .with_exit_status(exit_status))
}

fn present_config(inspection: &ConfigInspection) -> serde_json::Value {
    json!({
        "paths": inspection.paths,
        "configFileExists": inspection.file_exists,
        "envFileExists": inspection.env_exists,
        "buffer": {
            "apiBaseUrl": inspection.settings.api_base_url,
            "requestTimeoutMs": inspection.settings.request_timeout_ms,
            "token": inspection.settings.token,
        },
        "defaults": {
            "organizationId": inspection.settings.default_organization_id,
            "channels": inspection.settings.default_channels,
        },
        "media": inspection.settings.media,
        "fileConfig": inspection.file_config,
    })
}
