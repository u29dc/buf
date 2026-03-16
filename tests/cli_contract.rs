use std::fs;
use std::path::Path;
use std::process::Command;

use assert_cmd::cargo::cargo_bin;
use serde_json::Value;
use tempfile::TempDir;
use wiremock::matchers::{body_string_contains, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn buf_command(home: &Path) -> Command {
    let mut command = Command::new(cargo_bin("buf"));
    command.args(["--home", home.to_str().expect("home path utf8")]);
    command
}

fn parse_single_json_line(output: &std::process::Output) -> Value {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout utf8");
    let trimmed = stdout.trim_end();
    assert!(!trimmed.is_empty(), "expected stdout output");
    assert_eq!(
        trimmed.lines().count(),
        1,
        "expected one JSON line on stdout, got: {stdout}"
    );
    serde_json::from_str(trimmed).expect("stdout json")
}

fn write_env(home: &Path, body: &str) {
    fs::write(home.join(".env"), body).expect("write env file");
}

fn write_sample_png(path: &Path) {
    let png = [
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 255, 255, 63, 0,
        5, 254, 2, 254, 167, 53, 129, 132, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fs::write(path, png).expect("write sample png");
}

fn sample_organization() -> Value {
    serde_json::json!({
        "id": "org_123",
        "name": "Example Org",
        "ownerEmail": "owner@example.com",
        "channelCount": 1,
        "limits": {
            "scheduledPosts": 100,
            "scheduledStoriesPerChannel": 10,
            "ideas": 10,
            "tags": 10
        }
    })
}

fn sample_channel(service: &str) -> Value {
    let (name, channel_type, external_link) = match service {
        "threads" => (
            "Example Threads",
            "creator",
            "https://example.com/threads/example-profile".to_owned(),
        ),
        "linkedin" => (
            "Example LinkedIn",
            "company",
            "https://example.com/linkedin/example-company".to_owned(),
        ),
        _ => (
            "Example Instagram",
            "business",
            "https://example.com/instagram/example-profile".to_owned(),
        ),
    };

    serde_json::json!({
        "id": "ch_123",
        "name": name,
        "displayName": name,
        "service": service,
        "type": channel_type,
        "organizationId": "org_123",
        "isLocked": false,
        "isDisconnected": false,
        "isQueuePaused": false,
        "timezone": "Europe/London",
        "products": ["publish"],
        "externalLink": external_link
    })
}

fn sample_post(service: &str) -> Value {
    serde_json::json!({
        "id": "post_123",
        "status": "scheduled",
        "via": null,
        "schedulingType": "automatic",
        "shareMode": "customScheduled",
        "createdAt": "2026-03-13T10:00:00Z",
        "updatedAt": "2026-03-13T10:00:00Z",
        "dueAt": "2026-03-26T10:28:47Z",
        "sentAt": null,
        "text": "Hello Buffer",
        "externalLink": null,
        "channelId": "ch_123",
        "channelService": service,
        "tags": [],
        "assets": [
            {
                "__typename": "ImageAsset",
                "thumbnail": "https://example.com/thumb.jpg",
                "mimeType": "image/jpeg",
                "source": "upload",
                "image": {
                    "altText": null,
                    "width": 1080,
                    "height": 1350
                }
            }
        ]
    })
}

#[test]
fn tools_catalog_is_json_first() {
    let home = TempDir::new().expect("temp dir");
    let output = buf_command(home.path())
        .arg("tools")
        .output()
        .expect("run tools");

    assert!(output.status.success(), "tools should succeed");
    assert!(
        output.stderr.is_empty(),
        "tools should not write to stderr in JSON mode"
    );

    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["meta"]["tool"], Value::String("tools".to_owned()));
    assert!(payload["data"]["globalFlags"].is_array());
    assert!(payload["data"]["tools"].is_array());

    let tools = payload["data"]["tools"].as_array().expect("tools array");
    assert!(tools.iter().any(|tool| tool["name"] == "channels.list"));
    assert!(tools.iter().any(|tool| tool["name"] == "posts.create"));

    let resolve_tool = tools
        .iter()
        .find(|tool| tool["name"] == "channels.resolve")
        .expect("channels.resolve tool");
    assert_eq!(
        resolve_tool["example"],
        Value::String("buf channels resolve --service linkedin --query example-company".to_owned())
    );
}

#[test]
fn health_reports_blocked_when_token_is_missing() {
    let home = TempDir::new().expect("temp dir");
    let output = buf_command(home.path())
        .arg("health")
        .output()
        .expect("run health");

    assert_eq!(output.status.code(), Some(2));
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["status"],
        Value::String("blocked".to_owned())
    );

    let token_check = payload["data"]["checks"]
        .as_array()
        .expect("checks array")
        .iter()
        .find(|check| check["id"] == "auth.token" || check["id"] == "token")
        .expect("token check");
    assert_eq!(
        token_check["severity"],
        Value::String("blocking".to_owned())
    );
}

#[tokio::test]
async fn channels_list_reads_mocked_buffer_channels() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    write_env(
        home.path(),
        "BUF_API_TOKEN=test-token\nBUF_API_BASE_URL=http://placeholder.invalid\n",
    );

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query AccountOrganizations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "account": {
                    "organizations": [sample_organization()]
                }
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query OrganizationChannels"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "channels": [
                    sample_channel("instagram")
                ]
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "channels",
            "list",
            "--service",
            "instagram",
        ])
        .output()
        .expect("run channels list");

    assert!(output.status.success(), "channels list should succeed");
    let payload = parse_single_json_line(&output);
    let channels = payload["data"]["channels"]
        .as_array()
        .expect("channels array");
    assert_eq!(channels.len(), 1);
    assert_eq!(channels[0]["id"], Value::String("ch_123".to_owned()));
}

#[tokio::test]
async fn posts_create_dry_run_returns_normalized_request() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    let image_path = home.path().join("sample.png");
    write_sample_png(&image_path);
    write_env(
        home.path(),
        concat!(
            "BUF_API_TOKEN=test-token\n",
            "BUF_MEDIA_ENDPOINT=https://example-account.r2.cloudflarestorage.com\n",
            "BUF_MEDIA_BUCKET=buffer\n",
            "BUF_MEDIA_ACCESS_KEY_ID=test-access\n",
            "BUF_MEDIA_SECRET_ACCESS_KEY=test-secret\n",
            "BUF_MEDIA_BASE_URL=https://example-bucket.r2.dev\n",
        ),
    );

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query ChannelById"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "channel": sample_channel("instagram")
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "posts",
            "create",
            "--channel",
            "ch_123",
            "--body",
            "Hello Buffer",
            "--target",
            "schedule",
            "--at",
            "2026-03-26T10:28:47Z",
            "--media",
            image_path.to_str().expect("image path utf8"),
            "--first-comment",
            "More in comments",
            "--dry-run",
        ])
        .output()
        .expect("run posts create dry-run");

    assert!(output.status.success(), "dry-run create should succeed");
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["data"]["dryRun"], Value::Bool(true));
    assert_eq!(
        payload["data"]["request"]["mode"],
        Value::String("customScheduled".to_owned())
    );
    assert_eq!(
        payload["data"]["request"]["metadata"]["instagram"]["firstComment"],
        Value::String("More in comments".to_owned())
    );
    let public_url = payload["data"]["stagedMedia"]["items"][0]["staged"]["publicUrl"]
        .as_str()
        .expect("dry-run public url");
    assert!(public_url.starts_with("https://example-bucket.r2.dev/tmp/buf/"));
    assert_eq!(
        payload["data"]["request"]["assets"]["images"][0]["url"],
        Value::String(public_url.to_owned())
    );
}

#[tokio::test]
async fn posts_create_threads_dry_run_returns_normalized_request() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    let image_path = home.path().join("sample.png");
    write_sample_png(&image_path);
    write_env(
        home.path(),
        concat!(
            "BUF_API_TOKEN=test-token\n",
            "BUF_MEDIA_ENDPOINT=https://example-account.r2.cloudflarestorage.com\n",
            "BUF_MEDIA_BUCKET=buffer\n",
            "BUF_MEDIA_ACCESS_KEY_ID=test-access\n",
            "BUF_MEDIA_SECRET_ACCESS_KEY=test-secret\n",
            "BUF_MEDIA_BASE_URL=https://example-bucket.r2.dev\n",
        ),
    );

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query ChannelById"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "channel": sample_channel("threads")
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "posts",
            "create",
            "--channel",
            "ch_123",
            "--body",
            "Hello Threads",
            "--target",
            "schedule",
            "--at",
            "2026-03-26T10:28:47Z",
            "--media",
            image_path.to_str().expect("image path utf8"),
            "--dry-run",
        ])
        .output()
        .expect("run posts create dry-run");

    assert!(output.status.success(), "dry-run create should succeed");
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["data"]["dryRun"], Value::Bool(true));
    assert_eq!(
        payload["data"]["channel"]["service"],
        Value::String("threads".to_owned())
    );
    assert_eq!(payload["data"]["request"]["metadata"], Value::Null);
    let public_url = payload["data"]["stagedMedia"]["items"][0]["staged"]["publicUrl"]
        .as_str()
        .expect("dry-run public url");
    assert!(public_url.starts_with("https://example-bucket.r2.dev/tmp/buf/"));
    assert_eq!(
        payload["data"]["request"]["assets"]["images"][0]["url"],
        Value::String(public_url.to_owned())
    );
}

#[tokio::test]
async fn posts_create_calls_create_post_and_returns_created_post() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    write_env(home.path(), "BUF_API_TOKEN=test-token\n");

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query ChannelById"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "channel": sample_channel("instagram")
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("mutation CreatePost"))
        .and(body_string_contains("\"mode\":\"customScheduled\""))
        .and(body_string_contains(
            "\"images\":[{\"url\":\"https://example.com/image.jpg\"}]",
        ))
        .and(body_string_contains(
            "\"firstComment\":\"Source in comments\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "createPost": {
                    "__typename": "PostActionSuccess",
                    "post": sample_post("instagram")
                }
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "posts",
            "create",
            "--channel",
            "ch_123",
            "--body",
            "Hello Buffer",
            "--target",
            "schedule",
            "--at",
            "2026-03-26T10:28:47Z",
            "--media",
            "https://example.com/image.jpg",
            "--first-comment",
            "Source in comments",
        ])
        .output()
        .expect("run posts create");

    assert!(output.status.success(), "create should succeed");
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["post"]["id"],
        Value::String("post_123".to_owned())
    );
    assert_eq!(
        payload["data"]["post"]["status"],
        Value::String("scheduled".to_owned())
    );
}

#[tokio::test]
async fn posts_create_threads_remote_url_calls_create_post() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    write_env(home.path(), "BUF_API_TOKEN=test-token\n");

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query ChannelById"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "channel": sample_channel("threads")
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("mutation CreatePost"))
        .and(body_string_contains("\"mode\":\"customScheduled\""))
        .and(body_string_contains(
            "\"images\":[{\"url\":\"https://example.com/image.jpg\"}]",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "createPost": {
                    "__typename": "PostActionSuccess",
                    "post": sample_post("threads")
                }
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "posts",
            "create",
            "--channel",
            "ch_123",
            "--body",
            "Hello Threads",
            "--target",
            "schedule",
            "--at",
            "2026-03-26T10:28:47Z",
            "--media",
            "https://example.com/image.jpg",
        ])
        .output()
        .expect("run posts create");

    assert!(output.status.success(), "create should succeed");
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(
        payload["data"]["post"]["channelService"],
        Value::String("threads".to_owned())
    );
}

#[tokio::test]
async fn posts_list_uses_nested_due_at_filter_for_date_bounds() {
    let server = MockServer::start().await;
    let home = TempDir::new().expect("temp dir");
    write_env(
        home.path(),
        "BUF_API_TOKEN=test-token\nBUF_API_BASE_URL=http://placeholder.invalid\n",
    );

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query AccountOrganizations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "account": {
                    "organizations": [sample_organization()]
                }
            }
        })))
        .mount(&server)
        .await;

    Mock::given(method("POST"))
        .and(path("/"))
        .and(body_string_contains("query ListPosts"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "data": {
                "posts": {
                    "edges": [],
                    "pageInfo": {
                        "hasNextPage": false,
                        "endCursor": null
                    }
                }
            }
        })))
        .mount(&server)
        .await;

    let output = buf_command(home.path())
        .args([
            "--api-base-url",
            &server.uri(),
            "posts",
            "list",
            "--status",
            "scheduled",
            "--from",
            "2026-03-09T00:00:00Z",
            "--to",
            "2026-03-16T00:00:00Z",
        ])
        .output()
        .expect("run posts list");

    assert!(output.status.success(), "posts list should succeed");
    let payload = parse_single_json_line(&output);
    assert_eq!(payload["ok"], Value::Bool(true));
    assert_eq!(payload["data"]["posts"], Value::Array(vec![]));

    let requests = server.received_requests().await.expect("received requests");
    let list_request = requests
        .iter()
        .find(|request| String::from_utf8_lossy(&request.body).contains("query ListPosts"))
        .expect("list request");
    let body: Value = serde_json::from_slice(&list_request.body).expect("request body json");
    assert_eq!(
        body["variables"]["input"]["filter"]["dueAt"],
        serde_json::json!({
            "start": "2026-03-09T00:00:00Z",
            "end": "2026-03-16T00:00:00Z",
        })
    );
    assert_eq!(
        body["variables"]["input"]["filter"]["dueAtStart"],
        Value::Null
    );
    assert_eq!(
        body["variables"]["input"]["filter"]["dueAtStop"],
        Value::Null
    );
}
