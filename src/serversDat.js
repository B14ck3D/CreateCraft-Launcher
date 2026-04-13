const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const nbt = require('prismarine-nbt');

const DEFAULT_NAME = 'Create Crafts PL';

function getDefaultServerForDat() {
  let host = (
    process.env.CREATECRAFT_SERVER_HOST ||
    process.env.SUPERSMP_SERVER_HOST ||
    'main.createcrafts.pl'
  )
    .trim()
    .replace(/^https?:\/\//i, '');
  let port = String(
    process.env.CREATECRAFT_SERVER_PORT || process.env.SUPERSMP_SERVER_PORT || '25565'
  ).trim() || '25565';
  const hostPort = host.lastIndexOf(':');
  if (hostPort > 0 && /^\d+$/.test(host.slice(hostPort + 1))) {
    port = host.slice(hostPort + 1);
    host = host.slice(0, hostPort);
  }
  const ipField = port === '25565' ? host : `${host}:${port}`;
  const matchKey = `${host.toLowerCase()}:${port}`;
  return { name: DEFAULT_NAME, ipField, matchKey };
}

function normIp(s) {
  return String(s || '')
    .trim()
    .toLowerCase();
}

function rowMatchKeyFromIpValue(ipStr) {
  const raw = normIp(ipStr);
  if (!raw) return '';
  const idx = raw.lastIndexOf(':');
  if (idx === -1) return `${raw}:25565`;
  const h = raw.slice(0, idx).replace(/^\[|\]$/g, '');
  const p = raw.slice(idx + 1);
  if (/^\d+$/.test(p)) return `${h}:${p}`;
  return `${raw}:25565`;
}

function listCompoundItems(serversTag) {
  if (!serversTag || serversTag.type !== 'list') return null;
  const inner = serversTag.value;
  if (!inner || inner.type !== 'compound' || !Array.isArray(inner.value)) return null;
  return inner.value;
}

function readServersDatRoot(file, log) {
  let root = {
    type: 'compound',
    name: '',
    value: {
      servers: {
        type: 'list',
        value: {
          type: 'compound',
          value: [],
        },
      },
    },
  };
  if (!fs.existsSync(file)) return root;
  try {
    const raw = fs.readFileSync(file);
    const buf = zlib.gunzipSync(raw);
    const parsed = nbt.parseUncompressed(buf, 'big');
    if (parsed && parsed.type === 'compound' && parsed.value) {
      root = parsed;
      if (root.name == null) root.name = '';
    }
  } catch (e) {
    log(`[servers.dat] Nie można odczytać, tworzę nowy: ${e.message}`);
  }
  return root;
}

function ensureDefaultServerInServersDatSync(gameRoot, onLog) {
  const log = onLog || (() => {});
  const rootResolved = path.resolve(gameRoot);
  const file = path.join(rootResolved, 'servers.dat');
  const { name: defaultName, ipField, matchKey } = getDefaultServerForDat();
  log(`[servers.dat] Plik: ${file} (wpis: ${defaultName} → ${ipField})`);

  let root = readServersDatRoot(file, log);

  if (!root.value) root.value = {};
  let serversTag = root.value.servers;
  if (!serversTag || serversTag.type !== 'list') {
    serversTag = {
      type: 'list',
      value: { type: 'compound', value: [] },
    };
    root.value.servers = serversTag;
  }

  let items = listCompoundItems(serversTag);
  if (!items) {
    root.value.servers = {
      type: 'list',
      value: { type: 'compound', value: [] },
    };
    items = listCompoundItems(root.value.servers);
  }

  if (!items) {
    log('[servers.dat] Błąd: nie udało się uzyskać listy servers');
    return;
  }

  let idx = -1;
  for (let i = 0; i < items.length; i++) {
    const row = items[i];
    const ipTag = row && row.ip;
    if (ipTag && ipTag.type === 'string') {
      if (rowMatchKeyFromIpValue(ipTag.value) === matchKey) {
        idx = i;
        break;
      }
    }
  }

  let changed = false;
  if (idx >= 0) {
    const row = items[idx];
    const wantName = nbt.string(defaultName);
    const wantIp = nbt.string(ipField);
    if (row.name?.value !== wantName.value || row.ip?.value !== wantIp.value) {
      row.name = wantName;
      row.ip = wantIp;
      changed = true;
      log(`[servers.dat] Zaktualizowano wpis (${ipField})`);
    }
  } else {
    items.push({
      name: nbt.string(defaultName),
      ip: nbt.string(ipField),
    });
    changed = true;
    log(`[servers.dat] Dodano wpis: ${defaultName} → ${ipField}`);
  }

  if (!changed) {
    log('[servers.dat] Wpis już zgodny — pomijam zapis.');
    return;
  }

  try {
    const uncompressed = nbt.writeUncompressed(root, 'big');
    const gz = zlib.gzipSync(uncompressed);
    fs.writeFileSync(file, gz);
  } catch (e) {
    log(`[servers.dat] Zapis nieudany: ${e.message}`);
  }
}

async function ensureDefaultServerInServersDat(gameRoot, onLog) {
  ensureDefaultServerInServersDatSync(gameRoot, onLog);
  return undefined;
}

module.exports = {
  ensureDefaultServerInServersDat,
  ensureDefaultServerInServersDatSync,
  getDefaultServerForDat,
  DEFAULT_NAME,
};
