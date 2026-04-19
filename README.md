<p align="center">
  <img src="ANTE-ICON.png" alt="ante logo" width="140">
</p>

<h1 align="center">ANTE - AI Native Text Editor</h1>

<p align="center"><em>Write well. Without the bloat.</em></p>

Since creating software has become cheap and AI has become genuinely amazing, why not attack the big boss - Micro\*\*\*\* - with its own tools? Why pay a monthly subscription for a word processor when we can build one ourselves that is better, leaner, and less cluttered?

That is the idea behind **ante**: a small, fast, desktop writing app. Pages on a canvas. A toolbar you actually understand. AI ghost-autocompletion built in from day one. No ribbon. No telemetry. No login wall. No 300MB installer that wants admin rights.

It is not trying to be Word. It is trying to be the thing you reach for when you want to just write.

<p align="center">
  <img src="demo-gif.gif" alt="ante demo" width="720">
</p>

## Download

Grab the latest release for your platform:

[![macOS (Apple Silicon)](https://img.shields.io/badge/macOS-Apple%20Silicon-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/willhama/ante/releases/latest/download/ante-macOS-AppleSilicon.dmg)
[![macOS (Intel)](https://img.shields.io/badge/macOS-Intel-000000?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/willhama/ante/releases/latest/download/ante-macOS-Intel.dmg)
[![Windows (x64)](https://img.shields.io/badge/Windows-x64-0078D4?style=for-the-badge&logo=windows&logoColor=white)](https://github.com/willhama/ante/releases/latest/download/ante-Windows-x86_64-Setup.exe)
[![Linux (AppImage)](https://img.shields.io/badge/Linux-AppImage-FCC624?style=for-the-badge&logo=linux&logoColor=black)](https://github.com/willhama/ante/releases/latest/download/ante-Linux-x86_64.AppImage)
[![Linux (Deb)](https://img.shields.io/badge/Linux-deb-FCC624?style=for-the-badge&logo=debian&logoColor=white)](https://github.com/willhama/ante/releases/latest/download/ante-Linux-x86_64.deb)

[All releases](https://github.com/willhama/ante/releases)

Builds are currently unsigned. On macOS, run `xattr -d com.apple.quarantine /Applications/ante.app` after installing. On Windows, click "More info" then "Run anyway" on the SmartScreen prompt.

## Status

Early. Usable for plain prose. Expect rough edges while the core settles.

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

## License

MIT.
