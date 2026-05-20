# LolAC — LoL Auto-Accept Queue

Automatically accepts League of Legends match queue pop-ups by interfacing with the LCU API. No UI, no installer, no Electron — just a single script or compiled `.exe`.

## Usage

**From source** (requires Node.js 18+):
```
npm install
node auto-accept.js
```

**As compiled executable** (no Node.js required):
```
.\dist\lolac.exe
```

**Build the exe yourself:**
```
npm install
npm run build
```

## How It Works

1. Detects the League client by polling `lockfile` at the LoL install directory
2. Connects to the LCU WebSocket with auto-discovered port and password
3. Subscribes to gameflow phase events
4. On `"ReadyCheck"` — waits 2 seconds, re-confirms phase, then POSTs `/lol-matchmaking/v1/ready-check/accept`
5. Automatically reconnects on client restart

The lockfile is found in order at:
- `C:\Riot Games\League of Legends\lockfile`
- `%LOCALAPPDATA%\Riot Games\League of Legends\lockfile`
- `%ProgramFiles%\Riot Games\League of Legends\lockfile`
- `%ProgramFiles(x86)%\Riot Games\League of Legends\lockfile`

## Expected Output

```
Waiting for League client.................
[7:19:40 PM] League client detected.
[7:19:51 PM] Connected to LCU WebSocket.
[7:19:51 PM] Subscribed to gameflow phase events. Waiting for queue pop...

[7:20:26 PM] Queue pop detected! Waiting 2s before accepting...
[7:20:28 PM] ✓ Match accepted!
```
