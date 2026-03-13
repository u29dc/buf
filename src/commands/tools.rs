use serde::Serialize;
use serde_json::json;

use crate::commands::{CommandOutput, CommandResult};
use crate::error::CommandError;
use crate::tool_registry::{GlobalFlag, ToolMetadata, find_tool, global_flags, tool_registry};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolsCatalog<'a> {
    version: &'static str,
    global_flags: &'a [GlobalFlag],
    tools: &'a [ToolMetadata],
}

pub fn run(name: Option<&str>) -> CommandResult {
    match name {
        Some(tool_name) => detail(tool_name),
        None => catalog(),
    }
}

fn catalog() -> CommandResult {
    let payload = ToolsCatalog {
        version: env!("CARGO_PKG_VERSION"),
        global_flags: global_flags(),
        tools: tool_registry(),
    };
    let count = payload.tools.len();
    let data = serde_json::to_value(payload).expect("tools catalog serialization failed");

    Ok(CommandOutput::new("tools", data)
        .with_count(count)
        .with_total(count)
        .with_has_more(false)
        .with_text(format!("{count} tools available")))
}

fn detail(name: &str) -> CommandResult {
    let Some(tool) = find_tool(name) else {
        return Err(CommandError::failure(
            "NOT_FOUND",
            format!("tool `{name}` was not found"),
            "Run `buf tools` to inspect valid tool names",
        ));
    };

    Ok(CommandOutput::new("tools", json!({ "tool": tool }))
        .with_count(1)
        .with_total(1)
        .with_has_more(false)
        .with_text(format!("{} -> {}", tool.name, tool.command)))
}
