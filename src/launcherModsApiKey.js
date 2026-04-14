const fs = require('fs');
const path = require('path');
const { decryptModsApiKeyFromBuffer } = require('./launcherModsKeyEmbed');

const MIN_LEN = 16;

function normalizeFirstKey(raw) {
  const s = String(raw || '').trim();
  if (!s) return null;
  const first = s.split(',')[0].trim();
  return first.length >= MIN_LEN ? first : null;
}

function getElectronApp() {
  try {
    return require('electron').app;
  } catch {
    return null;
  }
}

function resolveEmbeddedResourceDir() {
  const app = getElectronApp();
  if (app && app.isPackaged) return process.resourcesPath;
  return path.join(__dirname, '..', 'build');
}

function loadEmbeddedLauncherModsApiKey() {
  try {
    const dir = resolveEmbeddedResourceDir();
    const plainPath = path.join(dir, 'launcher-mods-key');
    if (fs.existsSync(plainPath)) {
      const raw = fs.readFileSync(plainPath, 'utf8');
      const k = normalizeFirstKey(raw);
      if (k) return { key: k, source: 'embedded' };
    }
    const encPath = path.join(dir, 'launcher-mods-key.enc');
    if (!fs.existsSync(encPath)) return null;
    const buf = fs.readFileSync(encPath);
    const key = decryptModsApiKeyFromBuffer(buf);
    return key ? { key, source: 'embedded' } : null;
  } catch {
    return null;
  }
}

function removeStaleUserDataModsKeyFile(userDataDir, reason) {
  if (!userDataDir) return;
  try {
    const fp = path.join(userDataDir, 'launcher-mods-api-key');
    const app = getElectronApp();
    if (app && app.isPackaged && fs.existsSync(fp)) {
      fs.unlinkSync(fp);
      console.info('[mods-api-key]', reason, fp);
    }
  } catch {}
}

function loadLauncherModsApiKey({ userDataDir } = {}) {
  const embedded = loadEmbeddedLauncherModsApiKey();
  if (embedded) {
    removeStaleUserDataModsKeyFile(
      userDataDir,
      'Usunięto przestarzały launcher-mods-api-key — używany jest wbudowany launcher-mods-key.enc z builda/MSI.'
    );
    return embedded;
  }

  const fromEnv = normalizeFirstKey(process.env.LAUNCHER_MODS_API_KEY);
  if (fromEnv) return { key: fromEnv, source: 'env' };

  if (userDataDir) {
    const fp = path.join(userDataDir, 'launcher-mods-api-key');
    try {
      if (fs.existsSync(fp)) {
        const raw = fs.readFileSync(fp, 'utf8');
        const fromFile = normalizeFirstKey(raw);
        if (fromFile) return { key: fromFile, source: 'file' };
      }
    } catch {
    }
  }

  return null;
}

function requireLauncherModsApiKey(opts) {
  const r = loadLauncherModsApiKey(opts);
  if (!r) {
    throw new Error(
      'Brak klucza API modów (min. 16 znaków). Priorytet: launcher-mods-key (plain) lub launcher-mods-key.enc w buildzie/MSI, potem LAUNCHER_MODS_API_KEY, na końcu launcher-mods-api-key w userData.'
    );
  }
  return r.key;
}

module.exports = {
  loadLauncherModsApiKey,
  requireLauncherModsApiKey,
  MIN_LEN,
};
