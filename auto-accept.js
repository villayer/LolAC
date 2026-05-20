/**
 * LoL Auto-Accept Queue — Standalone Script
 *
 * Requirements: Node.js 18+
 * Install dep:  npm install ws
 * Build exe:    npm run build
 * Run:          node auto-accept.js
 */

'use strict';

const fs = require('fs');
const https = require('https');
const { WebSocket } = require('ws');

// ── Config ────────────────────────────────────────────────────────────────────

const LOCKFILE_PATHS = [
  'C:\\Riot Games\\League of Legends\\lockfile',
  `${process.env.LOCALAPPDATA}\\Riot Games\\League of Legends\\lockfile`,
  `${process.env.ProgramFiles}\\Riot Games\\League of Legends\\lockfile`,
  `${process.env['ProgramFiles(x86)']}\\Riot Games\\League of Legends\\lockfile`,
];

const RECONNECT_DELAY_MS = 5000;
const ACCEPT_DELAY_MS = 2000;
const POLL_INTERVAL_MS = 2000;

// ── Lockfile ──────────────────────────────────────────────────────────────────

function findLockfile() {
  for (const p of LOCKFILE_PATHS) {
    try {
      if (fs.existsSync(p)) return p;
    } catch {
      // path might be undefined if env var is missing
    }
  }
  return null;
}

function parseLockfile(path) {
  const [, , port, password] = fs.readFileSync(path, 'utf8').split(':');
  return { port, password };
}

// ── LCU HTTP ──────────────────────────────────────────────────────────────────

const tlsAgent = new https.Agent({ rejectUnauthorized: false });

function lcuRequest(port, password, method, endpoint) {
  return new Promise((resolve, reject) => {
    const auth = Buffer.from(`riot:${password}`).toString('base64');
    const req = https.request(
      {
        hostname: '127.0.0.1',
        port,
        path: endpoint,
        method,
        headers: { Authorization: `Basic ${auth}` },
        agent: tlsAgent,
      },
      (res) => {
        let body = '';
        res.on('data', (chunk) => (body += chunk));
        res.on('end', () => {
          try {
            resolve({ status: res.statusCode, data: body ? JSON.parse(body) : null });
          } catch {
            resolve({ status: res.statusCode, data: body });
          }
        });
      }
    );
    req.on('error', reject);
    req.end();
  });
}

async function getGameflowPhase(port, password) {
  try {
    const res = await lcuRequest(port, password, 'GET', '/lol-gameflow/v1/gameflow-phase');
    if (res.status === 200) return res.data;
  } catch {
    // client may not be ready yet
  }
  return null;
}

async function acceptReadyCheck(port, password) {
  return lcuRequest(port, password, 'POST', '/lol-matchmaking/v1/ready-check/accept');
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

function connectWebSocket(port, password, onPhaseChange, onClose) {
  const auth = Buffer.from(`riot:${password}`).toString('base64');

  const ws = new WebSocket(`wss://127.0.0.1:${port}/`, {
    headers: { Authorization: `Basic ${auth}` },
    rejectUnauthorized: false,
  });

  ws.on('open', () => {
    log('Connected to LCU WebSocket.');
    ws.send(JSON.stringify([5, 'OnJsonApiEvent_lol-gameflow_v1_gameflow-phase']));
    log('Subscribed to gameflow phase events. Waiting for queue pop...\n');
  });

  ws.on('message', (raw) => {
    try {
      const msg = JSON.parse(raw);
      if (!Array.isArray(msg) || msg[0] !== 8) return;
      const phase = msg[2]?.data;
      if (typeof phase === 'string') {
        onPhaseChange(phase, port, password);
      }
    } catch {
      // ignore malformed frames
    }
  });

  ws.on('close', () => {
    log('WebSocket closed.');
    onClose();
  });

  ws.on('error', (err) => {
    log(`WebSocket error: ${err.message}`);
  });
}

// ── Ready Check Handler ───────────────────────────────────────────────────────

let acceptPending = false;

async function handlePhaseChange(phase, port, password) {
  if (phase !== 'ReadyCheck') return;
  if (acceptPending) return;

  acceptPending = true;
  log(`Queue pop detected! Waiting ${ACCEPT_DELAY_MS / 1000}s before accepting...`);

  await sleep(ACCEPT_DELAY_MS);

  const currentPhase = await getGameflowPhase(port, password);
  if (currentPhase !== 'ReadyCheck') {
    log(`Phase changed to "${currentPhase}" during delay — skipping accept.\n`);
    acceptPending = false;
    return;
  }

  try {
    const res = await acceptReadyCheck(port, password);
    if (res.status === 200 || res.status === 204) {
      log('✓ Match accepted!\n');
    } else {
      log(`⚠ Accept returned status ${res.status} — may already be accepted.\n`);
    }
  } catch (err) {
    log(`⚠ Failed to accept: ${err.message}\n`);
  }

  acceptPending = false;
}

// ── Main Loop ─────────────────────────────────────────────────────────────────

function sleep(ms) {
  return new Promise((r) => setTimeout(r, ms));
}

function log(msg) {
  const time = new Date().toLocaleTimeString();
  console.log(`[${time}] ${msg}`);
}

async function waitForLockfile() {
  process.stdout.write('Waiting for League client');
  return new Promise((resolve) => {
    const interval = setInterval(() => {
      const path = findLockfile();
      if (path) {
        clearInterval(interval);
        process.stdout.write('\n');
        log('League client detected.');
        resolve(path);
      } else {
        process.stdout.write('.');
      }
    }, POLL_INTERVAL_MS);
  });
}

async function run() {
  const lockfilePath = await waitForLockfile();
  await sleep(2000);
  const { port, password } = parseLockfile(lockfilePath);
  connectWebSocket(port, password, handlePhaseChange, async () => {
    log(`Disconnected. Reconnecting in ${RECONNECT_DELAY_MS / 1000}s...`);
    acceptPending = false;
    await sleep(RECONNECT_DELAY_MS);
    run();
  });
}

run();
