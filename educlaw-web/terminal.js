/* educlaw-web/terminal.js
 *
 * xterm.js initialisation + WebSocket ↔ PTY bridge.
 * Called by api.js via window.startSession().
 */

let term, fitAddon, ws;

// ── Terminal setup ────────────────────────────────────────────────────────

function initTerminal() {
  term = new Terminal({
    cursorBlink: true,
    fontSize: 14,
    fontFamily: '"Cascadia Code", "Fira Code", "Consolas", monospace',
    theme: {
      background:  '#1a1a1a',
      foreground:  '#d4d4d4',
      cursor:      '#d4d4d4',
      black:       '#1a1a1a',
      brightBlack: '#555555',
      red:         '#e06c75',
      green:       '#98c379',
      yellow:      '#e5c07b',
      blue:        '#61afef',
      magenta:     '#c678dd',
      cyan:        '#56b6c2',
      white:       '#d4d4d4',
    },
    scrollback: 5000,
  });

  fitAddon = new FitAddon.FitAddon();
  term.loadAddon(fitAddon);
  term.open(document.getElementById('terminal-container'));
  fitAddon.fit();

  // Close-file-panel button (Phase 4 stub panel)
  document.getElementById('close-file-panel').addEventListener('click', () => {
    document.getElementById('file-panel').classList.add('hidden');
    fitAddon.fit();
  });

  window.addEventListener('resize', () => {
    fitAddon.fit();
    sendResize();
  });
}

// ── WebSocket connection ──────────────────────────────────────────────────

function connectSession(sessionId, token) {
  const proto = location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url   = `${proto}//${location.host}/sessions/${sessionId}/ws?token=${encodeURIComponent(token)}`;

  ws = new WebSocket(url);
  ws.binaryType = 'arraybuffer';

  ws.onopen = () => {
    setStatus('green', 'Connected');
    // Tell the server our initial dimensions
    sendResize();
    // Keepalive ping every 30 s
    startKeepalive();
  };

  ws.onmessage = (e) => {
    if (e.data instanceof ArrayBuffer) {
      // Raw PTY bytes → write directly to xterm
      term.write(new Uint8Array(e.data));
    } else {
      // JSON envelope from server
      try {
        handleEnvelope(JSON.parse(e.data));
      } catch {
        // Unknown text — display as-is
        term.write(e.data);
      }
    }
  };

  ws.onclose = () => {
    setStatus('red', 'Disconnected');
    term.write('\r\n\x1b[33m[EduClaw] Connection closed.\x1b[0m\r\n');
  };

  ws.onerror = () => {
    setStatus('yellow', 'Error');
  };

  // Keystrokes → server as JSON input frames
  term.onData((data) => {
    if (ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'input', data }));
    }
  });
}

// ── Helpers ───────────────────────────────────────────────────────────────

let keepaliveTimer = null;

function startKeepalive() {
  if (keepaliveTimer) clearInterval(keepaliveTimer);
  keepaliveTimer = setInterval(() => {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify({ type: 'ping' }));
    }
  }, 30_000);
}

function sendResize() {
  if (ws && ws.readyState === WebSocket.OPEN && term) {
    ws.send(JSON.stringify({ type: 'resize', cols: term.cols, rows: term.rows }));
  }
}

function setStatus(color, text) {
  const dot  = document.getElementById('status-dot');
  const span = document.getElementById('status-text');
  dot.className = `dot dot-${color}`;
  span.textContent = text;
}

// ── Envelope dispatcher (Phase 4 stubs) ──────────────────────────────────

function handleEnvelope(env) {
  switch (env.type) {
    case 'open_file':
      openFileStub(env.path, env.scope);
      break;
    case 'show_image':
      openFileStub(env.path, env.scope);
      break;
    case 'file_tree':
      // Phase 4: refresh file-tree sidebar
      break;
    case 'notify':
      showToast(env.message || '');
      break;
    default:
      // Ignore unknown envelope types
      break;
  }
}

function openFileStub(path, scope) {
  const panel = document.getElementById('file-panel');
  panel.classList.remove('hidden');
  document.getElementById('file-viewer').innerHTML =
    `<p class="stub-note" style="padding:0.5rem">
      <b>${scope || ''}/${path}</b><br>
      Full viewer — Phase 4
    </p>`;
  fitAddon.fit();
}

function showToast(message) {
  const t = document.createElement('div');
  t.className = 'toast';
  t.textContent = message;
  document.body.appendChild(t);
  setTimeout(() => t.remove(), 4_000);
}

// ── Public entry point (called by api.js) ─────────────────────────────────

window.startSession = function (sessionId, token, displayName) {
  document.getElementById('login-screen').classList.add('hidden');
  document.getElementById('session-screen').classList.remove('hidden');

  document.getElementById('session-label').textContent =
    `${displayName} · ${sessionId.slice(0, 8)}`;

  initTerminal();
  connectSession(sessionId, token);
};
