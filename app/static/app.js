// MiniArcade page: plain JavaScript with no build step. Everything the server
// sends reaches the screen through textContent, never as markup.

const POLL_MS = 1000;
const CODE = /^[ABCDEFGHJKMNPQRSTUVWXYZ23456789]{6}$/;

const byId = (id) => document.getElementById(id);
const ui = {
  start: byId('start'),
  intro: byId('intro'),
  newGame: byId('new-game'),
  lobby: byId('lobby'),
  status: byId('status'),
  role: byId('role'),
  invite: byId('invite'),
  inviteLink: byId('invite-link'),
  copyLink: byId('copy-link'),
  board: byId('board'),
  rematch: byId('rematch'),
  notice: byId('notice'),
};
const WELCOME = ui.intro.textContent;
const cells = [];

// The lobby this tab is in.
let code = null;
let token = null; // null while watching
let seat = null; // 1 or 2; null while watching
let full = false; // true when the join was refused because both seats are taken
let view = null; // the latest state from the server
let pollTimer = 0;
let polling = false;
let stopped = true;

// --- Seats -------------------------------------------------------------------
// sessionStorage belongs to one tab, so two tabs of one browser can hold both
// seats of a lobby. The in-memory copy covers browsers that block storage.

const memory = new Map();
const storageKey = (lobbyCode) => `miniarcade:${lobbyCode}`;

function loadSeat(lobbyCode) {
  if (memory.has(lobbyCode)) return memory.get(lobbyCode);
  try {
    const saved = JSON.parse(sessionStorage.getItem(storageKey(lobbyCode)));
    if (saved && typeof saved.token === 'string' && (saved.seat === 1 || saved.seat === 2)) {
      return saved;
    }
  } catch {
    // Unreadable or blocked storage: no seat.
  }
  return null;
}

function saveSeat(lobbyCode, newToken, newSeat) {
  const saved = { token: newToken, seat: newSeat };
  memory.set(lobbyCode, saved);
  try {
    sessionStorage.setItem(storageKey(lobbyCode), JSON.stringify(saved));
  } catch {
    // Blocked storage: the seat lasts until this page reloads.
  }
}

function forgetSeat(lobbyCode) {
  memory.delete(lobbyCode);
  try {
    sessionStorage.removeItem(storageKey(lobbyCode));
  } catch {
    // Nothing stored.
  }
}

// --- Server ------------------------------------------------------------------

// Only moves and rematches send the token; reading a lobby needs none.
async function api(method, path, { body, withToken = false } = {}) {
  const headers = {};
  if (withToken) headers['X-Player-Token'] = token;
  if (body !== undefined) headers['Content-Type'] = 'application/json';
  const response = await fetch(path, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
    cache: 'no-store',
  });
  const data = await response.json().catch(() => null);
  return { status: response.status, data };
}

// --- Screens -----------------------------------------------------------------

// Only touches the page when the text changes, so screen readers announce
// real changes rather than every poll.
function setText(node, text) {
  if (node.textContent !== text) node.textContent = text;
}

function showNotice(text) {
  setText(ui.notice, text);
  ui.notice.hidden = false;
}

function clearNotice() {
  ui.notice.hidden = true;
}

function showStart(message) {
  stopPolling();
  setText(ui.intro, message);
  ui.lobby.hidden = true;
  ui.start.hidden = false;
  ui.newGame.disabled = false;
}

function showExpired() {
  if (code !== null) forgetSeat(code);
  showStart("This lobby has expired or doesn't exist. Start a new game?");
}

function buildBoard() {
  for (let i = 0; i < 9; i += 1) {
    const cell = document.createElement('button');
    cell.type = 'button';
    cell.className = 'cell';
    cell.dataset.cell = String(i);
    cell.disabled = true;
    ui.board.append(cell);
    cells.push(cell);
  }
}

function myMark() {
  if (seat === null || view === null) return null;
  return seat === view.x_seat ? 'X' : 'O';
}

function statusText(mark) {
  switch (view.status) {
    case 'waiting':
      return seat === 1
        ? 'Waiting for a second player. Send them the link below.'
        : 'Waiting for a second player.';
    case 'playing':
      if (mark === null) return `${view.turn} to move.`;
      return view.turn === mark ? 'Your turn.' : 'Their turn.';
    case 'x_won':
    case 'o_won': {
      const winner = view.status === 'x_won' ? 'X' : 'O';
      if (mark === null) return `${winner} won.`;
      return winner === mark ? 'You won!' : 'You lost.';
    }
    case 'draw':
      return 'Draw.';
    default:
      return '';
  }
}

function roleText(mark) {
  if (mark !== null) return `You play ${mark}.`;
  return full ? "This lobby is full, so you're watching." : "You're watching.";
}

function render(next) {
  // A slow poll can answer after a move did: never step back to older state.
  if (view !== null && next.version < view.version) return;
  view = next;
  const mark = myMark();
  const myTurn = view.status === 'playing' && mark !== null && view.turn === mark;
  view.board.forEach((value, i) => {
    const cell = cells[i];
    setText(cell, value ?? '');
    cell.classList.toggle('x', value === 'X');
    cell.classList.toggle('o', value === 'O');
    cell.disabled = !(myTurn && value === null);
    const where = `Row ${Math.floor(i / 3) + 1}, column ${(i % 3) + 1}`;
    cell.setAttribute('aria-label', `${where}: ${value ?? 'empty'}`);
  });
  ui.board.classList.toggle('my-turn', myTurn);
  setText(ui.status, statusText(mark));
  setText(ui.role, roleText(mark));
  ui.invite.hidden = !(view.status === 'waiting' && seat === 1);
  const finished = ['x_won', 'o_won', 'draw'].includes(view.status);
  ui.rematch.hidden = !(finished && mark !== null);
}

// --- Polling -----------------------------------------------------------------

async function poll() {
  if (polling || stopped) return;
  polling = true;
  clearTimeout(pollTimer);
  try {
    const { status, data } = await api('GET', `/api/lobbies/${code}`);
    if (status === 200) {
      clearNotice();
      render(data);
    } else if (status === 404) {
      showExpired();
    } else {
      showNotice('The server is having trouble. Retrying…');
    }
  } catch {
    showNotice('Connection lost. Retrying…');
  } finally {
    polling = false;
  }
  if (!stopped) pollTimer = setTimeout(poll, POLL_MS);
}

function startPolling() {
  stopped = false;
  poll();
}

function stopPolling() {
  stopped = true;
  clearTimeout(pollTimer);
}

// --- Actions -----------------------------------------------------------------

function handleWrite(status, data) {
  if (status === 200) {
    clearNotice();
    render(data);
  } else if (status === 404) {
    showExpired();
  } else if (status === 403) {
    // The server no longer knows this tab's token: watch instead.
    forgetSeat(code);
    token = null;
    seat = null;
    poll();
  } else {
    // 409: someone else wrote first; 400: the board changed. Show what's there now.
    poll();
  }
}

async function play(cell) {
  for (const button of cells) button.disabled = true; // one click per turn
  try {
    const body = { cell, version: view.version };
    const { status, data } = await api('POST', `/api/lobbies/${code}/move`, { body, withToken: true });
    handleWrite(status, data);
  } catch {
    showNotice('Connection lost. Retrying…');
    poll();
  }
}

async function rematch() {
  ui.rematch.disabled = true;
  try {
    const body = { version: view.version };
    const { status, data } = await api('POST', `/api/lobbies/${code}/rematch`, { body, withToken: true });
    handleWrite(status, data);
  } catch {
    showNotice('Connection lost. Retrying…');
    poll();
  } finally {
    ui.rematch.disabled = false;
  }
}

async function enterLobby(lobbyCode) {
  code = lobbyCode;
  view = null;
  full = false;
  ({ token, seat } = loadSeat(code) ?? { token: null, seat: null });
  if (token === null) {
    try {
      const { status, data } = await api('POST', `/api/lobbies/${code}/join`);
      if (status === 200) {
        ({ token } = data);
        seat = 2;
        saveSeat(code, token, seat);
      } else if (status === 404) {
        showExpired();
        return;
      } else if (status === 409) {
        full = true;
      } else {
        showNotice("Couldn't take a seat, so you're watching. Reload to try again.");
      }
    } catch {
      showNotice("Couldn't reach the server. Reload to try again.");
    }
  }
  ui.inviteLink.value = `${location.origin}/?lobby=${code}`;
  ui.start.hidden = true;
  ui.lobby.hidden = false;
  startPolling();
}

async function createLobby() {
  ui.newGame.disabled = true;
  try {
    const { status, data } = await api('POST', '/api/lobbies');
    if (status !== 201 || !CODE.test(data?.code)) throw new Error(`create failed: ${status}`);
    saveSeat(data.code, data.token, 1);
    history.replaceState(null, '', `/?lobby=${data.code}`);
    clearNotice();
    await enterLobby(data.code);
  } catch {
    showNotice("Couldn't start a game. Try again in a moment.");
    ui.newGame.disabled = false;
  }
}

async function copyLink() {
  ui.inviteLink.select();
  let copied = false;
  try {
    // The Clipboard API exists only on HTTPS and localhost pages.
    await navigator.clipboard.writeText(ui.inviteLink.value);
    copied = true;
  } catch {
    // Plain HTTP: copy the selected text the older way.
    try {
      copied = document.execCommand('copy');
    } catch {
      copied = false;
    }
  }
  setText(ui.copyLink, copied ? 'Copied' : 'Press Ctrl+C');
  setTimeout(() => setText(ui.copyLink, 'Copy'), 2000);
}

// --- Start -------------------------------------------------------------------

function main() {
  buildBoard();
  ui.newGame.addEventListener('click', createLobby);
  ui.rematch.addEventListener('click', rematch);
  ui.copyLink.addEventListener('click', copyLink);
  ui.inviteLink.addEventListener('focus', () => ui.inviteLink.select());
  ui.board.addEventListener('click', (event) => {
    const cell = event.target.closest('.cell');
    if (cell && !cell.disabled) play(Number(cell.dataset.cell));
  });

  const requested = new URLSearchParams(location.search).get('lobby');
  if (requested === null) {
    showStart(WELCOME);
  } else if (CODE.test(requested.toUpperCase())) {
    enterLobby(requested.toUpperCase());
  } else {
    showExpired();
  }
}

main();
