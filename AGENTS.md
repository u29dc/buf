> `buf` is a Rust CLI for agents that discovers Buffer organizations and channels, inspects posts, normalizes local media with `ffmpeg` and `ffprobe`, stages it to Cloudflare R2, and creates Buffer posts through the public GraphQL API.

## 1. Documentation

- Primary references: [Buffer getting started](https://developers.buffer.com/guides/getting-started.html), [Buffer GraphQL reference](https://developers.buffer.com/reference.html), [Buffer API standards](https://developers.buffer.com/guides/api-standards.html), [Buffer API limits](https://developers.buffer.com/guides/api-limits.html), [Cloudflare R2 public buckets](https://developers.cloudflare.com/r2/data-access/public-buckets/), [Cloudflare R2 S3 compatibility](https://developers.cloudflare.com/r2/api/s3/api/)
- Local source-of-truth files: [`src/cli.rs`](src/cli.rs), [`src/tool_registry.rs`](src/tool_registry.rs), [`src/commands/posts.rs`](src/commands/posts.rs), [`src/media/pipeline.rs`](src/media/pipeline.rs), [`src/storage/service.rs`](src/storage/service.rs), [`tests/cli_contract.rs`](tests/cli_contract.rs)
- Use `buf tools` for the public command contract and `buf health` for readiness. They are more reliable than old notes or shell history.
- Treat [`.tmp/docs/research.md`](.tmp/docs/research.md) and related files under [`.tmp/docs/`](.tmp/docs/) as exploratory only. They predate the current unified `--media` surface and some Threads behavior.

## 2. Repository Structure

```text
.
├── src/
│   ├── commands/             thin CLI adapters
│   ├── media/                media parsing, profiling, and normalization
│   ├── storage/              staging boundary and Cloudflare R2 provider
│   ├── buffer_api.rs         Buffer GraphQL queries and mutation mapping
│   ├── cli.rs                clap grammar and enums
│   ├── config.rs             runtime path and settings resolution
│   ├── envelope.rs           JSON/text output contract
│   └── tool_registry.rs      `buf tools` metadata source
├── tests/
│   └── cli_contract.rs       JSON-first CLI and request-shape contract tests
├── .tmp/docs/                non-authoritative research notes
├── AGENTS.md                 canonical repo-level agent instructions
├── CLAUDE.md -> AGENTS.md
└── README.md -> AGENTS.md
```

- Keep [`src/commands/`](src/commands/) thin. Buffer transport belongs in [`src/buffer_api.rs`](src/buffer_api.rs), media behavior in [`src/media/`](src/media/), and staging in [`src/storage/`](src/storage/).
- Treat [`tests/cli_contract.rs`](tests/cli_contract.rs) as the public contract suite. Update it when flags, envelope shape, tool metadata, request normalization, or published URL behavior changes.
- This repository is intentionally agent-first. Keep [`AGENTS.md`](AGENTS.md) canonical and preserve the symlinks from [`CLAUDE.md`](CLAUDE.md) and [`README.md`](README.md).

## 3. Stack

| Layer | Choice | Notes |
| --- | --- | --- |
| Runtime | Rust 2024 | single binary crate with `unsafe_code = "forbid"` |
| CLI | `clap` | noun-first subcommands, JSON-first output, `--text` override |
| Buffer transport | GraphQL over blocking `reqwest` | documented queries plus `createPost` mutation only |
| Media pipeline | `ffprobe` + `ffmpeg` | local probe, profile fit, normalization to JPEG or H.264/AAC MP4 |
| Storage | Cloudflare R2 via AWS S3 SDK | public-read URL plus authenticated S3-compatible upload |
| Async bridge | local `tokio` runtime inside storage provider | keeps the main CLI synchronous |
| JS tooling | Bun + Husky + commitlint + lint-staged | wrappers, hooks, and one-command quality gate |
| Tests | `assert_cmd`, `wiremock`, `tempfile` | contract-heavy CLI coverage with mocked Buffer responses |

## 4. Commands

- `bun install` installs JS tooling and Husky hooks.
- Use `cargo run -- ...` while iterating on code. Use the installed or built `buf` binary when you need real `BUF_HOME` runtime behavior.
- `./target/debug/buf tools` and `./target/debug/buf tools posts.create` expose the generated command contract from [`src/tool_registry.rs`](src/tool_registry.rs).
- `./target/debug/buf health`, `./target/debug/buf config show`, and `./target/debug/buf config validate` inspect runtime state without creating Buffer posts.
- `./target/debug/buf channels list --service instagram --limit 10` and `./target/debug/buf channels resolve --service threads` are the primary discovery commands for org-scoped channel work.
- `./target/debug/buf posts list --service instagram --status sent --limit 10` and `./target/debug/buf posts get <post-id>` are the canonical read paths for publication state and `publishedUrl`.
- `./target/debug/buf posts create --channel <id> --body-file ./post.md --media ./asset.jpg --target draft --dry-run` is the safest write-path smoke test.
- `bun run util:check` is the required quality gate. `bun run build` also copies the release binary into `${BUF_HOME:-${TOOLS_HOME:-$HOME/.tools}/buf}/buf`.

## 5. Architecture

- [`src/main.rs`](src/main.rs) parses the CLI, defaults stdout to JSON, dispatches commands, and maps success or failure to exit codes `0`, `1`, and `2`.
- [`src/envelope.rs`](src/envelope.rs) defines the public envelope. JSON mode must emit exactly one stdout line with `{ ok, data | error, meta }`; `--text` is a human-readable escape hatch.
- [`src/tool_registry.rs`](src/tool_registry.rs) is the single source for `buf tools`. Keep command strings, examples, input schemas, output fields, and flags synchronized with [`src/cli.rs`](src/cli.rs) and [`tests/cli_contract.rs`](tests/cli_contract.rs).
- [`src/buffer_api.rs`](src/buffer_api.rs) owns documented GraphQL queries and the only implemented write mutation, `createPost`. Do not invent undocumented Buffer mutations or local-upload API behavior.
- [`src/commands/posts.rs`](src/commands/posts.rs) resolves exactly one body source, injects `publishedUrl` from Buffer `externalLink`, and resolves `--service` post filters to channel ids because Buffer post filtering is channel-based, not service-based.
- [`src/media/`](src/media/) keeps one public media surface: repeatable `--media`. Local paths are probed, profile-checked, normalized, and staged; remote URLs pass through unchanged after scheme and extension checks.
- [`src/media/profile.rs`](src/media/profile.rs) encodes service rules: Instagram auto-promotes multiple images to `carousel`, story and reel stay single-asset, LinkedIn allows one asset and default `post` only, Threads allows multiple images or one video but no service-specific metadata yet.
- [`src/storage/service.rs`](src/storage/service.rs) is the only staging boundary. Commands and Buffer requests should operate on hosted asset URLs, not provider-specific upload details.

## 6. Runtime and State

- Path resolution: `--home`, `--config-file`, and `--env-file` override defaults; otherwise `BUF_HOME` wins, then `TOOLS_HOME/buf`, then `~/.tools/buf`. Runtime files live at `buf.config.toml`, `.env`, and `tmp/` under that home.
- Settings precedence: CLI path and base-URL overrides first; for resolved settings the order is process env -> `.env` -> `buf.config.toml` -> built-in defaults.
- Keep `BUF_*` as the only supported env namespace: `BUF_API_TOKEN`, `BUF_API_BASE_URL`, `BUF_REQUEST_TIMEOUT_MS`, `BUF_ORGANIZATION_ID`, `BUF_DEFAULT_CHANNEL_INSTAGRAM`, `BUF_DEFAULT_CHANNEL_LINKEDIN`, `BUF_DEFAULT_CHANNEL_THREADS`, and `BUF_MEDIA_*`.
- [`buf.config.template.toml`](buf.config.template.toml) shows the intended small config surface. Cache `defaultOrganizationId`, `[default_channels]`, and `[media]` there; keep secrets in `.env`.
- `media.key_prefix` only comes from TOML and defaults to `tmp/buf`. Uploaded R2 objects use UTC date prefixes plus a sanitized source stem.
- Commands that need an organization auto-discover it. If Buffer returns multiple orgs and no explicit default is configured, the CLI blocks with `ORG_AMBIGUOUS`.
- `config show` and `config validate` expose masked secret provenance (`process` vs `envFile`) without calling Buffer. `health` also creates or probes the home and temp dirs, checks `ffmpeg` and `ffprobe`, validates R2 readiness, and calls Buffer org discovery when a token is present.
- Readiness commands report structured status in `data` even on non-zero exits. `health` can return `ok: true` with `data.status = "blocked"` and exit `2`; `config.validate` can return `ok: true` with `valid: false` and exit `1` for parse errors.
- Local-media dry-runs still require `ffprobe` and complete `BUF_MEDIA_*` settings because the CLI probes the file and plans an R2 key even when it skips normalization and upload.
- Non-dry-run local media writes create a temporary workdir under `BUF_HOME/tmp`, normalize files there, then upload to a public R2 URL. Remote media does not get dimension probing; it is accepted or rejected locally based on URL scheme and filename extension only.

## 7. Conventions

- Preserve JSON-first stdout. `--text` changes presentation only; post content must stay on `--body`, `--body-file`, or `--stdin`.
- Keep the public media surface unified as `--media`. Do not reintroduce split flags like `--image-url`, `--video-file`, or provider-specific upload arguments.
- Preserve `publishedUrl` on post outputs as a non-breaking alias for Buffer `externalLink`.
- Keep tool names noun-first and keep [`src/cli.rs`](src/cli.rs), [`src/tool_registry.rs`](src/tool_registry.rs), and [`tests/cli_contract.rs`](tests/cli_contract.rs) aligned whenever the surface changes.

## 8. Constraints

- Do not post to live Buffer accounts for routine verification. Prefer `--dry-run`, read-only commands, and mocked tests unless the user explicitly requests a live write.
- Do not commit operator data from `~/.tools/buf`, `.env`, `buf.config.toml`, `.tmp/`, staged asset URLs, or other runtime state.
- Treat [`src/envelope.rs`](src/envelope.rs), [`src/tool_registry.rs`](src/tool_registry.rs), and [`tests/cli_contract.rs`](tests/cli_contract.rs) as high-risk contract files. Small drift there breaks agents quickly.
- Treat remote `--media` URLs without a usable filename extension as unsupported. The current detector classifies remote media from the URL path, not from HTTP headers.
- Do not hand-edit local or generated artifacts in `target/`, `.tmp/`, or `.husky/_/`. Regenerate or ignore them as appropriate.
- The ignored smoke test in [`src/storage/service.rs`](src/storage/service.rs) talks to live R2. Keep it disposable, use temporary objects only, and rely on the `tmp/buf/` lifecycle cleanup strategy.

## 9. Validation

- Required gate: `bun run util:check`
- When changing CLI grammar, envelopes, tool metadata, or default resolution, run `cargo test --test cli_contract` plus the relevant unit tests in `src/*`.
- When changing the write path or media pipeline, smoke `posts create --dry-run` with one local asset and one public remote URL and verify request assets, metadata, and staged or pass-through URLs.
- When changing config or runtime-path behavior, run `buf config show`, `buf config validate`, and `buf health` against the intended `BUF_HOME`.
- If you change command names, flags, examples, or output fields, update [`src/tool_registry.rs`](src/tool_registry.rs) and re-check `buf tools`.
