const WebSocket = require('ws');
const http = require('http');
const fs = require('fs');

const PORT_FILE = `${process.env.HOME}/Library/Application Support/Google/Chrome/DevToolsActivePort`;
const SERVER_PORT = 9333;
const PID_FILE = '/tmp/chrome-cdp-server.pid';

function getEndpoint() {
  const lines = fs.readFileSync(PORT_FILE, 'utf8').trim().split('\n');
  return { port: lines[0], path: lines[1] };
}

// --- SERVER MODE ---
function startServer() {
  const { port, path } = getEndpoint();
  const wsUrl = `ws://127.0.0.1:${port}${path}`;
  let ws;
  let msgId = 0;
  const pending = new Map();
  const sessions = new Map(); // cache targetId -> sessionId

  function connect() {
    ws = new WebSocket(wsUrl);
    ws.on('open', () => console.log('Connected to Chrome CDP'));
    ws.on('message', (raw) => {
      const msg = JSON.parse(raw.toString());
      if (msg.id && pending.has(msg.id)) {
        pending.get(msg.id)(msg);
        pending.delete(msg.id);
      }
    });
    ws.on('close', () => { console.log('CDP disconnected'); process.exit(1); });
    ws.on('error', (err) => { console.error('CDP error:', err.message); process.exit(1); });
  }

  function send(method, params = {}, sessionId) {
    const id = ++msgId;
    return new Promise((resolve, reject) => {
      pending.set(id, resolve);
      const msg = { id, method, params };
      if (sessionId) msg.sessionId = sessionId;
      ws.send(JSON.stringify(msg));
      setTimeout(() => { pending.delete(id); reject(new Error('Timeout')); }, 15000);
    });
  }

  connect();

  const server = http.createServer(async (req, res) => {
    if (req.method !== 'POST') { res.writeHead(405); res.end(); return; }
    let body = '';
    req.on('data', c => body += c);
    req.on('end', async () => {
      try {
        const { action, args } = JSON.parse(body);
        const result = await handleAction(action, args, send, sessions);
        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ ok: true, result }));
      } catch (e) {
        res.writeHead(500, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ ok: false, error: e.message }));
      }
    });
  });

  server.listen(SERVER_PORT, '127.0.0.1', () => {
    fs.writeFileSync(PID_FILE, String(process.pid));
    console.log(`CDP server listening on http://127.0.0.1:${SERVER_PORT}`);
  });

  process.on('SIGTERM', () => { try { fs.unlinkSync(PID_FILE); } catch {} process.exit(0); });
  process.on('SIGINT', () => { try { fs.unlinkSync(PID_FILE); } catch {} process.exit(0); });
}

async function resolveTargetId(partialId, send) {
  const msg = await send('Target.getTargets');
  const match = msg.result.targetInfos.find(t => t.targetId.startsWith(partialId));
  if (!match) throw new Error(`No target found for: ${partialId}`);
  return match.targetId;
}

async function getSession(targetId, send, sessions) {
  if (sessions.has(targetId)) return sessions.get(targetId);
  const msg = await send('Target.attachToTarget', { targetId, flatten: true });
  sessions.set(targetId, msg.result.sessionId);
  return msg.result.sessionId;
}

async function handleAction(action, args, send, sessions) {
  // Resolve partial target IDs for commands that need them
  if (args.targetId && args.targetId.length < 32) {
    args.targetId = await resolveTargetId(args.targetId, send);
  }
  switch (action) {
    case 'tabs': {
      const msg = await send('Target.getTargets');
      const { targetInfos } = msg.result;
      return targetInfos.filter(t => t.type === 'page').map(p => ({
        id: p.targetId.slice(0, 8),
        fullId: p.targetId,
        title: p.title,
        url: p.url
      }));
    }
    case 'open': {
      const msg = await send('Target.createTarget', { url: args.url });
      return { opened: args.url, targetId: msg.result.targetId };
    }
    case 'close': {
      await send('Target.closeTarget', { targetId: args.targetId });
      return { closed: args.targetId };
    }
    case 'read': {
      const sessionId = await getSession(args.targetId, send, sessions);
      const msg = await send('Runtime.evaluate', { expression: 'document.body.innerText', returnByValue: true }, sessionId);
      return { text: msg.result.result.value };
    }
    case 'navigate': {
      const sessionId = await getSession(args.targetId, send, sessions);
      await send('Page.navigate', { url: args.url }, sessionId);
      return { navigated: args.url };
    }
    case 'screenshot': {
      const sessionId = await getSession(args.targetId, send, sessions);
      const msg = await send('Page.captureScreenshot', { format: 'png' }, sessionId);
      const outFile = args.outFile || '/tmp/screenshot.png';
      fs.writeFileSync(outFile, Buffer.from(msg.result.data, 'base64'));
      return { saved: outFile };
    }
    case 'js': {
      const sessionId = await getSession(args.targetId, send, sessions);
      const msg = await send('Runtime.evaluate', { expression: args.expression, returnByValue: true }, sessionId);
      return msg.result.result;
    }
    default:
      throw new Error(`Unknown action: ${action}`);
  }
}

// --- CLIENT MODE ---
async function client(action, args) {
  const body = JSON.stringify({ action, args });
  return new Promise((resolve, reject) => {
    const req = http.request({ hostname: '127.0.0.1', port: SERVER_PORT, method: 'POST', headers: { 'Content-Type': 'application/json' } }, (res) => {
      let data = '';
      res.on('data', c => data += c);
      res.on('end', () => {
        const parsed = JSON.parse(data);
        if (!parsed.ok) { reject(new Error(parsed.error)); return; }
        resolve(parsed.result);
      });
    });
    req.on('error', (e) => reject(new Error(`Server not running? Start with: node chrome.js serve\n${e.message}`)));
    req.write(body);
    req.end();
  });
}

async function main() {
  const [cmd, ...rest] = process.argv.slice(2);

  if (cmd === 'serve') {
    startServer();
    return;
  }

  // All other commands go through the client
  try {
    let result;
    switch (cmd) {
      case 'tabs':
        result = await client('tabs', {});
        result.forEach(t => console.log(`${t.id}  ${t.title}  |  ${t.url}`));
        break;
      case 'open':
        result = await client('open', { url: rest[0] });
        console.log('Opened:', result.opened, '| targetId:', result.targetId);
        break;
      case 'close':
        result = await client('close', { targetId: rest[0] });
        console.log('Closed:', result.closed);
        break;
      case 'read':
        result = await client('read', { targetId: rest[0] });
        console.log(result.text);
        break;
      case 'navigate':
        result = await client('navigate', { targetId: rest[0], url: rest[1] });
        console.log('Navigated to:', result.navigated);
        break;
      case 'screenshot':
        result = await client('screenshot', { targetId: rest[0], outFile: rest[1] });
        console.log('Saved screenshot to:', result.saved);
        break;
      case 'js':
        result = await client('js', { targetId: rest[0], expression: rest.slice(1).join(' ') });
        console.log(JSON.stringify(result, null, 2));
        break;
      case 'stop':
        const pid = fs.readFileSync(PID_FILE, 'utf8').trim();
        process.kill(Number(pid), 'SIGTERM');
        console.log('Server stopped');
        break;
      default:
        console.log('Start server:  node chrome.js serve');
        console.log('Commands:      tabs | open <url> | close <id> | read <id> | navigate <id> <url> | screenshot <id> [file] | js <id> <expr> | stop');
    }
  } catch (e) {
    console.error(e.message);
    process.exit(1);
  }
}

main();
