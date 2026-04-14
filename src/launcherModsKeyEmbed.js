const crypto = require('crypto');

const MAGIC = Buffer.from('CCMK01', 'ascii');

/** Fixed material split across buffers — not the API key; still recoverable from binary analysis. */
function embedMaterial() {
  return Buffer.concat([
    Buffer.from('7c9f2e41b8d304a6e51f0c2d8a7b4930', 'hex'),
    Buffer.from('pl.createcrafts.launcher.mods.embed', 'utf8'),
    Buffer.from('b3e8916c4f2a0d5e8c1b7a9d6e0f4c2a', 'hex'),
  ]);
}

function deriveEmbedKey() {
  const salt = Buffer.from('cc-lmods-embed-salt-v1\x00', 'utf8');
  return crypto.scryptSync(embedMaterial(), salt, 32, {
    N: 16384,
    r: 8,
    p: 1,
    maxmem: 64 * 1024 * 1024,
  });
}

function encryptModsApiKeyToBuffer(plainUtf8) {
  const plain = String(plainUtf8 || '').trim();
  if (plain.length < 16) throw new Error('Klucz musi mieć min. 16 znaków');
  const key = deriveEmbedKey();
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv('aes-256-gcm', key, iv);
  const enc = Buffer.concat([cipher.update(plain, 'utf8'), cipher.final()]);
  const tag = cipher.getAuthTag();
  return Buffer.concat([MAGIC, iv, tag, enc]);
}

function decryptModsApiKeyFromBuffer(buf) {
  if (!Buffer.isBuffer(buf) || buf.length < MAGIC.length + 12 + 16 + 1) return null;
  if (!buf.subarray(0, MAGIC.length).equals(MAGIC)) return null;
  let o = MAGIC.length;
  const iv = buf.subarray(o, o + 12);
  o += 12;
  const tag = buf.subarray(o, o + 16);
  o += 16;
  const data = buf.subarray(o);
  try {
    const key = deriveEmbedKey();
    const decipher = crypto.createDecipheriv('aes-256-gcm', key, iv);
    decipher.setAuthTag(tag);
    const dec = Buffer.concat([decipher.update(data), decipher.final()]);
    const s = dec.toString('utf8').trim();
    return s.length >= 16 ? s : null;
  } catch {
    return null;
  }
}

module.exports = {
  encryptModsApiKeyToBuffer,
  decryptModsApiKeyFromBuffer,
};
