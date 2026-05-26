# Agent Instructions — LolAC Tauri Port

## Overview

Port the existing `LolAC` Node.js auto-accept tool to a **Tauri v2** desktop app with a minimal but polished UI. The core LCU logic (lockfile discovery, WebSocket connection, gameflow phase tracking, auto-accept) should be preserved exactly as-is — only the runtime shell changes.

The champ select (auto-pick / auto-ban) feature is a **TODO placeholder only** — wire up the UI toggle and settings fields, but leave the backend logic as a stub that logs "not yet implemented".

---

## Tech Stack

- **Tauri v2** (Rust backend + WebView frontend)
- **Frontend**: React + Vite (TypeScript)
- **Styling**: Plain CSS (no Tailwind, no component library)
- **LCU logic**: Ported from `auto-accept.js` into Rust (Tauri commands + async)
- **State bridge**: Tauri events (`emit` from Rust → `listen` in React)

---

## Project Initialization

**Already done.** The Tauri v2 app has been scaffolded at `C:\Users\Shadow\Documents\Dev\LolAC\lolac-tauri` using the React + TypeScript + Vite template. It compiles and opens a window successfully. Do NOT re-scaffold or reinstall — work directly in this existing project.

Current structure:
```
lolac-tauri/
├── src/              # App.tsx, App.css, main.tsx  ← replace these
├── src-tauri/        # Cargo.toml, tauri.conf.json, src/  ← add to these
├── index.html        # add Google Fonts link here
└── package.json
```

Start from Step 1: Backend (Rust). Do not touch the frontend until `cargo check` passes.

---

## Directory Structure (target)

```
lolac-tauri/
├── src/                    # React frontend
│   ├── App.tsx
│   ├── App.css
│   └── main.tsx
├── src-tauri/
│   ├── src/
│   │   ├── main.rs
│   │   ├── lcu.rs          # All LCU logic (lockfile, websocket, accept)
│   │   └── champ_select.rs # Stub only — TODO
│   ├── Cargo.toml
│   └── tauri.conf.json
└── package.json
```

---

## Backend — Rust (`src-tauri/src/`)

### `Cargo.toml` — add these dependencies

```toml
[dependencies]
tauri = { version = "2", features = ["tray-icon"] }
tokio = { version = "1", features = ["full"] }
tokio-tungstenite = { version = "0.21", features = ["native-tls"] }
native-tls = "0.2"
reqwest = { version = "0.12", features = ["native-tls"] }
base64 = "0.22"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tauri-plugin-shell = "2"
```

---

### `lcu.rs` — Full LCU logic

Port `auto-accept.js` 1-to-1 into Rust. Key behaviors to preserve:

#### Lockfile Discovery

Poll every 2 seconds for the lockfile at these paths (in order):
1. `C:\Riot Games\League of Legends\lockfile`
2. `%LOCALAPPDATA%\Riot Games\League of Legends\lockfile`
3. `%ProgramFiles%\Riot Games\League of Legends\lockfile`
4. `%ProgramFiles(x86)%\Riot Games\League of Legends\lockfile`

Parse the lockfile format: `name:pid:port:password:protocol` (colon-separated, single line).

Emit a Tauri event to the frontend when the client is detected:
```rust
app_handle.emit("lcu-status", LcuStatus { connected: true, message: "League client detected.".into() });
```

#### WebSocket Connection

Connect to `wss://127.0.0.1:<port>/` with:
- Basic auth header: `Authorization: Basic <base64(riot:<password>)>`
- TLS: `danger_accept_invalid_certs(true)` (self-signed cert, localhost only — this is expected and safe)

On connect, subscribe to gameflow phase:
```json
[5, "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase"]
```

#### Event Handling

Parse incoming WS messages as `[opcode, eventName, eventData]`. When opcode is `8` and `eventData.data == "ReadyCheck"`:

1. Check `accept_pending` atomic flag — skip if already handling
2. Set `accept_pending = true`
3. Sleep 2 seconds
4. HTTP GET `https://127.0.0.1:<port>/lol-gameflow/v1/gameflow-phase` to reconfirm phase is still `"ReadyCheck"`
5. If confirmed: POST to `https://127.0.0.1:<port>/lol-matchmaking/v1/ready-check/accept`
6. Emit event to frontend with result
7. Set `accept_pending = false`

All HTTP requests: Basic auth header, `danger_accept_invalid_certs`.

#### Reconnect

On WS close or error: reset `accept_pending`, wait 5 seconds, restart from lockfile discovery.

#### Emit These Events to Frontend

Define a Rust struct for each event payload (derive `Serialize`):

| Tauri Event Name | Payload fields |
|---|---|
| `lcu-status` | `connected: bool, message: String` |
| `lcu-log` | `timestamp: String, text: String, level: String` (level = "info" / "success" / "error") |
| `accept-fired` | `success: bool` |

---

### `champ_select.rs` — Stub

```rust
// TODO: Implement auto-pick and auto-ban during champion select
// LCU endpoints to use when implementing:
//   GET  /lol-champ-select/v1/session        — current session state
//   POST /lol-champ-select/v1/session/actions/{id}/complete  — lock in pick
//   PATCH /lol-champ-select/v1/session/actions/{id}          — set champion
// Subscribe to: OnJsonApiEvent_lol-champ-select_v1_session

pub async fn auto_pick_stub() {
    println!("[champ-select] auto-pick not yet implemented");
}

pub async fn auto_ban_stub() {
    println!("[champ-select] auto-ban not yet implemented");
}
```

---

### `main.rs`

- Spawn the LCU loop as a `tokio::spawn` task on app startup, passing the `AppHandle`
- Register Tauri commands (see below)
- Keep the main window dimensions: **360×520** (compact, utility-style)

#### Tauri Commands to register

```rust
#[tauri::command]
fn set_auto_accept_enabled(state: tauri::State<AppState>, enabled: bool) { ... }

#[tauri::command]
fn get_status(state: tauri::State<AppState>) -> StatusResponse { ... }
```

`AppState` should hold:
- `auto_accept_enabled: Arc<AtomicBool>` — toggled by the UI switch
- `connected: Arc<AtomicBool>` — updated by lcu.rs

The LCU loop should check `auto_accept_enabled` before firing the accept POST — if disabled, log "Auto-accept is paused" and skip.

---

## Frontend — React (`src/`)

### Design Direction

**Industrial / gaming utility panel.** Think dark UI, sharp edges, monospace status text, a single accent color in League gold (`#C89B3C`). No rounded-corner card softness. No gradients. Status feed that looks like a terminal. Feels like a tool a professional player would run.

### Fonts
- Import from Google Fonts: `Rajdhani` (headings/labels) + `JetBrains Mono` (status log)
- Add to `index.html`:
```html
<link href="https://fonts.googleapis.com/css2?family=Rajdhani:wght@500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet">
```

### Color Palette (CSS variables in `App.css`)

```css
:root {
  --bg:         #0a0a0b;
  --surface:    #111114;
  --border:     #1e1e24;
  --gold:       #C89B3C;
  --gold-dim:   #8a6a27;
  --text:       #d4d4d8;
  --text-muted: #52525b;
  --success:    #4ade80;
  --error:      #f87171;
  --font-ui:    'Rajdhani', sans-serif;
  --font-mono:  'JetBrains Mono', monospace;
}
```

### Layout — `App.tsx`

Single-page app. No routing. Layout (top to bottom):

```
┌─────────────────────────────────┐
│  LOLAC          [status dot]    │  ← header bar, gold border-bottom
├─────────────────────────────────┤
│  AUTO ACCEPT                    │
│  [toggle switch]  ENABLED       │  ← main feature row
├─────────────────────────────────┤
│  CHAMP SELECT          [soon]   │  ← section header with "SOON" badge
│  Auto-pick   [input: champ]     │
│  Auto-ban    [input: champ]     │
│  (inputs disabled, tooltip:     │
│   "Coming soon")                │
├─────────────────────────────────┤
│  ┌───────────────────────────┐  │
│  │ 19:47:03  Connected       │  │  ← scrollable log feed
│  │ 19:47:05  Waiting for     │  │     monospace, max 60 lines
│  │           queue pop...    │  │
│  │ 19:47:26  ✓ Match accepted│  │
│  └───────────────────────────┘  │
└─────────────────────────────────┘
```

### Component Details

#### Header
- App name `LOLAC` in Rajdhani 700, letter-spacing 0.15em
- Status dot: 8px circle, green (`#4ade80`) when connected, red when not, with a CSS `@keyframes pulse` glow animation when connected
- Subtitle: `v2.0 · LCU` in muted monospace

#### Auto-Accept Toggle
- Custom CSS toggle switch (no third-party library)
- Label: `AUTO ACCEPT` in Rajdhani 600 uppercase
- State text: `ENABLED` (gold) / `PAUSED` (muted)
- On toggle: call Tauri command `set_auto_accept_enabled`

#### Champ Select Section
- Section label with a `SOON` badge (gold outline, small caps)
- Two rows: `AUTO PICK` and `AUTO BAN`, each with a text input for champion name
- Inputs are `disabled`, cursor `not-allowed`, `opacity: 0.4`
- Add `title="Coming soon"` attribute for native tooltip

#### Log Feed
- `overflow-y: auto`, `max-height: 180px`, auto-scroll to bottom on new entry
- Each line: `[timestamp]  message` in JetBrains Mono 12px
- Color-code by level: info = `--text-muted`, success = `--success`, error = `--error`
- Keep max 60 entries in state (drop oldest)

### Tauri Event Listeners (`useEffect` in App.tsx)

```typescript
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

// On mount:
unlisten1 = await listen('lcu-status', (event) => { /* update connected state */ });
unlisten2 = await listen('lcu-log',    (event) => { /* append to log feed */ });
unlisten3 = await listen('accept-fired', (event) => { /* flash UI / log */ });

// Cleanup on unmount
```

### Window Config (`tauri.conf.json`)

```json
"windows": [{
  "title": "LolAC",
  "width": 360,
  "height": 520,
  "resizable": false,
  "decorations": true,
  "center": true
}]
```

---

## Build & Run

```bash
# Dev mode (hot reload frontend, Rust recompiles on save)
npm run tauri dev

# Production build → outputs to src-tauri/target/release/
npm run tauri build
```

The release build produces a single `.exe` installer (NSIS) and a portable `.exe`. Both are in `src-tauri/target/release/bundle/`.

---

## What NOT to do

- Do not port the old `pkg` build pipeline — Tauri handles bundling
- Do not use `node-gyp` or any native Node addons
- Do not add Electron as a dependency
- Do not use Tailwind or any CSS framework
- Do not add routing (React Router, etc.) — single page only
- Do not implement champ select backend logic — stub + TODO only

---

## TODO (future task — do not implement now)

```
[ ] Auto-pick champion during champ select
    - Subscribe to OnJsonApiEvent_lol-champ-select_v1_session
    - When it's the player's pick turn, PATCH the action to set champion, then POST /complete
    - Configurable delay before locking (default 2s)

[ ] Auto-ban champion during champ select
    - Same session subscription
    - Detect ban phase + player's ban action ID
    - PATCH + complete with configured ban champion

[ ] Both features gated by the UI toggle in the Champ Select section
[ ] Champion name → champion ID lookup via LCU endpoint:
    GET /lol-game-data/assets/v1/champion-summary.json
```

---

## Questions / Unknowns Resolved by Best Practice

| Question | Decision |
|---|---|
| Tauri v1 or v2? | v2 — current stable, better async support |
| Frontend framework | React + TypeScript (consistent with Mohamed's stack) |
| LCU logic in Rust or JS? | Rust — keeps everything in the Tauri backend, no Node runtime needed |
| Window size | 360×520, not resizable — utility tool, not a dashboard |
| System tray? | Not in this iteration — keep scope tight |
| Auto-start on boot? | Not in this iteration — TODO |