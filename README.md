<p align="center">
  <img src="ANTE-ICON.png" alt="ante logo" width="140">
</p>

<h1 align="center">ANTE — AI Native Text Editor</h1>

<p align="center"><em>Write well. Without the bloat.</em></p>

Since creating software has become cheap and AI has become genuinely amazing, why not attack the big boss — Micro\*\*\*\* — with its own tools? Why pay a monthly subscription for a word processor when we can build one ourselves that is better, leaner, and less cluttered?

That is the idea behind **ante**: a small, fast, desktop writing app. Pages on a canvas. A toolbar you actually understand. AI ghost-autocompletion built in from day one. No ribbon. No telemetry. No login wall. No 300MB installer that wants admin rights.

It is not trying to be Word. It is trying to be the thing you reach for when you want to just write.

## Status

Early. Usable for plain prose. Expect rough edges while the core settles.

## Features

- **Page-based layout** — Letter / A4 / Legal. Your document looks the way it will print, with real margins and page breaks.
- **Editable headers & footers** — Click the top/bottom margin of any page and type. Mirrored across all pages, saved with the document.
- **Rich-text editing** powered by Tiptap: headings, lists, blockquotes, bold/italic/underline/strike, font family & size, color, alignment, indent, links.
- **AI ghost autocomplete** — Tab to accept, Esc to dismiss. Works with any OpenAI-compatible API.
  - Pick your model (gpt-4o-mini, gpt-4o, gpt-4.1, ...or a custom one).
  - Choose suggestion length: short / medium / long.
  - Choose trigger speed: eager / balanced / relaxed.
  - API key validated on paste with a live check.
- **Dark mode** that follows your system preference, or force light/dark.
- **Save / open / save-as** native dialogs. Documents are plain HTML on disk — open them in any browser, diff them in git.
- **Unsaved-changes prompt** on window close so you do not lose work.
- **Keyboard-first**: `Cmd+N` / `Cmd+O` / `Cmd+S` / `Cmd+Shift+S` / `Cmd+Shift+A` (toggle AI) / `Cmd+,` (settings).

## Stack

- **Frontend:** SvelteKit (Svelte 5 runes) + TypeScript + Tailwind CSS v4 + shadcn-svelte.
- **Editor:** Tiptap v3 (ProseMirror).
- **Shell:** Tauri v2 (Rust). Small binary, native file dialogs, real OS windows.
- **AI:** `reqwest` streaming SSE to any OpenAI-compatible `/chat/completions` endpoint.

## Development

```bash
pnpm install
pnpm tauri dev
```

Production build:

```bash
pnpm tauri build
```

## AI setup

1. Open Settings (`Cmd+,`).
2. Paste your OpenAI API key. It is tested automatically against `GET /v1/models` and stored locally in `ai-config.json` (plain JSON today — keychain migration tracked on the roadmap).
3. Pick a model, suggestion length, and trigger speed.
4. Start typing. After ~10 characters of context and a short pause, a ghost completion appears. Press `Tab` to accept, `Esc` to dismiss.

Each request sends a sliding window around the cursor — up to 500 chars before, 200 chars after — not the entire document. No conversation history is kept; every suggestion is an independent call.

## Roadmap

- Per-document settings (font, margins, page size persisted with the file)
- Find / replace
- Export to DOCX / PDF
- Inline comments & suggestions mode
- Local model support via Ollama
- Keychain-backed key storage

## License

MIT.
