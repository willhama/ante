# CLAUDE.md

Project context for future Claude sessions. Keep concise. Update when truths change.

## What this is

**ante** is a Tauri 2 + SvelteKit desktop writing app with AI ghost autocomplete. Pages-on-a-canvas layout, TipTap rich text, streaming completions from OpenAI / Anthropic / OpenAI-compatible endpoints.

## Stack

- Frontend: SvelteKit + Svelte 5 runes, TypeScript, Tailwind v4, shadcn-svelte.
- Editor: TipTap v3 (ProseMirror).
- Shell: Tauri v2, Rust edition 2021.
- AI transport: `reqwest` + `eventsource-stream` + `futures-util` on the Rust side. SSE streamed via Tauri events. No Vercel AI SDK. No frontend HTTP calls to providers.
- Package manager: **pnpm** (not npm, not yarn).

## Commands

| Action | Command |
|---|---|
| Install | `pnpm install` |
| Dev (full app) | `pnpm tauri dev` |
| Dev (frontend only) | `pnpm dev` |
| Build | `pnpm tauri build` |
| Type-check frontend | `pnpm check` |
| Production build (frontend only) | `pnpm build` |
| Rust check | `cd src-tauri && cargo check --all-targets` |
| Rust tests | `cd src-tauri && cargo test --lib` |

Rust work happens in `src-tauri/`. Frontend work in `src/`. Always run `pnpm check` + `cargo check --all-targets` before declaring a frontend or Rust change complete.

## Directory map

```
src/
  app.css                       Tailwind entry + globals. Do NOT re-reset Preflight.
  routes/+page.svelte           Main editor shell + startup bootstrap.
  lib/
    editor/GhostCompletion.ts   TipTap extension. Owns the invoke + event-listen streaming plumbing. Treat as stable.
    ui/SettingsDialog.svelte    Provider dropdown + per-provider config UI.
    ui/Toolbar.svelte           AI toggle button, dark-mode switch.
    state/app-state.svelte.ts   Global Svelte rune store.
src-tauri/
  src/
    lib.rs                      Tauri entry + invoke_handler! registration.
    errors.rs                   AnteError enum.
    ai.rs                       Commands: stream_completion, cancel_completion, load_ai_config, save_ai_config, test_ai_config, get_ai_config. Also: migration + env-var fallback + sanitize_error.
    ai/
      provider.rs               Provider enum + ProviderClient trait + StreamParams + ParseOutcome + client_for factory.
      openai.rs                 OpenAI + OpenAI-compatible impls (share one build_request + one parse_sse_data).
      anthropic.rs              Anthropic /v1/messages impl with named-event SSE parser.
  Cargo.toml                    Deps: tauri, reqwest (rustls), eventsource-stream, futures-util, async-trait, serde, tokio, tauri-plugin-store.
  capabilities/default.json     Tauri capabilities (network, dialog, store, etc).
.omc/plans/                     Work plans. multi-provider-ai.md is the authoritative spec for the AI architecture.
```

## AI architecture

- **Streaming lives in Rust.** Frontend calls `invoke('stream_completion', {context_before, context_after})` and listens on events `completion-chunk`, `completion-done`, `completion-error`. `cancel_completion(request_id)` aborts.
- **Provider abstraction** via the `ProviderClient` trait in `src-tauri/src/ai/provider.rs`. Three impls: `OpenAiClient`, `OpenAiCompatibleClient`, `AnthropicClient`. `parse_sse_data` takes `(event_name, data)` because Anthropic uses named SSE events (`content_block_delta`, `message_stop`, etc.). OpenAI uses anonymous events + `[DONE]` sentinel.
- **Config shape** in `ai-config.json` (tauri-plugin-store, app data dir):
  ```json
  {
    "active_provider": "openai" | "openai-compatible" | "anthropic",
    "providers": { "<slug>": { "api_key", "model", "base_url", "max_tokens" } },
    "trigger_speed": "eager" | "balanced" | "relaxed",
    "_migrated_v1": true
  }
  ```
  `api_key` lives here (plain JSON, file perms 0600 on Unix by Tauri default). No OS keychain; no stronghold.
- **Migration** from pre-v1 (single top-level `api_key` / `model` / `base_url` / `max_tokens`) runs once on first `load_ai_config` / `get_ai_config`. Idempotent, guarded by `_migrated_v1`.
- **Env-var fallbacks**: `ANTE_OPENAI_API_KEY`, `ANTE_OPENAI_COMPATIBLE_API_KEY`, `ANTE_ANTHROPIC_API_KEY`. Legacy `OPENAI_API_KEY` still honored for the `openai` slot with a one-time stderr deprecation warning (`AtomicBool` gate).
- **Error sanitization** (`sanitize_error` in `ai.rs`) strips URLs and tokens from any error surfaced to the UI. Extend it if you add a new error shape. Tests in `ai.rs` cover existing cases.
- **Test button** per provider: OpenAI/compat does `GET {base_url}/models` (cheap, auth-only). Anthropic does a 1-token `POST /v1/messages` with `claude-haiku-4-5-20251001` (fractions of a cent per click).

When you add a 4th provider: write a new `ProviderClient` impl, add an enum variant, update the factory in `provider.rs`, extend the settings UI with a new sub-panel. That is the whole checklist.

## Conventions

- **No em-dashes (`—`) in any text.** The user hates them. Use ` - ` (hyphen) in prose, comments, UI strings, commits, and plan docs. No en-dashes either.
- **Dark mode** is controlled by `[data-theme="dark"]` on `<html>`, not by shadcn's `.dark` class. Match this in any new styling.
- **No manual global `*` margin/padding resets** in `app.css`. Tailwind's Preflight already resets; re-resetting breaks shadcn component spacing.
- Keep `GhostCompletion.ts` streaming plumbing intact when changing AI internals. The TS contract (invoke + three events + cancel) is the stable boundary.
- Default to writing **no code comments**. Only add one when the WHY is non-obvious. Do not reference tasks / fixes / PRs in comments.
- Prefer editing existing files over creating new ones.
- No Vercel AI SDK, no `@tauri-apps/plugin-http`, no frontend HTTP calls to providers. All outbound HTTP goes through Rust.

## Testing

- Rust unit tests live in-module (`#[cfg(test)] mod tests`). `cargo test --lib` is the authoritative suite. Current count: 19 (sanitize_error, Provider serde roundtrip, OpenAI + Anthropic SSE parsers).
- No frontend unit tests yet. `pnpm check` (svelte-check) and `pnpm build` are the type + compile gate.
- Manual E2E for AI streaming requires real provider keys; there is no mocked streaming integration test.

## Gotchas

- **macOS sed / grep** are BSD flavors. Use `perl -i -pe` for in-place UTF-8 replacements, or `find … -print0 | xargs -0 grep -F $'\xe2\x80\x94'` for byte-pattern searches. The ripgrep shipped with Claude Code is sometimes unavailable; fall back to `grep` via Bash.
- **`eventsource-stream`'s `Event` exposes both `.event` (name) and `.data` (payload).** Today's OpenAI parse only needed `.data`. Anthropic needed `.event` too. `ProviderClient::parse_sse_data` takes both.
- **Store plugin unavailability in dev**: `get_config` falls back to env vars only if `tauri-plugin-store` is not initialized yet. Normal flow uses the store.
- **`tauri::generate_handler!` macro**: changing a command's Rust signature means updating the frontend `invoke` call site too. `pnpm check` will not catch Rust-to-TS type drift; you must.
- **`pnpm tauri dev` port conflict**: Vite defaults to 5173. If busy, `lsof -iTCP:5173 -sTCP:LISTEN` + kill the stale node process.
- **Anthropic test-key cost**: every click on "Test" in the Anthropic settings panel fires a real (paid) 1-token request. Negligible but not zero.

## Memory + plans

- `.omc/plans/multi-provider-ai.md` - authoritative spec for the current AI architecture. Keep in sync if you change the provider shape.
- `.omc/` is gitignored. `.claude/` and `.vscode/` are too.
