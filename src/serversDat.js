const fs = require('fs');
const path = require('path');
const zlib = require('zlib');
const nbt = require('prismarine-nbt');

const DEFAULT_NAME = 'Create Crafts PL';
const DEFAULT_IP = 'main.createcrafts.pl';

function normIp(s) {
  return String(s || '')
    .trim()
    .toLowerCase();
}

function listCompoundItems(serversTag) {
  if (!serversTag || serversTag.type !== 'list') return null;
  const inner = serversTag.value;
  if (!inner || inner.type !== 'compound' || !Array.isArray(inner.value)) return null;
  return inner.value;
}

async function ensureDefaultServerInServersDat(gameRoot, onLog) {
  const log = onLog || (() => {});
  const rootResolved = path.resolve(gameRoot);
  const file = path.join(rootResolved, 'servers.dat');
  log(`[servers.dat] Plik: ${file}`);

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

  if (fs.existsSync(file)) {
    try {
      const raw = fs.readFileSync(file);
      const { parsed } = await nbt.parse(raw);
      root = parsed;
    } catch (e) {
      log(`[servers.dat] Nie można odczytać, tworzę nowy: ${e.message}`);
    }
  }

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

  const target = normIp(DEFAULT_IP);
  const has = items.some((row) => {
    const ipTag = row && row.ip;
    return ipTag && ipTag.type === 'string' && normIp(ipTag.value) === target;
  });
  if (!has) {
    items.push({
      name: nbt.string(DEFAULT_NAME),
      ip: nbt.string(DEFAULT_IP),
    });
  }

  try {
    const uncompressed = nbt.writeUncompressed(root, 'big');
    const gz = zlib.gzipSync(uncompressed);
    fs.writeFileSync(file, gz);
    if (!has) log(`[servers.dat] Dodano wpis: ${DEFAULT_NAME} → ${DEFAULT_IP} (${file})`);
  } catch (e) {
    log(`[servers.dat] Zapis nieudany: ${e.message}`);
  }
}

module.exports = { ensureDefaultServerInServersDat, DEFAULT_NAME, DEFAULT_IP };
