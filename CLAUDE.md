# LolAC Instructions

This file combines the current Tauri port instructions with the original standalone Node.js notes. The Tauri guidance below is the active source of truth.

## Current Goal

Port the existing LolAC auto-accept tool to a Tauri v2 desktop app with a minimal, polished UI. Preserve the core LCU behavior exactly: lockfile discovery, WebSocket connection, gameflow phase tracking, and auto-accept logic. The champ select feature is a UI and backend stub only for now.

## Working Rules

- Start with the backend. Do not touch the frontend until `cargo check` passes from `src-tauri/`.
- Keep the app compact and utility-like. Main window target size is 360x520.
- Use Tauri events for backend-to-frontend state updates.
- Do not re-scaffold or reinstall the app; work inside the existing `lolac-tauri/` project.
- Keep champ select as a TODO stub. It should log or print that it is not implemented yet.
- If a Tauri capability or plugin is changed, keep `src-tauri/capabilities/default.json` in sync.

## Tech Stack

- Tauri v2
- Rust backend
- React + Vite + TypeScript frontend
- Plain CSS only, no Tailwind or component library
- Tauri events for state bridge

## Backend Structure

Target files in `src-tauri/src/`:

- `main.rs`
- `lcu.rs`
- `champ_select.rs`

### Cargo Dependencies

Use the Rust dependencies needed for async LCU work, TLS, HTTP, JSON, and Tauri integration. The intended stack is:

- `tauri`
- `tokio`
- `tokio-tungstenite`
- `native-tls`
- `reqwest`
- `base64`
- `serde`
- `serde_json`
- `tauri-plugin-shell`

## LCU Behavior

Preserve the current auto-accept flow:

1. Poll for the lockfile every 2 seconds.
2. Check the usual Windows lockfile locations in order.
3. Parse `name:pid:port:password:protocol`.
4. Authenticate with Basic auth using `riot` and the lockfile password.
5. Connect to `wss://127.0.0.1:<port>/` with invalid certs accepted for localhost.
6. Subscribe to `OnJsonApiEvent_lol-gameflow_v1_gameflow-phase`.
7. When `ReadyCheck` arrives, guard with an `accept_pending` flag, wait 2 seconds, reconfirm the phase over HTTP, then POST `/lol-matchmaking/v1/ready-check/accept` if still valid.
8. On WebSocket close or error, reset state, wait 5 seconds, and restart discovery from the lockfile.

### Events to Frontend

Emit these Tauri events:

- `lcu-status` with `connected` and `message`
- `lcu-log` with timestamped log entries and a `level`
- `accept-fired` with `success`

## Frontend Requirements

Build a single-page React UI with a dark industrial/gaming utility-panel style. Use no routing and no UI framework.

### Fonts

Load Rajdhani for labels and JetBrains Mono for log text.

### Layout

The screen should include:

- A top bar with the app name and connection status dot
- An auto-accept toggle row with enabled/paused state
- A champ select section with disabled inputs and a SOON badge
- A scrollable log feed with max 60 entries and auto-scroll

### Frontend Behavior

- Listen for `lcu-status`, `lcu-log`, and `accept-fired` events.
- Call the `set_auto_accept_enabled` command when the toggle changes.
- Keep champ select controls disabled and labeled as coming soon.

## Tauri Commands

Register commands for:

- Toggling auto-accept enabled state
- Reading status for the UI

The application state should track whether auto-accept is enabled and whether the client is connected.

## Build and Run

- Dev: `npm run tauri dev`
- Release: `npm run tauri build`

## What Not to Do

- Do not use Electron.
- Do not keep the old `pkg` build pipeline.
- Do not add Node native addons.
- Do not add Tailwind or a component library.
- Do not implement champ select logic yet.
- Do not add routing.

## Legacy Standalone Notes

The original project was a minimal Node.js auto-accept script built around the LCU API. It used a lockfile, Basic auth, a self-signed TLS connection, a gameflow phase subscription, and a delayed re-check before accepting queue pop. Those details remain the behavioral reference for the Tauri backend.

The old standalone implementation relied on `auto-accept.js`, `ws`, and `pkg`, but those pieces are now superseded by the Tauri app.

## Repository Note

Before frontend edits, run a backend compile check from `src-tauri/`.
