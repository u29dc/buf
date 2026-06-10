#![forbid(unsafe_code)]

mod buffer_api;
mod cli;
mod commands;
mod config;
mod env;
mod envelope;
mod error;
mod media;
mod storage;
mod tool_registry;

use std::process::ExitCode;
use std::time::Instant;

use clap::Command as ClapCommand;
use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Command, ConfigCommand};
use crate::commands::GlobalOptions;
use crate::envelope::{OutputFormat, emit_error, emit_success};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.command.is_none() {
        print_root_help();
        return ExitCode::SUCCESS;
    }

    let format = if cli.toon {
        OutputFormat::Toon
    } else {
        OutputFormat::Json
    };
    let options = GlobalOptions {
        home: cli.home.clone(),
        config_file: cli.config_file.clone(),
        env_file: cli.env_file.clone(),
        api_base_url: cli.api_base_url.clone(),
    };
    let start = Instant::now();
    let tool_name = infer_tool_name(&cli);
    let command = cli.command.expect("command should exist");
    if print_group_help_if_missing_subcommand(&command) {
        return ExitCode::SUCCESS;
    }

    match execute(command, &options) {
        Ok(output) => {
            emit_success(
                output.tool,
                &output.data,
                start.elapsed().as_millis(),
                &output.meta,
                format,
            );
            ExitCode::from(output.exit_status.code())
        }
        Err(error) => {
            emit_error(tool_name, &error, start.elapsed().as_millis(), format);
            ExitCode::from(error.exit_status().code())
        }
    }
}

fn execute(command: Command, options: &GlobalOptions) -> commands::CommandResult {
    match command {
        Command::Tools(args) => commands::tools::run(args.name.as_deref()),
        Command::Health => commands::health::run(options),
        Command::Config(args) => match args.command {
            Some(ConfigCommand::Show) => commands::config::show(options),
            Some(ConfigCommand::Validate) => commands::config::validate(options),
            None => unreachable!("config help should be printed before dispatch"),
        },
        Command::Channels(args) => match args.command {
            Some(crate::cli::ChannelsCommand::List(list_args)) => {
                commands::channels::list(options, &list_args)
            }
            Some(crate::cli::ChannelsCommand::Resolve(resolve_args)) => {
                commands::channels::resolve(options, &resolve_args)
            }
            None => unreachable!("channels help should be printed before dispatch"),
        },
        Command::Posts(args) => match args.command {
            Some(crate::cli::PostsCommand::List(list_args)) => {
                commands::posts::list(options, &list_args)
            }
            Some(crate::cli::PostsCommand::Get(get_args)) => {
                commands::posts::get(options, &get_args)
            }
            Some(crate::cli::PostsCommand::Create(create_args)) => {
                commands::posts::create(options, &create_args)
            }
            Some(crate::cli::PostsCommand::Delete(delete_args)) => {
                commands::posts::delete(options, &delete_args)
            }
            Some(crate::cli::PostsCommand::Limits(limits_args)) => {
                commands::posts::limits(options, &limits_args)
            }
            None => unreachable!("posts help should be printed before dispatch"),
        },
    }
}

fn print_root_help() {
    let mut command = Cli::command().subcommand_required(true);
    command.print_help().expect("print root help");
    println!();
}

fn print_group_help_if_missing_subcommand(command: &Command) -> bool {
    let group_name = match command {
        Command::Config(args) if args.command.is_none() => "config",
        Command::Channels(args) if args.command.is_none() => "channels",
        Command::Posts(args) if args.command.is_none() => "posts",
        _ => return false,
    };

    let mut root = Cli::command();
    print_subcommand_help(&mut root, group_name);
    true
}

fn print_subcommand_help(root: &mut ClapCommand, group_name: &str) {
    let subcommand = root
        .find_subcommand_mut(group_name)
        .expect("group subcommand should exist");
    subcommand.print_help().expect("print group help");
    println!();
}

fn infer_tool_name(cli: &Cli) -> &'static str {
    match cli.command.as_ref() {
        Some(Command::Tools(_)) => "tools",
        Some(Command::Health) => "health",
        Some(Command::Config(args)) => match &args.command {
            Some(ConfigCommand::Show) => "config.show",
            Some(ConfigCommand::Validate) => "config.validate",
            None => "config",
        },
        Some(Command::Channels(args)) => match &args.command {
            Some(crate::cli::ChannelsCommand::List(_)) => "channels.list",
            Some(crate::cli::ChannelsCommand::Resolve(_)) => "channels.resolve",
            None => "channels",
        },
        Some(Command::Posts(args)) => match &args.command {
            Some(crate::cli::PostsCommand::List(_)) => "posts.list",
            Some(crate::cli::PostsCommand::Get(_)) => "posts.get",
            Some(crate::cli::PostsCommand::Create(_)) => "posts.create",
            Some(crate::cli::PostsCommand::Delete(_)) => "posts.delete",
            Some(crate::cli::PostsCommand::Limits(_)) => "posts.limits",
            None => "posts",
        },
        None => "buf",
    }
}
