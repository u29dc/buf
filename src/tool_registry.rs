use std::sync::OnceLock;

use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolParameter {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub param_type: &'static str,
    pub required: bool,
    pub description: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolMetadata {
    pub name: &'static str,
    pub command: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub parameters: Vec<ToolParameter>,
    pub output_fields: Vec<&'static str>,
    pub output_schema: Value,
    pub input_schema: Value,
    pub idempotent: bool,
    pub rate_limit: Option<&'static str>,
    pub example: &'static str,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GlobalFlag {
    pub name: &'static str,
    #[serde(rename = "type")]
    pub flag_type: &'static str,
    pub description: &'static str,
}

static TOOL_REGISTRY: OnceLock<Vec<ToolMetadata>> = OnceLock::new();
static DOCUMENTED_SERVICES: [&str; 12] = [
    "instagram",
    "facebook",
    "twitter",
    "linkedin",
    "pinterest",
    "tiktok",
    "googlebusiness",
    "startPage",
    "mastodon",
    "youtube",
    "threads",
    "bluesky",
];

pub fn tool_registry() -> &'static [ToolMetadata] {
    TOOL_REGISTRY
        .get_or_init(|| {
            let mut tools = vec![
                tools_tool(),
                health_tool(),
                config_show_tool(),
                config_validate_tool(),
                channels_list_tool(),
                channels_resolve_tool(),
                posts_list_tool(),
                posts_get_tool(),
                posts_create_tool(),
                posts_delete_tool(),
                posts_limits_tool(),
            ];
            tools.sort_by(|left, right| {
                left.category
                    .cmp(right.category)
                    .then(left.name.cmp(right.name))
            });
            tools
        })
        .as_slice()
}

pub fn find_tool(name: &str) -> Option<&'static ToolMetadata> {
    tool_registry().iter().find(|tool| tool.name == name)
}

pub fn global_flags() -> &'static [GlobalFlag] {
    static FLAGS: [GlobalFlag; 5] = [
        GlobalFlag {
            name: "--text",
            flag_type: "boolean",
            description: "Emit human-readable output instead of the default JSON envelope.",
        },
        GlobalFlag {
            name: "--home",
            flag_type: "path",
            description: "Override BUF_HOME.",
        },
        GlobalFlag {
            name: "--config-file",
            flag_type: "path",
            description: "Override buf.config.toml path.",
        },
        GlobalFlag {
            name: "--env-file",
            flag_type: "path",
            description: "Override .env path.",
        },
        GlobalFlag {
            name: "--api-base-url",
            flag_type: "string",
            description: "Override the Buffer GraphQL API base URL.",
        },
    ];
    &FLAGS
}

fn tools_tool() -> ToolMetadata {
    ToolMetadata {
        name: "tools",
        command: "buf tools [name]",
        category: "infra",
        description: "List all available tools or return one tool metadata record.",
        parameters: vec![parameter(
            "name",
            "string",
            false,
            "Optional dotted tool name for detail mode.",
        )],
        output_fields: vec!["version", "globalFlags", "tools", "tool"],
        output_schema: json!({
            "type": "object",
            "properties": {
                "version": { "type": "string" },
                "globalFlags": { "type": "array" },
                "tools": { "type": "array" },
                "tool": { "type": "object" }
            }
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: None,
        example: "buf tools posts.create",
    }
}

fn health_tool() -> ToolMetadata {
    ToolMetadata {
        name: "health",
        command: "buf health",
        category: "infra",
        description: "Check Buffer auth, R2 staging config, ffmpeg prerequisites, and local runtime readiness.",
        parameters: vec![],
        output_fields: vec!["status", "paths", "checks", "summary"],
        output_schema: json!({
            "type": "object",
            "required": ["status", "paths", "checks", "summary"],
            "properties": {
                "status": { "type": "string" },
                "paths": { "type": "object" },
                "checks": { "type": "array" },
                "summary": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: None,
        example: "buf health",
    }
}

fn config_show_tool() -> ToolMetadata {
    ToolMetadata {
        name: "config.show",
        command: "buf config show",
        category: "config",
        description: "Show resolved Buffer, default-channel, and R2 media settings with masked secret metadata.",
        parameters: vec![],
        output_fields: vec![
            "paths",
            "configFileExists",
            "envFileExists",
            "buffer",
            "defaults",
            "media",
            "fileConfig",
        ],
        output_schema: json!({
            "type": "object",
            "required": ["paths", "configFileExists", "envFileExists", "buffer", "defaults", "media", "fileConfig"],
            "properties": {
                "paths": { "type": "object" },
                "configFileExists": { "type": "boolean" },
                "envFileExists": { "type": "boolean" },
                "buffer": { "type": "object" },
                "defaults": { "type": "object" },
                "media": { "type": "object" },
                "fileConfig": {},
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: None,
        example: "buf config show",
    }
}

fn config_validate_tool() -> ToolMetadata {
    ToolMetadata {
        name: "config.validate",
        command: "buf config validate",
        category: "config",
        description: "Validate config parsing and report missing Buffer or R2 prerequisites without touching the API.",
        parameters: vec![],
        output_fields: vec!["valid", "warnings", "config"],
        output_schema: json!({
            "type": "object",
            "required": ["valid", "warnings", "config"],
            "properties": {
                "valid": { "type": "boolean" },
                "warnings": { "type": "array" },
                "config": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: None,
        example: "buf config validate",
    }
}

fn channels_list_tool() -> ToolMetadata {
    ToolMetadata {
        name: "channels.list",
        command: "buf channels list [--service <service>] [--query <text>] [--limit <n>]",
        category: "channels",
        description: "List channels for the resolved organization with optional service and text filters.",
        parameters: vec![
            parameter(
                "--service",
                "string",
                false,
                "Filter by a documented Buffer service value such as `instagram`, `linkedin`, or `threads`.",
            ),
            parameter(
                "--query",
                "string",
                false,
                "Case-insensitive match against id, name, or display name.",
            ),
            parameter(
                "--limit",
                "integer",
                false,
                "Maximum number of channels to return.",
            ),
        ],
        output_fields: vec!["organization", "channels", "query"],
        output_schema: json!({
            "type": "object",
            "required": ["organization", "channels", "query"],
            "properties": {
                "organization": { "type": ["object", "null"] },
                "channels": { "type": "array" },
                "query": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "service": { "type": "string", "enum": DOCUMENTED_SERVICES },
                "query": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: Some("Buffer API limits apply; cache organization and channel lookups."),
        example: "buf channels list --service instagram --limit 10",
    }
}

fn channels_resolve_tool() -> ToolMetadata {
    ToolMetadata {
        name: "channels.resolve",
        command: "buf channels resolve --service <service> [--query <text>]",
        category: "channels",
        description: "Resolve exactly one channel or fail with a deterministic ambiguity error.",
        parameters: vec![
            parameter("--service", "string", true, "Required service selector."),
            parameter(
                "--query",
                "string",
                false,
                "Optional case-insensitive match against channel identity fields.",
            ),
        ],
        output_fields: vec!["channel"],
        output_schema: json!({
            "type": "object",
            "required": ["channel"],
            "properties": {
                "channel": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "required": ["service"],
            "properties": {
                "service": { "type": "string", "enum": DOCUMENTED_SERVICES },
                "query": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: Some("Buffer API limits apply; cache organization and channel lookups."),
        example: "buf channels resolve --service linkedin --query example-company",
    }
}

fn posts_list_tool() -> ToolMetadata {
    ToolMetadata {
        name: "posts.list",
        command: "buf posts list [--channel <id>] [--service <service>] [--status draft|needs_approval|scheduled|sending|sent|error] [--from <iso>] [--to <iso>] [--limit <n>] [--cursor <cursor>]",
        category: "posts",
        description: "List posts for the resolved organization with cursor pagination. `--from` and `--to` currently map to Buffer's `dueAt` comparator. Returned post objects include publishedUrl as a non-breaking alias for Buffer externalLink.",
        parameters: vec![
            parameter("--channel", "string", false, "Optional Buffer channel id."),
            parameter(
                "--service",
                "string",
                false,
                "Optional service filter resolved to matching channel ids before querying Buffer.",
            ),
            parameter(
                "--status",
                "string",
                false,
                "Optional post status filter: draft, needs_approval, scheduled, sending, sent, or error.",
            ),
            parameter(
                "--from",
                "string",
                false,
                "Optional RFC 3339 timestamp lower bound.",
            ),
            parameter(
                "--to",
                "string",
                false,
                "Optional RFC 3339 timestamp upper bound.",
            ),
            parameter(
                "--limit",
                "integer",
                false,
                "Maximum posts to request from Buffer.",
            ),
            parameter(
                "--cursor",
                "string",
                false,
                "Opaque pagination cursor from a prior response.",
            ),
        ],
        output_fields: vec!["posts", "posts[].publishedUrl", "pageInfo", "query"],
        output_schema: json!({
            "type": "object",
            "required": ["posts", "pageInfo", "query"],
            "properties": {
                "posts": { "type": "array" },
                "pageInfo": { "type": "object" },
                "query": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "channel": { "type": "string" },
                "service": { "type": "string", "enum": DOCUMENTED_SERVICES },
                "status": {
                    "type": "string",
                    "enum": ["draft", "needs_approval", "scheduled", "sending", "sent", "error"]
                },
                "from": { "type": "string" },
                "to": { "type": "string" },
                "limit": { "type": "integer", "minimum": 1 },
                "cursor": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: Some("Buffer API limits apply; avoid aggressive polling."),
        example: "buf posts list --status sent --service linkedin --limit 10",
    }
}

fn posts_get_tool() -> ToolMetadata {
    ToolMetadata {
        name: "posts.get",
        command: "buf posts get <post-id>",
        category: "posts",
        description: "Fetch one Buffer post by id. Returned post includes publishedUrl as a non-breaking alias for Buffer externalLink.",
        parameters: vec![parameter("post-id", "string", true, "Buffer post id.")],
        output_fields: vec!["post", "post.publishedUrl"],
        output_schema: json!({
            "type": "object",
            "required": ["post"],
            "properties": {
                "post": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "required": ["postId"],
            "properties": {
                "postId": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: Some("Buffer API limits apply; avoid tight polling loops."),
        example: "buf posts get post_123",
    }
}

fn posts_create_tool() -> ToolMetadata {
    ToolMetadata {
        name: "posts.create",
        command: "buf posts create --channel <channel-id> [--body <text> | --body-file <path> | --stdin] [--target draft|schedule|queue|next|now|recommended] [--at <iso>] [--delivery automatic|notification] [--type post|carousel|story|reel] [--media <path-or-url> ...] [--first-comment <text>] [--link-url <url>] [--share-to-feed] [--meta-json <json-string>] [--dry-run]",
        category: "posts",
        description: "Create a draft, scheduled post, queued post, or immediate post through Buffer with one unified media input surface. Returned post includes publishedUrl as a non-breaking alias for Buffer externalLink when available.",
        parameters: vec![
            parameter("--channel", "string", true, "Buffer channel id."),
            parameter("--body", "string", false, "Inline post body text."),
            parameter(
                "--body-file",
                "path",
                false,
                "Path to a text file whose contents become the post body.",
            ),
            parameter(
                "--stdin",
                "boolean",
                false,
                "Read the post body from stdin.",
            ),
            parameter(
                "--target",
                "string",
                false,
                "One of draft, schedule, queue, next, now, or recommended.",
            ),
            parameter(
                "--at",
                "string",
                false,
                "RFC 3339 timestamp required with --target schedule.",
            ),
            parameter(
                "--delivery",
                "string",
                false,
                "One of automatic or notification.",
            ),
            parameter(
                "--type",
                "string",
                false,
                "Instagram post type: post, carousel, story, or reel.",
            ),
            parameter(
                "--media",
                "array",
                false,
                "Repeatable local path or public URL. Local files are normalized and staged to R2 automatically.",
            ),
            parameter(
                "--first-comment",
                "string",
                false,
                "Instagram or LinkedIn first comment.",
            ),
            parameter(
                "--link-url",
                "string",
                false,
                "Instagram Shop Grid link or LinkedIn/Threads link attachment URL.",
            ),
            parameter(
                "--share-to-feed",
                "boolean",
                false,
                "Instagram-only flag to share reel media to the main feed.",
            ),
            parameter(
                "--meta-json",
                "string",
                false,
                "JSON object string merged into Instagram, LinkedIn, or Threads metadata.",
            ),
            parameter(
                "--dry-run",
                "boolean",
                false,
                "Return the normalized Buffer input without creating a post.",
            ),
        ],
        output_fields: vec![
            "dryRun",
            "channel",
            "request",
            "stagedMedia",
            "post",
            "post.publishedUrl",
        ],
        output_schema: json!({
            "type": "object",
            "properties": {
                "dryRun": { "type": "boolean" },
                "channel": { "type": "object" },
                "request": { "type": "object" },
                "stagedMedia": { "type": "object" },
                "post": { "type": "object" }
            }
        }),
        input_schema: json!({
            "type": "object",
            "required": ["channel"],
            "properties": {
                "channel": { "type": "string" },
                "body": { "type": "string" },
                "bodyFile": { "type": "string" },
                "stdin": { "type": "boolean" },
                "target": { "type": "string", "enum": ["draft", "schedule", "queue", "next", "now", "recommended"] },
                "at": { "type": "string" },
                "delivery": { "type": "string", "enum": ["automatic", "notification"] },
                "type": { "type": "string", "enum": ["post", "carousel", "story", "reel"] },
                "media": { "type": "array", "items": { "type": "string" } },
                "firstComment": { "type": "string" },
                "linkUrl": { "type": "string" },
                "shareToFeed": { "type": "boolean" },
                "metaJson": { "type": "string" },
                "dryRun": { "type": "boolean" }
            },
            "additionalProperties": false
        }),
        idempotent: false,
        rate_limit: Some("Buffer API limits apply; prefer drafts first for new automation."),
        example: "buf posts create --channel ch_123 --body-file ./post.md --media ./asset.jpg --target draft",
    }
}

fn posts_delete_tool() -> ToolMetadata {
    ToolMetadata {
        name: "posts.delete",
        command: "buf posts delete <post-id>",
        category: "posts",
        description: "Delete one Buffer post by id.",
        parameters: vec![parameter("post-id", "string", true, "Buffer post id.")],
        output_fields: vec!["deleted", "postId"],
        output_schema: json!({
            "type": "object",
            "required": ["deleted", "postId"],
            "properties": {
                "deleted": { "type": "boolean" },
                "postId": { "type": "string" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "required": ["postId"],
            "properties": {
                "postId": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: false,
        rate_limit: Some(
            "Buffer API limits apply; prefer explicit operator intent before destructive actions.",
        ),
        example: "buf posts delete post_123",
    }
}

fn posts_limits_tool() -> ToolMetadata {
    ToolMetadata {
        name: "posts.limits",
        command: "buf posts limits [--channel <id> ...] [--service <service>] [--date <iso>]",
        category: "posts",
        description: "Return Buffer daily posting limit status for explicit channel ids or all channels matching one service.",
        parameters: vec![
            parameter(
                "--channel",
                "array",
                false,
                "Repeatable Buffer channel id. Use one or more ids, or use --service to resolve matching channels automatically.",
            ),
            parameter(
                "--service",
                "string",
                false,
                "Optional Buffer service value used to resolve matching channels before querying daily limits.",
            ),
            parameter(
                "--date",
                "string",
                false,
                "Optional RFC 3339 timestamp for the date to inspect. Defaults to Buffer's current date if omitted.",
            ),
        ],
        output_fields: vec!["limits", "query"],
        output_schema: json!({
            "type": "object",
            "required": ["limits", "query"],
            "properties": {
                "limits": { "type": "array" },
                "query": { "type": "object" }
            },
            "additionalProperties": false
        }),
        input_schema: json!({
            "type": "object",
            "properties": {
                "channelIds": { "type": "array", "items": { "type": "string" } },
                "service": { "type": "string", "enum": DOCUMENTED_SERVICES },
                "date": { "type": "string" }
            },
            "additionalProperties": false
        }),
        idempotent: true,
        rate_limit: Some(
            "Buffer API limits apply; cache daily limit reads when polling multiple channels.",
        ),
        example: "buf posts limits --service instagram",
    }
}

fn parameter(
    name: &'static str,
    param_type: &'static str,
    required: bool,
    description: &'static str,
) -> ToolParameter {
    ToolParameter {
        name,
        param_type,
        required,
        description,
    }
}
