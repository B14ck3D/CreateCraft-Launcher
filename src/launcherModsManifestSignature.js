const crypto = require('crypto');

function buildManifestSignaturePayload(manifest) {
  const generated = String(manifest.generated ?? '');
  const count = Number(manifest.count);
  if (!Number.isFinite(count) || count < 0) {
    throw new Error('Manifest: nieprawidłowe pole count');
  }
  const countStr = String(count);
  const mods = Array.isArray(manifest.mods) ? manifest.mods : [];
  const lines = [];
  for (const m of mods) {
    const name = String(m.name ?? '');
    const size = Number(m.size);
    const sha256 = String(m.sha256 ?? '').toLowerCase();
    if (!name || !Number.isFinite(size) || size < 0) {
      throw new Error(`Manifest: nieprawidłowy wpis modu (name/size): ${name}`);
    }
    if (!/^[0-9a-f]{64}$/.test(sha256)) {
      throw new Error(`Manifest: nieprawidłowy sha256 dla: ${name}`);
    }
    lines.push(`${name}\t${size}\t${sha256}`);
  }
  lines.sort((a, b) => a.localeCompare(b, 'en'));
  return ['v1', generated, countStr, ...lines].join('\n');
}

function verifyLauncherModsManifestSignature(manifest, secret) {
  const sig = String(manifest.signature ?? '').toLowerCase();
  if (!/^[0-9a-f]{64}$/.test(sig)) return false;
  let payload;
  try {
    payload = buildManifestSignaturePayload(manifest);
  } catch {
    return false;
  }
  const hmac = crypto.createHmac('sha256', secret).update(payload, 'utf8').digest('hex');
  try {
    return crypto.timingSafeEqual(Buffer.from(hmac, 'utf8'), Buffer.from(sig, 'utf8'));
  } catch {
    return false;
  }
}

module.exports = {
  buildManifestSignaturePayload,
  verifyLauncherModsManifestSignature,
};
