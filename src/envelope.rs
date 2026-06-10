use serde::Serialize;
use serde_json::Value;

use crate::buffer_api::BufferWarning;
use crate::commands::CommandMeta;
use crate::error::CommandError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Json,
    Toon,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    warnings: Option<Vec<BufferWarning>>,
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

pub fn emit_success(
    tool: &str,
    data: &Value,
    elapsed: u128,
    meta: &CommandMeta,
    format: OutputFormat,
) {
    let envelope = SuccessEnvelope {
        ok: true,
        data,
        meta: EnvelopeMeta {
            tool: tool.to_owned(),
            elapsed: clamp_elapsed(elapsed),
            count: meta.count,
            total: meta.total,
            has_more: meta.has_more,
            warnings: meta.warnings.clone(),
        },
    };

    emit_envelope(&envelope, format, tool, elapsed);
}

pub fn emit_error(tool: &str, error: &CommandError, elapsed: u128, format: OutputFormat) {
    let code = normalize_error_code(error.code());
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: &code,
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
            warnings: None,
        },
    };

    emit_envelope(&envelope, format, tool, elapsed);
}

fn emit_envelope<T: Serialize>(envelope: &T, format: OutputFormat, tool: &str, elapsed: u128) {
    let rendered = match format {
        OutputFormat::Json => serde_json::to_string(envelope).map_err(|err| err.to_string()),
        OutputFormat::Toon => toon_format::encode_default(envelope).map_err(|err| err.to_string()),
    };

    match rendered {
        Ok(payload) => println!("{payload}"),
        Err(_) => emit_serialization_error(tool, elapsed, format),
    }
}

fn emit_serialization_error(tool: &str, elapsed: u128, format: OutputFormat) {
    let envelope = ErrorEnvelope {
        ok: false,
        error: ErrorBody {
            code: "serialization_error",
            message: "failed to serialize envelope",
            hint: "Retry the command after reducing output size",
            details: None,
        },
        meta: EnvelopeMeta {
            tool: tool.to_owned(),
            elapsed: clamp_elapsed(elapsed),
            count: None,
            total: None,
            has_more: None,
            warnings: None,
        },
    };

    let rendered = match format {
        OutputFormat::Json => serde_json::to_string(&envelope).ok(),
        OutputFormat::Toon => toon_format::encode_default(&envelope).ok(),
    };

    if let Some(payload) = rendered {
        println!("{payload}");
    } else {
        println!(
            "{{\"ok\":false,\"error\":{{\"code\":\"serialization_error\",\"message\":\"failed to serialize error envelope\",\"hint\":\"Retry the command after reducing output size\"}},\"meta\":{{\"tool\":\"{tool}\",\"elapsed\":{}}}}}",
            clamp_elapsed(elapsed)
        );
    }
}

fn clamp_elapsed(value: u128) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn normalize_error_code(code: &str) -> String {
    let mut normalized = String::with_capacity(code.len());
    let mut previous_was_underscore = false;

    for character in code.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_underscore = false;
        } else if !previous_was_underscore {
            normalized.push('_');
            previous_was_underscore = true;
        }
    }

    normalized.trim_matches('_').to_owned()
}
