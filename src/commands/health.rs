use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

use serde::Serialize;
use serde_json::{Value, json};

use crate::commands::{CommandOutput, CommandResult, GlobalOptions, build_client};
use crate::config::load_runtime;
use crate::error::ProcessExit;

#[derive(Debug, Clone, Serialize)]
struct HealthCheck {
    id: String,
    label: String,
    status: String,
    severity: String,
    detail: String,
    fix: Value,
}

#[derive(Debug, Clone, Default)]
struct Summary {
    ok: usize,
    degraded: usize,
    blocking: usize,
}

pub fn run(options: &GlobalOptions) -> CommandResult {
    let runtime = load_runtime(&options.path_overrides(), options.api_base_url.as_deref())?;
    let mut checks = vec![
        check_writable_dir("dir.home", "Directory: home", &runtime.paths.home),
        check_writable_dir("dir.temp", "Directory: temp", &runtime.paths.temp_dir),
        check_presence(
            "config",
            "Configuration File",
            &runtime.paths.config_file,
            runtime.file_exists,
        ),
        check_presence(
            "env",
            "Environment File",
            &runtime.paths.env_file,
            runtime.env_exists,
        ),
        check_token(runtime.settings.token.present, &runtime.paths.env_file),
        check_media_config(runtime.settings.media.ready, &runtime.paths.env_file),
        check_binary("ffmpeg", "Media Normalizer: ffmpeg"),
        check_binary("ffprobe", "Media Normalizer: ffprobe"),
    ];

    if runtime.settings.token.present {
        match build_client(&runtime).and_then(|client| client.list_organizations()) {
            Ok(response) => {
                let organizations = response.data;
                let needs_default =
                    runtime.settings.default_organization_id.is_none() && organizations.len() > 1;
                checks.push(HealthCheck {
                    id: "api.auth".to_owned(),
                    label: "Buffer API Authentication".to_owned(),
                    status: if needs_default { "warning" } else { "ok" }.to_owned(),
                    severity: if needs_default { "degraded" } else { "info" }.to_owned(),
                    detail: if needs_default {
                        format!(
                            "{} organizations visible; set BUF_ORGANIZATION_ID to remove ambiguity",
                            organizations.len()
                        )
                    } else {
                        format!("{} organization(s) visible", organizations.len())
                    },
                    fix: if needs_default {
                        json!(["Set BUF_ORGANIZATION_ID or keep the discovered default in buf.config.toml"])
                    } else {
                        Value::Null
                    },
                });
                if !response.warnings.is_empty() {
                    checks.push(HealthCheck {
                        id: "api.warnings".to_owned(),
                        label: "Buffer API Warnings".to_owned(),
                        status: "warning".to_owned(),
                        severity: "degraded".to_owned(),
                        detail: format!(
                            "{} upstream warning(s) returned with successful API responses",
                            response.warnings.len()
                        ),
                        fix: json!(
                            response
                                .warnings
                                .into_iter()
                                .map(|warning| match warning.code {
                                    Some(code) => format!("{code}: {}", warning.message),
                                    None => warning.message,
                                })
                                .collect::<Vec<_>>()
                        ),
                    });
                }
            }
            Err(error) => checks.push(HealthCheck {
                id: "api.auth".to_owned(),
                label: "Buffer API Authentication".to_owned(),
                status: "error".to_owned(),
                severity: if error.exit_status() == ProcessExit::Blocked {
                    "blocking".to_owned()
                } else {
                    "degraded".to_owned()
                },
                detail: error.message().to_owned(),
                fix: json!([error.hint()]),
            }),
        }
    } else {
        checks.push(HealthCheck {
            id: "api.auth".to_owned(),
            label: "Buffer API Authentication".to_owned(),
            status: "skipped".to_owned(),
            severity: "info".to_owned(),
            detail: "skipped because BUF_API_TOKEN is missing".to_owned(),
            fix: Value::Null,
        });
    }

    let summary = summarize(&checks);
    let status = if summary.blocking > 0 {
        "blocked"
    } else if summary.degraded > 0 {
        "degraded"
    } else {
        "ready"
    };
    let exit_status = if status == "blocked" {
        ProcessExit::Blocked
    } else {
        ProcessExit::Success
    };
    let count = checks.len();

    Ok(CommandOutput::new(
        "health",
        json!({
            "status": status,
            "paths": {
                "home": runtime.paths.home.display().to_string(),
                "config": runtime.paths.config_file.display().to_string(),
                "env": runtime.paths.env_file.display().to_string(),
                "temp": runtime.paths.temp_dir.display().to_string(),
            },
            "checks": checks,
            "summary": {
                "ok": summary.ok,
                "degraded": summary.degraded,
                "blocking": summary.blocking,
            }
        }),
    )
    .with_count(count)
    .with_total(count)
    .with_has_more(false)
    .with_text(format!("health status: {status}"))
    .with_exit_status(exit_status))
}

fn check_writable_dir(id: &str, label: &str, path: &Path) -> HealthCheck {
    match fs::create_dir_all(path).and_then(|()| probe_write(path)) {
        Ok(()) => HealthCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            status: "ok".to_owned(),
            severity: "info".to_owned(),
            detail: path.display().to_string(),
            fix: Value::Null,
        },
        Err(error) => HealthCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            status: "error".to_owned(),
            severity: "blocking".to_owned(),
            detail: format!("{} ({error})", path.display()),
            fix: json!([format!("ensure {} exists and is writable", path.display())]),
        },
    }
}

fn check_presence(id: &str, label: &str, path: &Path, exists: bool) -> HealthCheck {
    if exists {
        HealthCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            status: "ok".to_owned(),
            severity: "info".to_owned(),
            detail: path.display().to_string(),
            fix: Value::Null,
        }
    } else {
        HealthCheck {
            id: id.to_owned(),
            label: label.to_owned(),
            status: "missing".to_owned(),
            severity: "info".to_owned(),
            detail: format!("missing {}", path.display()),
            fix: Value::Null,
        }
    }
}

fn check_token(present: bool, env_file: &Path) -> HealthCheck {
    if present {
        HealthCheck {
            id: "auth.token".to_owned(),
            label: "API Token".to_owned(),
            status: "ok".to_owned(),
            severity: "info".to_owned(),
            detail: "BUF_API_TOKEN detected".to_owned(),
            fix: Value::Null,
        }
    } else {
        HealthCheck {
            id: "auth.token".to_owned(),
            label: "API Token".to_owned(),
            status: "missing".to_owned(),
            severity: "blocking".to_owned(),
            detail: "BUF_API_TOKEN is not configured".to_owned(),
            fix: json!([
                "export BUF_API_TOKEN=your-token",
                format!("or add BUF_API_TOKEN to {}", env_file.display())
            ]),
        }
    }
}

fn check_media_config(ready: bool, env_file: &Path) -> HealthCheck {
    if ready {
        HealthCheck {
            id: "storage.r2".to_owned(),
            label: "Media Storage: Cloudflare R2".to_owned(),
            status: "ok".to_owned(),
            severity: "info".to_owned(),
            detail: "R2 credentials and public base URL detected".to_owned(),
            fix: Value::Null,
        }
    } else {
        HealthCheck {
            id: "storage.r2".to_owned(),
            label: "Media Storage: Cloudflare R2".to_owned(),
            status: "missing".to_owned(),
            severity: "degraded".to_owned(),
            detail: "local media staging is unavailable until BUF_MEDIA_* settings are configured"
                .to_owned(),
            fix: json!([format!(
                "Add BUF_MEDIA_ENDPOINT, BUF_MEDIA_BUCKET, BUF_MEDIA_ACCESS_KEY_ID, BUF_MEDIA_SECRET_ACCESS_KEY, and BUF_MEDIA_BASE_URL to {}",
                env_file.display()
            )]),
        }
    }
}

fn check_binary(binary: &str, label: &str) -> HealthCheck {
    match Command::new(binary).arg("-version").output() {
        Ok(output) if output.status.success() => HealthCheck {
            id: format!("binary.{binary}"),
            label: label.to_owned(),
            status: "ok".to_owned(),
            severity: "info".to_owned(),
            detail: format!("{binary} available"),
            fix: Value::Null,
        },
        Ok(_) | Err(_) => HealthCheck {
            id: format!("binary.{binary}"),
            label: label.to_owned(),
            status: "missing".to_owned(),
            severity: "degraded".to_owned(),
            detail: format!("{binary} is not available on PATH"),
            fix: json!([format!(
                "Install {binary} and ensure it is available on PATH"
            )]),
        },
    }
}

fn probe_write(path: &Path) -> std::io::Result<()> {
    let probe = path.join(".buf-healthcheck.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&probe)?;
    file.write_all(b"ok")?;
    drop(file);
    let _ = fs::remove_file(probe);
    Ok(())
}

fn summarize(checks: &[HealthCheck]) -> Summary {
    let mut summary = Summary::default();
    for check in checks {
        match check.severity.as_str() {
            "blocking" => summary.blocking += 1,
            "degraded" => summary.degraded += 1,
            _ => summary.ok += 1,
        }
    }
    summary
}
