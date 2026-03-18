use std::fs;
use std::io::{self, Read};

use chrono::DateTime;
use serde_json::{Map, Value, json};

use crate::buffer_api::{BufferClient, ListPostsOptions, Post};
use crate::cli::{CreateTarget, InstagramPostType, PostsCreateArgs, PostsGetArgs, PostsListArgs};
use crate::commands::{
    CommandOutput, CommandResult, GlobalOptions, build_client, filter_channels,
    resolve_organization_id, validate_limit,
};
use crate::config::load_runtime;
use crate::error::CommandError;
use crate::media::{PreparedMediaBundle, prepare_media_for_post};

pub fn list(options: &GlobalOptions, args: &PostsListArgs) -> CommandResult {
    validate_limit(args.limit, "--limit")?;
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let client = build_client(&runtime)?;
    let (organization_id, _) = resolve_organization_id(&client, &runtime.settings)?;
    let channel_ids = resolve_list_channel_ids(&client, &organization_id, args)?;

    if args.service.is_some() && channel_ids.is_empty() {
        return Ok(empty_list_output(args));
    }

    let mut response = client.list_posts(&ListPostsOptions {
        organization_id,
        channel_ids,
        status: args.status,
        from: args.from.clone(),
        to: args.to.clone(),
        limit: args.limit,
        cursor: args.cursor.clone(),
    })?;
    let posts = serialize_posts(std::mem::take(&mut response.posts))?;
    let count = posts.len();
    Ok(CommandOutput::new(
        "posts.list",
        json!({
            "posts": posts,
            "pageInfo": response.page_info,
            "query": build_list_query(args),
        }),
    )
    .with_count(count)
    .with_total(count)
    .with_has_more(response.page_info.has_more)
    .with_text(format!("{count} post(s) matched")))
}

pub fn get(options: &GlobalOptions, args: &PostsGetArgs) -> CommandResult {
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let client = build_client(&runtime)?;
    let post = client.get_post(&args.post_id)?.ok_or_else(|| {
        CommandError::failure(
            "NOT_FOUND",
            format!("post `{}` was not found", args.post_id),
            "Verify the Buffer post id and retry",
        )
    })?;
    let post = serialize_post(post)?;

    Ok(CommandOutput::new("posts.get", json!({ "post": post }))
        .with_count(1)
        .with_total(1)
        .with_has_more(false)
        .with_text(format!("post: {}", args.post_id)))
}

pub fn create(options: &GlobalOptions, args: &PostsCreateArgs) -> CommandResult {
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let client = build_client(&runtime)?;
    let channel = client.get_channel(&args.channel)?.ok_or_else(|| {
        CommandError::failure(
            "NOT_FOUND",
            format!("channel `{}` was not found", args.channel),
            "Run `buf channels list` to inspect available channel ids",
        )
    })?;

    let body = resolve_body(args)?;
    let prepared_media = prepare_media_for_post(
        &runtime,
        &channel.service,
        args.post_type,
        &args.media,
        args.dry_run,
    )?;
    let request = build_create_input(&channel.service, args, &body, &prepared_media)?;

    if args.dry_run {
        return Ok(CommandOutput::new(
            "posts.create",
            json!({
                "dryRun": true,
                "channel": channel,
                "request": request,
                "stagedMedia": prepared_media,
                "post": Value::Null,
            }),
        )
        .with_count(1)
        .with_total(1)
        .with_has_more(false)
        .with_text("dry-run request generated"));
    }

    let post = serialize_post(client.create_post(request.clone())?)?;
    Ok(CommandOutput::new(
        "posts.create",
        json!({
            "dryRun": false,
            "channel": channel,
            "request": request,
            "stagedMedia": prepared_media,
            "post": post,
        }),
    )
    .with_count(1)
    .with_total(1)
    .with_has_more(false)
    .with_text("post created"))
}

fn resolve_list_channel_ids(
    client: &BufferClient,
    organization_id: &str,
    args: &PostsListArgs,
) -> Result<Vec<String>, CommandError> {
    if args.service.is_none() {
        return Ok(args.channel.iter().cloned().collect());
    }

    // Buffer posts filtering supports channelIds but not service, so resolve the
    // requested service to channel ids before querying posts.
    let channels = client.list_channels(organization_id)?;
    let mut channel_ids = filter_channels(&channels, args.service, None)
        .into_iter()
        .map(|channel| channel.id)
        .collect::<Vec<_>>();

    if let Some(channel_id) = args.channel.as_ref() {
        channel_ids.retain(|candidate| candidate == channel_id);
    }

    Ok(channel_ids)
}

fn empty_list_output(args: &PostsListArgs) -> CommandOutput {
    CommandOutput::new(
        "posts.list",
        json!({
            "posts": [],
            "pageInfo": {
                "hasMore": false,
                "nextCursor": Value::Null,
            },
            "query": build_list_query(args),
        }),
    )
    .with_count(0)
    .with_total(0)
    .with_has_more(false)
    .with_text("0 post(s) matched")
}

fn build_list_query(args: &PostsListArgs) -> Value {
    json!({
        "channel": args.channel,
        "service": args.service.map(|service| service.as_str().to_owned()),
        "status": args.status.map(|status| status.as_str().to_owned()),
        "from": args.from,
        "to": args.to,
        "limit": args.limit,
        "cursor": args.cursor,
    })
}

fn serialize_post(post: Post) -> Result<Value, CommandError> {
    let mut value = serde_json::to_value(post).map_err(|error| {
        CommandError::failure(
            "SERIALIZATION_ERROR",
            format!("failed to serialize post output: {error}"),
            "Retry the command after reducing output size",
        )
    })?;
    inject_published_url(&mut value);
    Ok(value)
}

fn serialize_posts(posts: Vec<Post>) -> Result<Vec<Value>, CommandError> {
    posts.into_iter().map(serialize_post).collect()
}

fn inject_published_url(value: &mut Value) {
    let Value::Object(map) = value else {
        return;
    };

    let published_url = map.get("externalLink").cloned().unwrap_or(Value::Null);
    map.insert("publishedUrl".to_owned(), published_url);
}

fn resolve_body(args: &PostsCreateArgs) -> Result<String, CommandError> {
    let mut sources = 0;
    if args.body.is_some() {
        sources += 1;
    }
    if args.body_file.is_some() {
        sources += 1;
    }
    if args.stdin {
        sources += 1;
    }
    if sources != 1 {
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            "provide exactly one post body source",
            "Use one of --body, --body-file, or --stdin",
        ));
    }

    if let Some(body) = args.body.as_deref() {
        return Ok(body.to_owned());
    }
    if let Some(path) = args.body_file.as_ref() {
        return fs::read_to_string(path).map_err(|error| {
            CommandError::failure(
                "RUNTIME_ERROR",
                format!("failed to read {}: {error}", path.display()),
                "Check the file path and retry",
            )
        });
    }

    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer).map_err(|error| {
        CommandError::failure(
            "RUNTIME_ERROR",
            format!("failed to read stdin: {error}"),
            "Pipe post text into stdin and retry",
        )
    })?;
    Ok(buffer)
}

fn build_create_input(
    service: &str,
    args: &PostsCreateArgs,
    body: &str,
    prepared_media: &PreparedMediaBundle,
) -> Result<Value, CommandError> {
    if body.trim().is_empty() {
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            "post body cannot be empty",
            "Provide non-empty text via --body, --body-file, or --stdin",
        ));
    }

    if matches!(args.target, CreateTarget::Schedule) {
        let Some(at) = args.at.as_deref() else {
            return Err(CommandError::failure(
                "VALIDATION_ERROR",
                "`--at` is required with `--target schedule`",
                "Pass a full ISO-8601 timestamp with timezone",
            ));
        };
        validate_rfc3339(at, "--at")?;
    } else if let Some(at) = args.at.as_deref() {
        validate_rfc3339(at, "--at")?;
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            "`--at` is only valid with `--target schedule`",
            "Remove --at or switch the target to schedule",
        ));
    }

    if let Some(link_url) = args.link_url.as_deref() {
        validate_public_url(link_url, "--link-url")?;
    }

    let metadata = build_metadata(service, args, prepared_media.effective_post_type.as_deref())?;
    Ok(json!({
        "channelId": args.channel,
        "text": body.trim(),
        "saveToDraft": matches!(args.target, CreateTarget::Draft),
        "mode": if matches!(args.target, CreateTarget::Draft) {
            Value::Null
        } else {
            json!(args.target.share_mode())
        },
        "dueAt": args.at,
        "schedulingType": args.delivery.as_str(),
        "assets": prepared_media.assets.clone(),
        "metadata": metadata,
    }))
}

fn build_metadata(
    service: &str,
    args: &PostsCreateArgs,
    effective_post_type: Option<&str>,
) -> Result<Value, CommandError> {
    let mut service_meta = match args.meta_json.as_deref() {
        Some(raw) => {
            let parsed: Value = serde_json::from_str(raw).map_err(|error| {
                CommandError::failure(
                    "VALIDATION_ERROR",
                    format!("invalid --meta-json payload: {error}"),
                    "Pass a valid JSON object string",
                )
            })?;
            parsed.as_object().cloned().ok_or_else(|| {
                CommandError::failure(
                    "VALIDATION_ERROR",
                    "--meta-json must be a JSON object",
                    "Pass a valid JSON object string",
                )
            })?
        }
        None => Map::new(),
    };

    if let Some(first_comment) = args.first_comment.as_deref() {
        service_meta.insert("firstComment".to_owned(), json!(first_comment));
    }

    match service {
        "instagram" => {
            let post_type = effective_post_type
                .map(ToOwned::to_owned)
                .or_else(|| args.post_type.map(|item| item.as_str().to_owned()))
                .unwrap_or_else(|| InstagramPostType::Post.as_str().to_owned());
            service_meta.insert("type".to_owned(), json!(post_type));
            if let Some(link_url) = args.link_url.as_deref() {
                service_meta.insert("link".to_owned(), json!(link_url));
            }
            if args.share_to_feed {
                service_meta.insert("shouldShareToFeed".to_owned(), json!(true));
            }
            if service_meta.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(json!({ "instagram": Value::Object(service_meta) }))
            }
        }
        "linkedin" => {
            if args
                .post_type
                .is_some_and(|post_type| post_type != InstagramPostType::Post)
            {
                return Err(CommandError::failure(
                    "VALIDATION_ERROR",
                    "LinkedIn only supports the default `post` type in this prototype",
                    "Remove --type or leave it as `post`",
                ));
            }
            if args.share_to_feed {
                return Err(CommandError::failure(
                    "VALIDATION_ERROR",
                    "--share-to-feed only applies to Instagram channels",
                    "Remove --share-to-feed for LinkedIn",
                ));
            }
            if let Some(link_url) = args.link_url.as_deref() {
                service_meta.insert("linkAttachment".to_owned(), json!({ "url": link_url }));
            }
            if service_meta.is_empty() {
                Ok(Value::Null)
            } else {
                Ok(json!({ "linkedin": Value::Object(service_meta) }))
            }
        }
        "threads" => {
            if args
                .post_type
                .is_some_and(|post_type| post_type != InstagramPostType::Post)
            {
                return Err(CommandError::failure(
                    "VALIDATION_ERROR",
                    "Threads only supports the default `post` type in this prototype",
                    "Remove --type or leave it as `post`",
                ));
            }
            if !service_meta.is_empty() || args.link_url.is_some() || args.share_to_feed {
                return Err(CommandError::failure(
                    "VALIDATION_ERROR",
                    "service-specific metadata is not implemented for `threads`",
                    "Use a plain Threads post or extend Threads metadata support first",
                ));
            }
            Ok(Value::Null)
        }
        _ => {
            if !service_meta.is_empty() || args.link_url.is_some() || args.share_to_feed {
                return Err(CommandError::failure(
                    "VALIDATION_ERROR",
                    format!("service-specific metadata is not implemented for `{service}`"),
                    "Use an Instagram or LinkedIn channel for service-specific metadata fields",
                ));
            }
            Ok(Value::Null)
        }
    }
}

fn validate_rfc3339(value: &str, flag: &str) -> Result<(), CommandError> {
    DateTime::parse_from_rfc3339(value).map_err(|error| {
        CommandError::failure(
            "VALIDATION_ERROR",
            format!("{flag} must be a valid RFC 3339 timestamp: {error}"),
            "Use a full ISO-8601 timestamp with timezone, for example 2026-03-20T09:00:00+00:00",
        )
    })?;
    Ok(())
}

fn validate_public_url(value: &str, flag: &str) -> Result<(), CommandError> {
    let parsed = reqwest::Url::parse(value).map_err(|error| {
        CommandError::failure(
            "VALIDATION_ERROR",
            format!("{flag} must be a valid absolute URL: {error}"),
            "Use a public https:// URL",
        )
    })?;
    let scheme = parsed.scheme();
    if scheme != "http" && scheme != "https" {
        return Err(CommandError::failure(
            "VALIDATION_ERROR",
            format!("{flag} must use http or https"),
            "Use a public https:// URL",
        ));
    }
    Ok(())
}
