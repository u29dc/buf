pub mod channels;
pub mod config;
pub mod health;
pub mod posts;
pub mod tools;

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::buffer_api::{BufferClient, Channel, Organization};
use crate::cli::ChannelService;
use crate::config::{PathOverrides, ResolvedSettings, RuntimeContext};
use crate::error::{CommandError, ProcessExit};

pub type CommandResult = Result<CommandOutput, CommandError>;

#[derive(Debug, Clone, Default)]
pub struct CommandMeta {
    pub count: Option<usize>,
    pub total: Option<usize>,
    pub has_more: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct CommandOutput {
    pub tool: &'static str,
    pub data: Value,
    pub text: String,
    pub meta: CommandMeta,
    pub exit_status: ProcessExit,
}

#[derive(Debug, Clone, Default)]
pub struct GlobalOptions {
    pub home: Option<PathBuf>,
    pub config_file: Option<PathBuf>,
    pub env_file: Option<PathBuf>,
    pub api_base_url: Option<String>,
}

impl CommandOutput {
    #[must_use]
    pub fn new(tool: &'static str, data: Value) -> Self {
        Self {
            tool,
            data,
            text: String::new(),
            meta: CommandMeta::default(),
            exit_status: ProcessExit::Success,
        }
    }

    #[must_use]
    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    #[must_use]
    pub fn with_count(mut self, count: usize) -> Self {
        self.meta.count = Some(count);
        self
    }

    #[must_use]
    pub fn with_total(mut self, total: usize) -> Self {
        self.meta.total = Some(total);
        self
    }

    #[must_use]
    pub fn with_has_more(mut self, has_more: bool) -> Self {
        self.meta.has_more = Some(has_more);
        self
    }

    #[must_use]
    pub const fn with_exit_status(mut self, exit_status: ProcessExit) -> Self {
        self.exit_status = exit_status;
        self
    }
}

impl GlobalOptions {
    #[must_use]
    pub fn path_overrides(&self) -> PathOverrides {
        PathOverrides {
            home: self.home.clone(),
            config_file: self.config_file.clone(),
            env_file: self.env_file.clone(),
        }
    }
}

pub fn build_client(runtime: &RuntimeContext) -> Result<BufferClient, CommandError> {
    let Some(token) = runtime.token_value.clone() else {
        return Err(CommandError::blocked(
            "TOKEN_MISSING",
            "Buffer API token is not configured",
            format!(
                "Set BUF_API_TOKEN in the shell or in `{}`",
                runtime.paths.env_file.display()
            ),
        ));
    };

    BufferClient::new(
        runtime.settings.api_base_url.clone(),
        token,
        runtime.settings.request_timeout_ms,
    )
}

pub fn resolve_organization_id(
    client: &BufferClient,
    settings: &ResolvedSettings,
) -> Result<(String, Vec<Organization>), CommandError> {
    let organizations = client.list_organizations()?;
    if organizations.is_empty() {
        return Err(CommandError::failure(
            "ORG_NOT_FOUND",
            "Buffer account returned no organizations",
            "Confirm that the token belongs to a Buffer account with at least one organization",
        ));
    }

    if let Some(explicit_id) = settings.default_organization_id.as_deref() {
        if organizations
            .iter()
            .any(|organization| organization.id == explicit_id)
        {
            return Ok((explicit_id.to_owned(), organizations));
        }
        return Err(CommandError::failure(
            "ORG_NOT_FOUND",
            format!("configured organization `{explicit_id}` was not found"),
            "Update BUF_ORGANIZATION_ID or the config file to an available organization id",
        )
        .with_details(json!({
            "organizationId": explicit_id,
            "organizations": organizations,
        })));
    }

    if organizations.len() == 1 {
        return Ok((organizations[0].id.clone(), organizations));
    }

    Err(CommandError::blocked(
        "ORG_AMBIGUOUS",
        "multiple Buffer organizations are available",
        "Set BUF_ORGANIZATION_ID or add defaultOrganizationId to buf.config.toml",
    )
    .with_details(json!({ "organizations": organizations })))
}

pub fn filter_channels(
    channels: &[Channel],
    service: Option<ChannelService>,
    query: Option<&str>,
) -> Vec<Channel> {
    channels
        .iter()
        .filter(|channel| {
            service.is_none_or(|service_filter| channel.service == service_filter.as_str())
        })
        .filter(|channel| match_query(channel, query))
        .cloned()
        .collect()
}

pub fn resolve_default_channel_id(
    settings: &ResolvedSettings,
    service: ChannelService,
) -> Option<&str> {
    match service {
        ChannelService::Instagram => settings.default_channels.instagram.as_deref(),
        ChannelService::LinkedIn => settings.default_channels.linkedin.as_deref(),
    }
}

fn match_query(channel: &Channel, query: Option<&str>) -> bool {
    let Some(query_text) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };

    let query_lower = query_text.to_ascii_lowercase();
    [
        Some(channel.id.as_str()),
        Some(channel.name.as_str()),
        channel.display_name.as_deref(),
        channel.external_link.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|candidate| candidate.to_ascii_lowercase().contains(&query_lower))
}

pub fn validate_limit(limit: usize, flag: &str) -> Result<(), CommandError> {
    if limit == 0 {
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            format!("{flag} must be at least 1"),
            format!("Pass `{flag} 1` or a larger integer"),
        ));
    }
    Ok(())
}
