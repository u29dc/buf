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

use clap::{CommandFactory, Parser};

use crate::cli::{Cli, Command, ConfigCommand};
use crate::commands::GlobalOptions;
use crate::envelope::{OutputMode, emit_error, emit_success, print_text_error};

fn main() -> ExitCode {
    let cli = Cli::parse();
    if cli.command.is_none() {
        print_root_help();
        return ExitCode::SUCCESS;
    }

    let mode = if cli.text {
        OutputMode::Text
    } else {
        OutputMode::Json
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

    match execute(command, &options) {
        Ok(output) => {
            match mode {
                OutputMode::Json => {
                    emit_success(
                        output.tool,
                        &output.data,
                        start.elapsed().as_millis(),
                        &output.meta,
                    );
                }
                OutputMode::Text => {
                    if !output.text.trim().is_empty() {
                        println!("{}", output.text);
                    }
                }
            }
            ExitCode::from(output.exit_status.code())
        }
        Err(error) => {
            match mode {
                OutputMode::Json => emit_error(tool_name, &error, start.elapsed().as_millis()),
                OutputMode::Text => print_text_error(tool_name, &error),
            }
            ExitCode::from(error.exit_status().code())
        }
    }
}

fn execute(command: Command, options: &GlobalOptions) -> commands::CommandResult {
    match command {
        Command::Tools(args) => commands::tools::run(args.name.as_deref()),
        Command::Health => commands::health::run(options),
        Command::Config(args) => match args.command {
            ConfigCommand::Show => commands::config::show(options),
            ConfigCommand::Validate => commands::config::validate(options),
        },
        Command::Channels(args) => match args.command {
            crate::cli::ChannelsCommand::List(list_args) => {
                commands::channels::list(options, &list_args)
            }
            crate::cli::ChannelsCommand::Resolve(resolve_args) => {
                commands::channels::resolve(options, &resolve_args)
            }
        },
        Command::Posts(args) => match args.command {
            crate::cli::PostsCommand::List(list_args) => commands::posts::list(options, &list_args),
            crate::cli::PostsCommand::Get(get_args) => commands::posts::get(options, &get_args),
            crate::cli::PostsCommand::Create(create_args) => {
                commands::posts::create(options, &create_args)
            }
        },
    }
}

fn print_root_help() {
    let mut command = Cli::command().subcommand_required(true);
    command.print_help().expect("print root help");
    println!();
}

fn infer_tool_name(cli: &Cli) -> &'static str {
    match cli.command.as_ref() {
        Some(Command::Tools(_)) => "tools",
        Some(Command::Health) => "health",
        Some(Command::Config(args)) => match &args.command {
            ConfigCommand::Show => "config.show",
            ConfigCommand::Validate => "config.validate",
        },
        Some(Command::Channels(args)) => match &args.command {
            crate::cli::ChannelsCommand::List(_) => "channels.list",
            crate::cli::ChannelsCommand::Resolve(_) => "channels.resolve",
        },
        Some(Command::Posts(args)) => match &args.command {
            crate::cli::PostsCommand::List(_) => "posts.list",
            crate::cli::PostsCommand::Get(_) => "posts.get",
            crate::cli::PostsCommand::Create(_) => "posts.create",
        },
        None => "buf",
    }
}
