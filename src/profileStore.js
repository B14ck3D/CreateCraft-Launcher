const fs = require('fs');
const path = require('path');
const { app, safeStorage } = require('electron');

function sessionsDir() {
  const d = path.join(app.getPath('userData'), 'premium-sessions');
  fs.mkdirSync(d, { recursive: true });
  return d;
}

function sessionPath(profileId) {
  const id = String(profileId || '').replace(/[^a-f0-9-]/gi, '');
  if (!id) throw new Error('profileId');
  return path.join(sessionsDir(), `${id}.enc`);
}

function sessionPathPlain(profileId) {
  const id = String(profileId || '').replace(/[^a-f0-9-]/gi, '');
  return path.join(sessionsDir(), `${id}.plain.json`);
}

function savePremiumMclc(profileId, mclcObj) {
  const key = String(mclcObj?.uuid || profileId || '').replace(/[^a-f0-9-]/gi, '');
  if (!key) throw new Error('Brak UUID profilu Minecraft');
  const json = JSON.stringify(mclcObj);
  if (safeStorage.isEncryptionAvailable()) {
    const enc = safeStorage.encryptString(json);
    const buf = Buffer.isBuffer(enc) ? enc : Buffer.from(String(enc), 'utf8');
    fs.writeFileSync(sessionPath(key), buf);
    try {
      fs.unlinkSync(sessionPathPlain(key));
    } catch {
      /* ignore */
    }
  } else {
    fs.writeFileSync(sessionPathPlain(key), json, { encoding: 'utf8', mode: 0o600 });
    try {
      fs.unlinkSync(sessionPath(key));
    } catch {
      /* ignore */
    }
  }
}

function loadPremiumMclc(profileId) {
  const key = String(profileId || '').replace(/[^a-f0-9-]/gi, '');
  if (!key) return null;
  const encPath = sessionPath(key);
  const plainPath = sessionPathPlain(key);
  if (fs.existsSync(encPath) && safeStorage.isEncryptionAvailable()) {
    const buf = fs.readFileSync(encPath);
    return JSON.parse(safeStorage.decryptString(buf));
  }
  if (fs.existsSync(plainPath)) {
    return JSON.parse(fs.readFileSync(plainPath, 'utf8'));
  }
  return null;
}

function deletePremiumMclc(profileId) {
  const key = String(profileId || '').replace(/[^a-f0-9-]/gi, '');
  if (!key) return;
  for (const p of [sessionPath(key), sessionPathPlain(key)]) {
    try {
      if (fs.existsSync(p)) fs.unlinkSync(p);
    } catch {
      /* ignore */
    }
  }
}

function migrateProfilesArray(profiles, lastProfileId) {
  if (!Array.isArray(profiles)) return { profiles: [], changed: false, newLastProfileId: lastProfileId || null };
  let changed = false;
  let newLast = lastProfileId ? String(lastProfileId) : null;
  const out = profiles.map((p) => {
    if (!p) return p;
    if (p.type !== 'premium' || !p.token || typeof p.token !== 'object') {
      const { token: _t, ...rest } = p;
      return rest;
    }
    const uuid = String(p.token.uuid || '').replace(/[^a-f0-9-]/gi, '');
    if (!uuid) {
      const { token: _t, ...rest } = p;
      return rest;
    }
    try {
      savePremiumMclc(uuid, p.token);
    } catch {
      return p;
    }
    changed = true;
    const oldId = String(p.id || '');
    if (newLast && oldId && newLast === oldId && oldId !== uuid) {
      newLast = uuid;
    }
    const { token: _t, ...rest } = p;
    return { ...rest, id: uuid };
  });
  return { profiles: out, changed, newLastProfileId: newLast };
}

module.exports = {
  savePremiumMclc,
  loadPremiumMclc,
  deletePremiumMclc,
  migrateProfilesArray,
};
