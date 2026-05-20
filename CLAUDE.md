# LolAC — LoL Auto-Accept Queue

A minimal standalone tool that automatically accepts League of Legends match queue pop-ups by interfacing with the League Client Update (LCU) API. No UI, no installer, no Electron — just a single script or compiled `.exe`.

## Project Structure

```
LolAC/
├── auto-accept.js   # Main script — all logic lives here
├── package.json     # Dependencies and pkg build config
└── dist/
    └── lolac.exe    # Compiled standalone executable (after build)
```

## Running

```bash
# From source (requires Node 18+ and npm install first)
node auto-accept.js

# As compiled exe (no Node required)
.\dist\lolac.exe
```

## Building the Executable

```bash
npm install
npm run build
# Output: dist/lolac.exe
```

Uses `pkg` to bundle Node.js + the script + dependencies into a single Windows executable.

**Critical**: the script must stay CommonJS (`require`) — `pkg` does not support ES modules (`import`). Do not add `"type": "module"` to `package.json`.

## Dependencies

- `ws` — WebSocket client (only external runtime dependency)
- `pkg` — dev dependency for compiling to exe

Everything else uses Node built-ins (`fs`, `https`).

---

## How It Works

### Overview

The League of Legends client exposes a local REST + WebSocket API called the **LCU (League Client Update) API**, accessible only on localhost. This tool authenticates with it, subscribes to game state events, and fires an accept call when a queue pop is detected.

### Step 1 — Lockfile Discovery

When the League client starts, it writes a **lockfile** at the League install directory:

```
C:\Riot Games\League of Legends\lockfile
```

The lockfile format is a single line, colon-separated:

```
name:pid:port:password:protocol
```

Example:
```
LeagueClient:12345:53421:abc123xyz:https
```

Fields used:
- `port` — the local port the LCU API is listening on (changes every launch)
- `password` — the Basic auth password for that session (changes every launch)
- `username` is always hardcoded as `riot`

The lockfile only exists while the client is running. When the client exits, it deletes the lockfile.

**Lockfile paths checked** (in order):
1. `C:\Riot Games\League of Legends\lockfile`
2. `%LOCALAPPDATA%\Riot Games\League of Legends\lockfile`
3. `%ProgramFiles%\Riot Games\League of Legends\lockfile`
4. `%ProgramFiles(x86)%\Riot Games\League of Legends\lockfile`

The script polls these paths every 2 seconds until one exists.

### Step 2 — LCU Authentication

All LCU requests use **HTTP Basic auth**:
- Username: `riot`
- Password: value from lockfile

Encoded as `Base64(riot:password)` and sent as the `Authorization: Basic <token>` header.

The LCU uses a **self-signed TLS certificate**, so both the HTTP agent and WebSocket connection must set `rejectUnauthorized: false`. This is expected and safe since all traffic stays on localhost.

### Step 3 — WebSocket Connection

The LCU WebSocket endpoint is:
```
wss://127.0.0.1:<port>/
```

Same Basic auth header is used. Once connected, events are subscribed to by sending a JSON message:
```json
[5, "OnJsonApiEvent_<endpoint_path_underscored>"]
```

Opcode `5` = subscribe, opcode `6` = unsubscribe.

Incoming event messages follow this format:
```json
[opcode, eventName, eventData]
```

Known opcodes:
- `8` — event payload (the only one we care about)

### Step 4 — Gameflow Phase Events

This tool subscribes to:
```
OnJsonApiEvent_lol-gameflow_v1_gameflow-phase
```

The full incoming message shape:
```json
[
  8,
  "OnJsonApiEvent_lol-gameflow_v1_gameflow-phase",
  {
    "uri": "/lol-gameflow/v1/gameflow-phase",
    "eventType": "Update",
    "data": "ReadyCheck"
  }
]
```

`eventData.data` is a plain string representing the current game phase. Possible values include:
- `"None"` — client idle
- `"Lobby"` — in a lobby
- `"Matchmaking"` — in queue
- `"ReadyCheck"` — queue popped, waiting for accept
- `"ChampSelect"` — champion select
- `"InProgress"` — game in progress
- `"EndOfGame"` — post-game screen

**Why gameflow phase and not the matchmaking ready-check event?**
The original Bocchi implementation uses gameflow phase transitions rather than subscribing directly to `OnJsonApiEvent_lol-matchmaking_v1_ready-check`. The phase event is more reliable as a trigger — it fires as soon as the client transitions state, while the matchmaking event requires parsing a more complex payload with `state` and `playerResponse` fields. The gameflow approach is simpler and proven to work.

### Step 5 — Accept Logic

When `eventData.data === "ReadyCheck"`:

1. Check `acceptPending` flag — if already handling a queue pop, ignore duplicate events
2. Set `acceptPending = true`
3. Wait **2 seconds** (intentional delay, mirrors Bocchi's behavior)
4. Re-query `/lol-gameflow/v1/gameflow-phase` via HTTP to confirm phase is still `"ReadyCheck"`
5. If confirmed, POST to `/lol-matchmaking/v1/ready-check/accept`
6. Set `acceptPending = false`

The re-confirm step is important — it prevents a stale accept from firing if the queue was cancelled or timed out during the delay.

The accept endpoint returns an empty body with status `200` or `204` on success.

### Step 6 — Reconnect

On WebSocket `close`:
1. `acceptPending` is reset to `false`
2. Wait 5 seconds
3. Full restart — re-poll for lockfile (handles client restart, port/password change)

This means if the League client is closed and reopened, the tool automatically picks up the new session without any manual intervention.

---

## Key Implementation Details

### `acceptPending` Flag

The gameflow phase WebSocket can fire multiple `Update` events in quick succession as the client transitions state. Without the `acceptPending` guard, multiple accept calls could be fired for the same queue pop. The flag ensures only one accept flow runs at a time and is always reset after completion or skip.

### ECONNREFUSED on First Connect

When the tool first detects the lockfile, the League client may not have fully initialized its WebSocket server yet. The first connection attempt often gets `ECONNREFUSED`. This is expected — the `close` handler catches it and retries after 5 seconds, by which time the client is ready. You will see this in the terminal output on first launch and it is not an error.

### Queue Cancelled During Delay

If a teammate declines the queue or the ready check times out during the 2-second wait, the gameflow phase will transition away from `"ReadyCheck"` (back to `"Matchmaking"` or `"None"`). The re-confirm HTTP check catches this and skips the accept call, logging the phase it changed to.

### Client Restart Mid-Session

If League is closed and reopened, the lockfile is deleted and rewritten with a new port and password. The WebSocket `close` event triggers the reconnect loop, which re-polls for the lockfile from scratch, picking up the new credentials automatically.

---

## Development & Testing

### Testing Without Queuing

You can interact with the LCU API directly using curl or any HTTP client. With the League client open:

```bash
# Get current lockfile contents
type "C:\Riot Games\League of Legends\lockfile"

# Check current gameflow phase (replace port and token)
curl -k -u riot:<password> https://127.0.0.1:<port>/lol-gameflow/v1/gameflow-phase

# Manually trigger accept (for testing if you're in a ready check)
curl -k -u riot:<password> -X POST https://127.0.0.1:<port>/lol-matchmaking/v1/ready-check/accept
```

The `-k` flag skips TLS verification (same as `rejectUnauthorized: false`).

### Inspecting LCU Events Live

Use [lcu-explorer](https://github.com/nicholasess/lcu-explorer) or similar community tools to browse all available LCU endpoints and watch WebSocket events in real time. Useful for debugging or adding new features.

### Watching Logs

When running from terminal, all output is timestamped. A healthy session looks like:

```
Waiting for League client.................
[7:19:40 PM] League client detected.
[7:19:42 PM] WebSocket error: connect ECONNREFUSED 127.0.0.1:53028
[7:19:42 PM] WebSocket closed.
[7:19:42 PM] Disconnected. Reconnecting in 5s...
Waiting for League client
[7:19:49 PM] League client detected.
[7:19:51 PM] Connected to LCU WebSocket.
[7:19:51 PM] Subscribed to gameflow phase events. Waiting for queue pop...

[7:20:26 PM] Queue pop detected! Waiting 2s before accepting...
[7:20:28 PM] ✓ Match accepted!
```

---

## Possible Future Improvements

- **System tray icon** — toggle auto-accept on/off without keeping a terminal open (would require Electron or a native tray library like `node-tray`)
- **Config file** — `config.json` for custom lockfile path, custom delay, enable/disable flag
- **CLI flags** — `--delay 3000`, `--lockfile "D:\Games\LoL\lockfile"`, `--no-reconfirm`
- **Log to file** — append timestamped logs to `lolac.log` alongside the exe
- **Notification on accept** — Windows toast notification via `node-notifier` or PowerShell
- **Auto-start on Windows login** — add a shortcut to `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`

---

## Derived From

The auto-accept logic is based on how [hitori-rebocchi/hitori-bocchi](https://github.com/hitori-rebocchi/hitori-bocchi) implements it internally, specifically:
- `src/main/services/lcuConnector.ts` — lockfile discovery, WebSocket setup, LCU HTTP client
- `src/main/services/gameflowMonitor.ts` — gameflow phase tracking and ready-check accept logic

This standalone version strips out all Electron IPC, Jotai state, renderer process communication, tray menu integration, and settings persistence — keeping only the core detection and accept logic as a self-contained Node.js script.
