use serde::Serialize;
use serde_json::Value;

use crate::commands::CommandMeta;
use crate::error::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    Json,
    Text,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct EnvelopeMeta {
    tool: String,
    elapsed: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_more: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SuccessEnvelope<'a> {
    ok: bool,
    data: &'a Value,
    meta: EnvelopeMeta,
}

#[derive(Debug, Serialize)]
struct ErrorBody<'a> {
    code: &'a str,
    message: &'a str,
    hint: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<&'a Value>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope<'a> {
    ok: bool,
    error: ErrorBody<'a>,
    meta: EnvelopeMeta,
}

pub fn emit_success(tool: &str, data: &Value, elapsed: u128, meta: &CommandMeta) {
    let envelope = SuccessEnvelope {
        ok: true,
        data,
        meta: EnvelopeMeta {
            tool: tool.to_owned(),
            elapsed: clamp_elapsed(elapsed),
            count: meta.count,
            total: meta.total,
            has_more: meta.has_more,
        },
    };

    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"SERIALIZATION_ERROR\",\"message\":\"failed to serialize success envelope\",\"hint\":\"Retry the command after reducing output size\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":0}}}}"
        ),
    }
}

pub fn emit_error(tool: &str, error: &CommandError, elapsed: u128) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: error.code(),
            message: error.message(),
            hint: error.hint(),
            details: error.details(),
        },
        meta: EnvelopeMeta {
            tool: tool.to_owned(),
            elapsed: clamp_elapsed(elapsed),
            count: None,
            total: None,
            has_more: None,
        },
    };

    match serde_json::to_string(&envelope) {
        Ok(payload) => println!("{payload}"),
        Err(_) => println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"SERIALIZATION_ERROR\",\"message\":\"failed to serialize error envelope\",\"hint\":\"Retry the command after reducing output size\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":0}}}}"
        ),
    }
}

pub fn print_text_error(tool: &str, error: &CommandError) {
    eprintln!("ERROR [{tool}] {}: {}", error.code(), error.message());
    eprintln!("HINT  {}", error.hint());
    if let Some(details) = error.details() {
        eprintln!("DETAILS {details}");
    }
}

fn clamp_elapsed(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}
