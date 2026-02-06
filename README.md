# Job Hunter (Tauri + Next.js)

Job Hunter is a Tauri 2.0 desktop app with a Next.js 16 + Tailwind frontend that analyzes job listings from a URL, runs a multi-agent pipeline, and stores match results in SQLite.

## Features
- Multi-agent analysis: extractor + matcher.
- MCP-style service inside the Tauri backend.
- URL parameter sync for deep linking.
- Tauri Store-backed job search preferences.
- SQLite persistence for job matches.

## Architecture
- **Frontend**: Next.js 16 (App Router) + Tailwind CSS.
- **Backend**: Tauri 2.0 with an embedded MCP server.
- **Analysis agent**: a separate process of the same binary (spawned with `--analysis-agent`) that connects to the MCP service.
- **Storage**: Tauri Store (`job_settings.json`) + SQLite (`job_matches.sqlite`).

## Development
Install dependencies:

```bash
npm install
```

Run the Tauri app:

```bash
npm run tauri dev
```

If you want to preview the web UI only (no Tauri backend):

```bash
npm run dev
```

## Build
Create production bundles:

```bash
npm run tauri build
```

## MCP Tools
The embedded MCP server exposes these tools:
- `set_query_params`
- `fetch_content`
- `reload_page`
- `get_settings`
- `set_settings`
- `save_job_match`
- `list_job_matches`
- `clear_job_matches`

## GitHub Actions
The workflow in `.github/workflows/publish.yml` builds and publishes artifacts on tag pushes (`v*`) or manual dispatch.
