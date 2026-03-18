## 1. Documentation

- Start with official product docs before changing behavior:
    - Buffer: `https://developers.buffer.com/guides/getting-started.html`
    - Buffer GraphQL reference: `https://developers.buffer.com/reference.html`
    - Buffer API standards: `https://developers.buffer.com/guides/api-standards.html`
    - Buffer API limits: `https://developers.buffer.com/guides/api-limits.html`
    - Cloudflare R2 public buckets: `https://developers.cloudflare.com/r2/data-access/public-buckets/`
    - Cloudflare R2 S3 compatibility: `https://developers.cloudflare.com/r2/api/s3/api/`
- Treat official docs as the contract for Buffer behavior. Use live account inspection only to validate runtime behavior, not to invent undocumented API guarantees.
- Treat local media upload as unsupported by Buffer itself. `buf` solves local files by normalizing them locally, staging them to R2, and then passing Buffer hosted URLs.
- Read code before changing surface area:
    - [`src/cli.rs`](/Users/han/Git/buf/src/cli.rs) for command grammar
    - [`src/tool_registry.rs`](/Users/han/Git/buf/src/tool_registry.rs) for discovery metadata
    - [`src/commands/posts.rs`](/Users/han/Git/buf/src/commands/posts.rs) for write-path orchestration
    - [`src/media/pipeline.rs`](/Users/han/Git/buf/src/media/pipeline.rs) for local media preparation
    - [`src/storage/service.rs`](/Users/han/Git/buf/src/storage/service.rs) for staging boundary
- Use `buf tools` and `buf health` as the live source of truth for contract and readiness checks. Do not rely on stale notes or shell history.
- When channel topology changes in Buffer, refresh with `buf channels list` and update the relevant `BUF_DEFAULT_CHANNEL_*` values before assuming old defaults still apply.

## 2. Repository Structure

```text
.
├── AGENTS.md
├── CLAUDE.md -> AGENTS.md
├── README.md -> AGENTS.md
├── src/
│   ├── commands/
│   ├── media/
│   ├── storage/
│   ├── buffer_api.rs
│   ├── cli.rs
│   ├── config.rs
│   ├── envelope.rs
│   ├── error.rs
│   ├── main.rs
│   └── tool_registry.rs
├── tests/
│   └── cli_contract.rs
├── Cargo.toml
├── buf.config.template.toml
└── package.json
```

- Keep `AGENTS.md` as the canonical project policy file. `CLAUDE.md` and `README.md` are compatibility symlinks and should not diverge.
- Keep command entrypoints thin under `src/commands/`.
- Keep platform-independent media logic under `src/media/`.
- Keep staging/provider logic under `src/storage/`.
- Keep Buffer GraphQL transport and schema mapping in [`src/buffer_api.rs`](/Users/han/Git/buf/src/buffer_api.rs).
- Keep contract tests in [`tests/cli_contract.rs`](/Users/han/Git/buf/tests/cli_contract.rs). Add unit tests close to modules when behavior is local and deterministic.

## 3. Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| CLI | Rust + Clap | JSON-first, non-interactive command surface |
| Envelope | `serde` + `serde_json` | Stable `ok/data/error/meta` contract |
| Buffer transport | GraphQL over `reqwest` blocking client | Simple runtime, explicit query/mutation mapping |
| Config | `.env` + `buf.config.toml` | Secrets in `.env`; small cached/default values in TOML |
| Media preparation | `ffprobe` + `ffmpeg` | Probe, normalize, and transcode local assets before staging |
| Storage abstraction | Internal `storage` service | Commands never talk to R2 directly |
| Storage provider | Cloudflare R2 via AWS S3 SDK | Public read URL plus authenticated S3-compatible upload |
| Quality gate | Bun wrapper around Cargo | Matches sibling Rust CLI projects and one-command verification |

## 4. Commands

- Use the installed `buf` binary for realistic validation. Use `cargo run --` while developing.
- Core discovery and readiness commands:
    - `buf tools`
    - `buf tools posts.create`
    - `buf health`
    - `buf config show`
    - `buf config validate`
- Read-only Buffer commands:
    - `buf channels list`
    - `buf channels resolve`
    - `buf posts list`
    - `buf posts get`
- Supported service selectors currently include `instagram`, `linkedin`, and `threads` for channel discovery, default-channel resolution, and post listing filters.
- For automation, treat `buf posts get <post-id>` as the canonical lookup for publication state and the live published permalink.
- Post outputs expose both Buffer `externalLink` and normalized `publishedUrl`. Use `publishedUrl` for app-level published post URLs.
- `buf posts list --service <service>` resolves that service to matching channel ids before querying Buffer. Prefer `--channel <id>` when the workflow already knows the exact channel.
- Write-path command:
    - `buf posts create`
- `posts.create` rules:
    - Use exactly one body source: `--body`, `--body-file`, or `--stdin`
    - Use one media surface only: repeat `--media <path-or-url>` as needed
    - Local file path: normalize locally, stage to R2, pass hosted URL to Buffer
    - Remote URL: pass through as-is in v1
    - Prefer `--dry-run` first for new flows, new assets, or changed normalization logic
    - Cross-posting policy is workflow-level, not implicit CLI behavior. If a non-Instagram post should also go to Threads, create that Threads post deliberately instead of assuming `buf` will fan out automatically.
- Quality commands:
    - `bun run util:check`
    - `bun run build`

## 5. Architecture

- Keep command modules thin. Business logic belongs in `buffer_api`, `media`, and `storage`.
- Preserve the CLI contract:
    - JSON is default stdout mode
    - one JSON line only in JSON mode
    - stable envelope keys: `ok`, `data` or `error`, `meta`
    - exit codes: `0` success, `1` failure, `2` blocked
- Keep one public media entry point only. Do not add parallel flags like `--image-url`, `--image-file`, `--video-url`, or provider-specific upload arguments.
- Keep storage provider details internal. Public commands should not expose R2-specific vocabulary unless a real operator workflow requires it.
- Keep provider substitution possible:
    - `storage::service` is the entry point
    - provider-specific code stays under `src/storage/providers/`
    - commands and Buffer transport should operate on normalized hosted assets only
- Keep local-media behavior deterministic:
    - images normalize to JPEG
    - videos normalize to H.264/AAC MP4
    - preserve aspect ratio
    - no crop or pad in v1
    - reject unsupported aspect ratios explicitly
    - preserve media order
    - reject mixed image/video sets in one post in v1
- Keep profiles platform-aware and conservative:
    - Instagram feed/carousel: fit within `2160 x 2700`
    - Instagram story/reel: fit within `2160 x 3840`
    - LinkedIn image: fit within `2160 x 2700`
    - LinkedIn video: fit within a `2304` long-edge envelope
- Keep default-channel handling explicit and small:
    - `.env` may define `BUF_DEFAULT_CHANNEL_INSTAGRAM`, `BUF_DEFAULT_CHANNEL_LINKEDIN`, and `BUF_DEFAULT_CHANNEL_THREADS`
    - `buf.config.toml` may cache the same values under `[default_channels]`
    - LinkedIn may resolve to either a company page or a personal profile; do not hardcode assumptions about channel type
- Keep published URL handling explicit:
    - published post URLs come from post lookup, not channel lookup
    - scheduled or draft posts may return `publishedUrl = null`
    - sent posts should surface `publishedUrl`, which aliases Buffer `externalLink`
    - store Buffer `post.id` from create flows and resolve the same id later instead of inferring publication from list scans alone
- Keep secrets out of TOML and git:
    - `.env` holds `BUF_API_TOKEN` and `BUF_MEDIA_*`
    - `buf.config.toml` is optional and should stay small
    - use discovered defaults for organization and channels instead of making the user fill a large config matrix
- Keep `BUF_*` as the only env namespace. Do not reintroduce `BUFF_*` or generic `BUFFER_*` aliases.
- Do not assume undocumented Buffer mutations exist. `createPost` is the only public write path currently implemented.
- Do not post to live Buffer accounts for routine verification unless explicitly asked. Prefer `--dry-run`, read-only commands, and isolated storage smoke tests.

## 6. Quality

- Update [`src/tool_registry.rs`](/Users/han/Git/buf/src/tool_registry.rs) whenever the command surface changes. `tools` output is part of the contract.
- Add or update tests whenever behavior changes in:
    - config resolution
    - health readiness
    - command parsing
    - post request normalization
    - media planning
    - storage staging
    - error envelopes
- Keep contract tests JSON-first. Assert on stdout envelope shape, not just text output.
- Prefer unit tests for deterministic helpers and ignored live tests for external systems.
- Live R2 tests must remain safe:
    - use temporary objects only
    - rely on the `tmp/buf/` lifecycle cleanup rule
    - never require a Buffer post mutation to validate storage
- Run `bun run util:check` before completion. If install path or release behavior changed, also run `bun run build`.
- Validate at least one realistic dry-run flow when changing the write path:
    - local image -> normalize -> stage -> Buffer request
    - remote URL -> pass-through -> Buffer request
- Keep errors explicit and actionable. Every failure path should map to a stable error code with a concrete hint.
