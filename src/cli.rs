use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "buf", version, about = "JSON-first Buffer CLI for agents")]
pub struct Cli {
    #[arg(long, global = true, help = "Emit human-readable text")]
    pub text: bool,

    #[arg(long, global = true, value_name = "PATH", help = "Override BUF_HOME")]
    pub home: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "PATH",
        help = "Override buf.config.toml path"
    )]
    pub config_file: Option<PathBuf>,

    #[arg(long, global = true, value_name = "PATH", help = "Override .env path")]
    pub env_file: Option<PathBuf>,

    #[arg(
        long,
        global = true,
        value_name = "URL",
        help = "Override Buffer API base URL"
    )]
    pub api_base_url: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Tools(ToolsArgs),
    Health,
    Config(ConfigArgs),
    Channels(ChannelsArgs),
    Posts(PostsArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ToolsArgs {
    #[arg(value_name = "NAME", help = "Optional dotted tool name")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ConfigCommand {
    Show,
    Validate,
}

#[derive(Debug, Clone, Args)]
pub struct ChannelsArgs {
    #[command(subcommand)]
    pub command: ChannelsCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum ChannelsCommand {
    List(ChannelsListArgs),
    Resolve(ChannelsResolveArgs),
}

#[derive(Debug, Clone, Args)]
pub struct ChannelsListArgs {
    #[arg(long)]
    pub service: Option<ChannelService>,

    #[arg(long)]
    pub query: Option<String>,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,
}

#[derive(Debug, Clone, Args)]
pub struct ChannelsResolveArgs {
    #[arg(long)]
    pub service: ChannelService,

    #[arg(long)]
    pub query: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct PostsArgs {
    #[command(subcommand)]
    pub command: PostsCommand,
}

#[derive(Debug, Clone, Subcommand)]
pub enum PostsCommand {
    List(PostsListArgs),
    Get(PostsGetArgs),
    Create(PostsCreateArgs),
}

#[derive(Debug, Clone, Args)]
pub struct PostsListArgs {
    #[arg(long)]
    pub channel: Option<String>,

    #[arg(long)]
    pub service: Option<ChannelService>,

    #[arg(long)]
    pub status: Option<PostStatus>,

    #[arg(long, value_name = "ISO-8601")]
    pub from: Option<String>,

    #[arg(long, value_name = "ISO-8601")]
    pub to: Option<String>,

    #[arg(long, default_value_t = 20)]
    pub limit: usize,

    #[arg(long)]
    pub cursor: Option<String>,
}

#[derive(Debug, Clone, Args)]
pub struct PostsGetArgs {
    #[arg(value_name = "POST_ID")]
    pub post_id: String,
}

#[derive(Debug, Clone, Args)]
pub struct PostsCreateArgs {
    #[arg(long)]
    pub channel: String,

    #[arg(long)]
    pub body: Option<String>,

    #[arg(long = "body-file", value_name = "PATH")]
    pub body_file: Option<PathBuf>,

    #[arg(long, default_value_t = false)]
    pub stdin: bool,

    #[arg(long, default_value = "draft")]
    pub target: CreateTarget,

    #[arg(long, value_name = "ISO-8601")]
    pub at: Option<String>,

    #[arg(long, default_value = "automatic")]
    pub delivery: DeliveryMode,

    #[arg(long = "type")]
    pub post_type: Option<InstagramPostType>,

    #[arg(long = "media", value_name = "PATH_OR_URL")]
    pub media: Vec<String>,

    #[arg(long)]
    pub first_comment: Option<String>,

    #[arg(long)]
    pub link_url: Option<String>,

    #[arg(long, default_value_t = false)]
    pub share_to_feed: bool,

    #[arg(long)]
    pub meta_json: Option<String>,

    #[arg(long, default_value_t = false)]
    pub dry_run: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ChannelService {
    Instagram,
    #[value(name = "linkedin", alias = "linked-in")]
    LinkedIn,
    Threads,
}

impl ChannelService {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Instagram => "instagram",
            Self::LinkedIn => "linkedin",
            Self::Threads => "threads",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum PostStatus {
    Draft,
    Scheduled,
    Sent,
    Error,
    Sending,
    #[value(name = "needs_approval", alias = "needs-approval")]
    NeedsApproval,
}

impl PostStatus {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Scheduled => "scheduled",
            Self::Sent => "sent",
            Self::Error => "error",
            Self::Sending => "sending",
            Self::NeedsApproval => "needs_approval",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum CreateTarget {
    #[default]
    Draft,
    Schedule,
    Queue,
    Next,
    Now,
}

impl CreateTarget {
    #[must_use]
    pub const fn share_mode(self) -> &'static str {
        match self {
            Self::Draft => "addToQueue",
            Self::Schedule => "customScheduled",
            Self::Queue => "addToQueue",
            Self::Next => "shareNext",
            Self::Now => "shareNow",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum DeliveryMode {
    #[default]
    Automatic,
    Notification,
}

impl DeliveryMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::Notification => "notification",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum InstagramPostType {
    Post,
    Carousel,
    Story,
    Reel,
}

impl InstagramPostType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Post => "post",
            Self::Carousel => "carousel",
            Self::Story => "story",
            Self::Reel => "reel",
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{
        ChannelService, ChannelsCommand, Cli, Command, PostStatus, PostsArgs, PostsCommand,
    };

    #[test]
    fn linkedin_service_parses_from_documented_value() {
        let cli = Cli::try_parse_from(["buf", "channels", "list", "--service", "linkedin"])
            .expect("parse linkedin service");
        match cli.command.expect("subcommand") {
            Command::Channels(args) => match args.command {
                ChannelsCommand::List(list) => {
                    assert_eq!(list.service.expect("service").as_str(), "linkedin");
                }
                _ => panic!("expected channels list"),
            },
            _ => panic!("expected channels command"),
        }
    }

    #[test]
    fn threads_service_parses_from_documented_value() {
        let cli = Cli::try_parse_from(["buf", "channels", "resolve", "--service", "threads"])
            .expect("parse threads service");
        match cli.command.expect("subcommand") {
            Command::Channels(args) => match args.command {
                ChannelsCommand::Resolve(resolve) => {
                    assert_eq!(resolve.service, ChannelService::Threads);
                    assert_eq!(resolve.service.as_str(), "threads");
                }
                _ => panic!("expected channels resolve"),
            },
            _ => panic!("expected channels command"),
        }
    }

    #[test]
    fn needs_approval_status_parses_from_documented_value() {
        let cli = Cli::try_parse_from(["buf", "posts", "list", "--status", "needs_approval"])
            .expect("parse needs_approval status");
        match cli.command.expect("subcommand") {
            Command::Posts(PostsArgs {
                command: PostsCommand::List(list),
            }) => {
                assert_eq!(list.status.expect("status"), PostStatus::NeedsApproval);
            }
            _ => panic!("expected posts list"),
        }
    }
}
