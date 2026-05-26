import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

type LogLevel = "info" | "success" | "error";

interface LcuStatusPayload {
  connected: boolean;
  message: string;
}

interface LcuLogPayload {
  timestamp: string;
  text: string;
  level: LogLevel;
}

interface AcceptFiredPayload {
  success: boolean;
}

interface StatusResponse {
  auto_accept_enabled: boolean;
  connected: boolean;
}

interface LogLine {
  timestamp: string;
  text: string;
  level: LogLevel;
}

function App() {
  const [connected, setConnected] = useState(false);
  const [autoAcceptEnabled, setAutoAcceptEnabled] = useState(true);
  const [acceptFlash, setAcceptFlash] = useState(false);
  const [logs, setLogs] = useState<LogLine[]>([]);
  const logFeedRef = useRef<HTMLDivElement | null>(null);
  const flashTimerRef = useRef<number | null>(null);

  const connectionText = useMemo(() => {
    return connected ? "CONNECTED" : "DISCONNECTED";
  }, [connected]);

  const appendLog = (entry: LogLine) => {
    setLogs((prev) => {
      const next = [...prev, entry];
      return next.slice(-60);
    });
  };

  const nowTimestamp = () => {
    return new Date().toLocaleTimeString("en-GB", { hour12: false });
  };

  const formatTimestamp = (raw: string) => {
    if (/^\d+$/.test(raw)) {
      const epochMs = Number.parseInt(raw, 10) * 1000;
      return new Date(epochMs).toLocaleTimeString("en-GB", { hour12: false });
    }
    return raw;
  };

  useEffect(() => {
    const feed = logFeedRef.current;
    if (!feed) {
      return;
    }
    feed.scrollTop = feed.scrollHeight;
  }, [logs]);

  useEffect(() => {
    let isMounted = true;
    const unlisteners: UnlistenFn[] = [];

    const setupListeners = async () => {
      try {
        const status = await invoke<StatusResponse>("get_status");
        if (isMounted) {
          setAutoAcceptEnabled(status.auto_accept_enabled);
          setConnected(status.connected);
        }
      } catch {
        appendLog({
          timestamp: nowTimestamp(),
          text: "Failed to fetch initial status.",
          level: "error",
        });
      }

      const unlistenStatus = await listen<LcuStatusPayload>("lcu-status", (event) => {
        setConnected(event.payload.connected);
        appendLog({
          timestamp: nowTimestamp(),
          text: event.payload.message,
          level: event.payload.connected ? "success" : "info",
        });
      });

      const unlistenLog = await listen<LcuLogPayload>("lcu-log", (event) => {
        appendLog({
          timestamp: formatTimestamp(event.payload.timestamp),
          text: event.payload.text,
          level: event.payload.level,
        });
      });

      const unlistenAccept = await listen<AcceptFiredPayload>("accept-fired", (event) => {
        setAcceptFlash(true);
        if (flashTimerRef.current !== null) {
          window.clearTimeout(flashTimerRef.current);
        }
        flashTimerRef.current = window.setTimeout(() => {
          setAcceptFlash(false);
        }, 900);

        appendLog({
          timestamp: nowTimestamp(),
          text: event.payload.success ? "Accept signal confirmed." : "Accept signal failed.",
          level: event.payload.success ? "success" : "error",
        });
      });

      unlisteners.push(unlistenStatus, unlistenLog, unlistenAccept);
    };

    void setupListeners();

    return () => {
      isMounted = false;
      unlisteners.forEach((unlisten) => unlisten());
      if (flashTimerRef.current !== null) {
        window.clearTimeout(flashTimerRef.current);
      }
    };
  }, []);

  const onToggleAutoAccept = async () => {
    const next = !autoAcceptEnabled;
    setAutoAcceptEnabled(next);

    try {
      await invoke("set_auto_accept_enabled", { enabled: next });
      appendLog({
        timestamp: nowTimestamp(),
        text: next ? "Auto-accept enabled." : "Auto-accept paused.",
        level: "info",
      });
    } catch {
      setAutoAcceptEnabled(!next);
      appendLog({
        timestamp: nowTimestamp(),
        text: "Failed to update auto-accept setting.",
        level: "error",
      });
    }
  };

  return (
    <main className={`app-shell ${acceptFlash ? "accept-flash" : ""}`}>
      <header className="panel-header">
        <div>
          <h1>LOLAC</h1>
          <p>v2.0 · LCU</p>
        </div>
        <div className="connection-wrap" title={connectionText}>
          <span className={`status-dot ${connected ? "online" : "offline"}`} />
          <span className="status-label">{connectionText}</span>
        </div>
      </header>

      <section className="panel-section">
        <p className="section-title">AUTO ACCEPT</p>
        <div className="toggle-row">
          <button
            className={`toggle ${autoAcceptEnabled ? "enabled" : ""}`}
            type="button"
            onClick={onToggleAutoAccept}
            aria-label="Toggle auto accept"
            aria-pressed={autoAcceptEnabled}
          >
            <span className="toggle-knob" />
          </button>
          <span className={`toggle-state ${autoAcceptEnabled ? "enabled" : "paused"}`}>
            {autoAcceptEnabled ? "ENABLED" : "PAUSED"}
          </span>
        </div>
      </section>

      <section className="panel-section">
        <div className="section-headline">
          <p className="section-title">CHAMP SELECT</p>
          <span className="soon-badge">SOON</span>
        </div>

        <label className="champ-row">
          <span>AUTO PICK</span>
          <input type="text" placeholder="Champion" disabled title="Coming soon" />
        </label>

        <label className="champ-row">
          <span>AUTO BAN</span>
          <input type="text" placeholder="Champion" disabled title="Coming soon" />
        </label>
      </section>

      <section className="panel-section log-shell">
        <div className="log-feed" ref={logFeedRef}>
          {logs.map((line, index) => (
            <p key={`${line.timestamp}-${index}`} className={`log-line level-${line.level}`}>
              <span className="log-time">[{line.timestamp}]</span>
              <span className="log-text">{line.text}</span>
            </p>
          ))}
          {logs.length === 0 && <p className="log-line level-info">[--:--:--] Idle. Waiting for events...</p>}
        </div>
      </section>
    </main>
  );
}

export default App;
