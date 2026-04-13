const fs = require('fs');
const path = require('path');
const https = require('https');
const tls = require('tls');
const crypto = require('crypto');
const { PassThrough } = require('stream');
const { pipeline } = require('stream/promises');
const { URL } = require('url');

const CREATECRAFTS_SPKI_SHA256_B64 = ['mXC/m3zXpYXTKFA4fKCGeYq0jpeXjpxc0WNHYGvv5n8='];

function isCreateCraftsPlHost(hostname) {
  const h = String(hostname || '').toLowerCase();
  return h === 'createcrafts.pl' || h === 'www.createcrafts.pl';
}

function pinCreateCraftsTls(servername, cert) {
  const err = tls.checkServerIdentity(servername, cert);
  if (err) return err;
  const der = cert && cert.raw;
  if (!der) return new Error('Brak certyfikatu serwera');
  const pin = crypto.createHash('sha256').update(der).digest('base64');
  if (!CREATECRAFTS_SPKI_SHA256_B64.includes(pin)) {
    return new Error(`Pin TLS niezgodny (${servername})`);
  }
  return undefined;
}

function assertLauncherModsApiPath(pathname) {
  const p = String(pathname || '');
  if (!p.startsWith('/api/launcher/mods/')) {
    throw new Error(`Niedozwolona ścieżka API modów: ${p}`);
  }
}

function modsApiBaseUrl() {
  const raw = (process.env.CREATECRAFTS_MODS_API_BASE || 'https://createcrafts.pl').trim();
  const u = new URL(raw.endsWith('/') ? raw.slice(0, -1) : raw);
  if (u.protocol !== 'https:') throw new Error('CREATECRAFTS_MODS_API_BASE musi być HTTPS');
  if (!isCreateCraftsPlHost(u.hostname)) {
    throw new Error(`Niedozwolony host API modów: ${u.hostname}`);
  }
  return u.origin;
}

function httpErrorMessage(statusCode) {
  if (statusCode === 401) return 'Odrzucono klucz API modów (401). Sprawdź LAUNCHER_MODS_API_KEY lub plik launcher-mods-api-key.';
  if (statusCode === 503) return 'Serwer modów niedostępny (503) — brak konfiguracji lub katalogu MODS_DIR po stronie panel-api.';
  if (statusCode === 404) return 'Nie znaleziono pliku moda (404).';
  if (statusCode === 400) return 'Nieprawidłowe żądanie pobrania moda (400).';
  return `HTTP ${statusCode}`;
}

function httpsRequestLauncherMods(urlStr, apiKey, method) {
  return new Promise((resolve, reject) => {
    const u = new URL(urlStr);
    if (u.protocol !== 'https:') return reject(new Error('Wymagany HTTPS'));
    if (!isCreateCraftsPlHost(u.hostname)) {
      return reject(new Error(`Niedozwolony host: ${u.hostname}`));
    }
    assertLauncherModsApiPath(u.pathname);
    const opts = {
      protocol: u.protocol,
      hostname: u.hostname,
      port: u.port || 443,
      path: u.pathname + u.search,
      method: method || 'GET',
      headers: {
        'User-Agent': 'createcrafts-launcher-modpack/2',
        'X-Launcher-Key': apiKey,
      },
      rejectUnauthorized: true,
      servername: u.hostname,
      checkServerIdentity: (srv, cert) => pinCreateCraftsTls(srv, cert),
    };
    const req = https.request(opts, (res) => resolve(res));
    req.on('error', reject);
    req.setTimeout(120000, () => {
      req.destroy();
      reject(new Error('Timeout'));
    });
    req.end();
  });
}

async function fetchLauncherModsManifestJson(apiKey) {
  const base = modsApiBaseUrl();
  const url = `${base}/api/launcher/mods/manifest`;
  const res = await httpsRequestLauncherMods(url, apiKey, 'GET');
  const chunks = [];
  for await (const c of res) chunks.push(c);
  const body = Buffer.concat(chunks).toString('utf8');
  if (res.statusCode !== 200) {
    throw new Error(`${httpErrorMessage(res.statusCode)} — manifest`);
  }
  try {
    return JSON.parse(body);
  } catch {
    throw new Error('Manifest modów: niepoprawny JSON');
  }
}

async function downloadLauncherModJar(apiKey, fileName, destFile, expectedSha256, expectedSize, onBytes) {
  const base = modsApiBaseUrl();
  const u = new URL('/api/launcher/mods/download', base);
  u.searchParams.set('file', fileName);
  const res = await httpsRequestLauncherMods(u.href, apiKey, 'GET');
  if (res.statusCode !== 200) {
    res.resume();
    throw new Error(`${httpErrorMessage(res.statusCode)} — ${fileName}`);
  }
  await fs.promises.mkdir(path.dirname(destFile), { recursive: true });
  const tmp = `${destFile}.part`;
  const hash = crypto.createHash('sha256');
  let received = 0;
  const total = parseInt(res.headers['content-length'] || '0', 10);
  const tap = new PassThrough();
  tap.on('data', (chunk) => {
    hash.update(chunk);
    received += chunk.length;
    if (typeof onBytes === 'function') onBytes(received, total);
  });
  const ws = fs.createWriteStream(tmp);
  try {
    await pipeline(res, tap, ws);
  } catch (e) {
    await fs.promises.unlink(tmp).catch(() => {});
    throw e;
  }
  const dig = hash.digest('hex');
  const exp = String(expectedSha256 || '').toLowerCase();
  if (dig !== exp) {
    await fs.promises.unlink(tmp).catch(() => {});
    throw new Error(`SHA-256 niezgodny po pobraniu: ${fileName}`);
  }
  if (Number.isFinite(expectedSize) && expectedSize >= 0 && received !== expectedSize) {
    await fs.promises.unlink(tmp).catch(() => {});
    throw new Error(`Rozmiar niezgodny po pobraniu: ${fileName} (oczekiwano ${expectedSize}, pobrano ${received})`);
  }
  await fs.promises.rename(tmp, destFile);
}

module.exports = {
  fetchLauncherModsManifestJson,
  downloadLauncherModJar,
  modsApiBaseUrl,
};
