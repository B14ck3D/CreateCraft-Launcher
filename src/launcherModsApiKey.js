const fs = require('fs');
const path = require('path');

const MIN_LEN = 16;

function normalizeFirstKey(raw) {
  const s = String(raw || '').trim();
  if (!s) return null;
  const first = s.split(',')[0].trim();
  return first.length >= MIN_LEN ? first : null;
}

function loadLauncherModsApiKey({ userDataDir } = {}) {
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
      'Brak klucza API modów (min. 16 znaków). Ustaw zmienną LAUNCHER_MODS_API_KEY albo plik launcher-mods-api-key w folderze danych aplikacji (userData).'
    );
  }
  return r.key;
}

module.exports = {
  loadLauncherModsApiKey,
  requireLauncherModsApiKey,
  MIN_LEN,
};
