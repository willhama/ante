<p align="center">
  <img src="ANTE-ICON.png" alt="ante logo" width="140">
</p>

<h1 align="center">ANTE - AI Native Text Editor</h1>

<p align="center"><em>Write well. Without the bloat.</em></p>

Since creating software has become cheap and AI has become genuinely amazing, why not attack the big boss - Micro\*\*\*\* - with its own tools? Why pay a monthly subscription for a word processor when we can build one ourselves that is better, leaner, and less cluttered?

That is the idea behind **ante**: a small, fast, desktop writing app. Pages on a canvas. A toolbar you actually understand. AI ghost-autocompletion built in from day one. No ribbon. No telemetry. No login wall. No 300MB installer that wants admin rights.

It is not trying to be Word. It is trying to be the thing you reach for when you want to just write.

## Status

Early. Usable for plain prose. Expect rough edges while the core settles.

## Features

- **Page-based layout** - Letter / A4 / Legal. Your document looks the way it will print, with real margins and page breaks.
- **Editable headers & footers** - Click the top/bottom margin of any page and type. Mirrored across all pages, saved with the document.
- **Rich-text editing** powered by Tiptap: headings, lists, blockquotes, bold/italic/underline/strike, font family & size, color, alignment, indent, links.
- **AI ghost autocomplete** - Tab to accept, Esc to dismiss. Multi-provider: pick OpenAI, Anthropic, or any OpenAI-compatible endpoint (Groq, Together, OpenRouter, local Ollama/LM Studio, ...).
  - Pick your model per provider.
  - Choose suggestion length: short / medium / long.
  - Choose trigger speed: eager / balanced / relaxed.
  - API key validated on paste with a live check.
  - API keys stored in the app data directory (`ai-config.json`), never transmitted outside the Rust process.
- **Dark mode** that follows your system preference, or force light/dark.
- **Save / open / save-as** native dialogs. Documents are plain HTML on disk - open them in any browser, diff them in git.
- **Unsaved-changes prompt** on window close so you do not lose work.
- **Keyboard-first**: `Cmd+N` / `Cmd+O` / `Cmd+S` / `Cmd+Shift+S` / `Cmd+Shift+A` (toggle AI) / `Cmd+,` (settings).

## Stack

- **Frontend:** SvelteKit (Svelte 5 runes) + TypeScript + Tailwind CSS v4 + shadcn-svelte.
- **Editor:** Tiptap v3 (ProseMirror).
- **Shell:** Tauri v2 (Rust). Small binary, native file dialogs, real OS windows.
- **AI:** `reqwest` streaming SSE from Rust. Pluggable `Provider` trait with concrete impls for OpenAI (`/chat/completions`), OpenAI-compatible endpoints, and Anthropic (`/v1/messages`). Keys stored in `ai-config.json` (app data dir) - they never cross the Tauri bridge.

## Development

```bash
pnpm install
pnpm tauri dev
```

Production build:

```bash
pnpm tauri build
```

## Providers

ante speaks three wire formats out of the box. Pick one in Settings (`Cmd+,`):

- **OpenAI** - native `/chat/completions`. Key validated with `GET /v1/models`.
- **Anthropic** - native `/v1/messages` with SSE. Key validated with a 1-token dry-run (costs fractions of a cent per click).
- **OpenAI-compatible** - any endpoint that mimics OpenAI's `/chat/completions` API. Supply a custom base URL. Examples: Groq (`https://api.groq.com/openai/v1`), Together, OpenRouter, vLLM, local Ollama (`http://localhost:11434/v1`), LM Studio.

Each provider has its own slot for API key + model + base URL; switching providers does not erase the others' settings. Keys are stored in `ai-config.json` in the app data directory - they never cross the Tauri bridge or leave the local machine.

Environment-variable fallbacks: `ANTE_OPENAI_API_KEY`, `ANTE_OPENAI_COMPATIBLE_API_KEY`, `ANTE_ANTHROPIC_API_KEY`. The legacy `OPENAI_API_KEY` is still accepted as a fallback for the `openai` provider in v1 but is deprecated.

Each request sends a sliding window around the cursor - up to 500 chars before, 200 chars after - not the entire document. No conversation history is kept; every suggestion is an independent call.

## AI setup

1. Open Settings (`Cmd+,`).
2. Pick a provider and paste your API key. It is tested automatically and, on success, stored in `ai-config.json` in the app data directory.
3. Pick a model, suggestion length, and trigger speed.
4. Start typing. After ~10 characters of context and a short pause, a ghost completion appears. Press `Tab` to accept, `Esc` to dismiss.

## Roadmap

- Per-document settings (font, margins, page size persisted with the file)
- Find / replace
- Export to DOCX / PDF
- Inline comments & suggestions mode
- Local model support via Ollama (already supported via the OpenAI-compatible provider; a dedicated first-class flow is still open)

## License

MIT.
