use serde_json::json;

use crate::cli::{ChannelsListArgs, ChannelsResolveArgs};
use crate::commands::{
    CommandOutput, CommandResult, build_client, filter_channels, resolve_default_channel_id,
    resolve_organization_id, validate_limit,
};
use crate::config::load_runtime;
use crate::error::CommandError;

pub fn list(options: &crate::commands::GlobalOptions, args: &ChannelsListArgs) -> CommandResult {
    validate_limit(args.limit, "--limit")?;
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let client = build_client(&runtime)?;
    let (organization_id, organizations) = resolve_organization_id(&client, &runtime.settings)?;
    let organization = organizations
        .iter()
        .find(|item| item.id == organization_id)
        .cloned();
    let channels = client.list_channels(&organization_id)?;
    let mut filtered = filter_channels(&channels, args.service, args.query.as_deref());
    if filtered.len() > args.limit {
        filtered.truncate(args.limit);
    }
    let count = filtered.len();

    Ok(CommandOutput::new(
        "channels.list",
        json!({
            "organization": organization,
            "channels": filtered,
            "query": {
                "service": args.service.map(|service| service.as_str().to_owned()),
                "query": args.query,
                "limit": args.limit,
            }
        }),
    )
    .with_count(count)
    .with_total(count)
    .with_has_more(false)
    .with_text(format!("{count} channel(s) matched")))
}

pub fn resolve(
    options: &crate::commands::GlobalOptions,
    args: &ChannelsResolveArgs,
) -> CommandResult {
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let client = build_client(&runtime)?;
    let (organization_id, organizations) = resolve_organization_id(&client, &runtime.settings)?;
    let organization = organizations
        .iter()
        .find(|item| item.id == organization_id)
        .cloned();
    let channels = client.list_channels(&organization_id)?;
    let filtered = filter_channels(&channels, Some(args.service), args.query.as_deref());

    let resolved = if let Some(default_id) =
        resolve_default_channel_id(&runtime.settings, args.service)
        && args.query.is_none()
        && let Some(channel) = filtered.iter().find(|item| item.id == default_id)
    {
        channel.clone()
    } else if filtered.len() == 1 {
        filtered[0].clone()
    } else if filtered.is_empty() {
        return Err(CommandError::failure(
            "NOT_FOUND",
            format!("no {} channels matched", args.service.as_str()),
            "Run `buf channels list --service <service>` to inspect available channel ids",
        )
        .with_details(json!({
            "organization": organization,
            "service": args.service.as_str(),
            "query": args.query,
        })));
    } else {
        return Err(CommandError::failure(
            "CHANNEL_AMBIGUOUS",
            format!("multiple {} channels matched", args.service.as_str()),
            "Use --query or configure a default channel id in buf.config.toml",
        )
        .with_details(json!({
            "organization": organization,
            "service": args.service.as_str(),
            "query": args.query,
            "matches": filtered,
        })));
    };

    Ok(CommandOutput::new(
        "channels.resolve",
        json!({
            "channel": resolved,
        }),
    )
    .with_count(1)
    .with_total(1)
    .with_has_more(false)
    .with_text(format!("resolved {}", args.service.as_str())))
}
