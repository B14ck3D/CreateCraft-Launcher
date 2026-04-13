/**
 * CreateCrafts — synchronizacja modów z katalogu Apache (NeoForge 1.21.1)
 * + pobranie instalatora NeoForge pod MCLC (options.forge).
 */

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');

const pack = require(path.join(__dirname, 'gamePackConstants.json'));

const MODS_INDEX_URLS = [
  'https://createcrafts.pl/mods/',
  'https://www.createcrafts.pl/mods/',
];

/**
 * NeoForge — wersja z `gamePackConstants.json` (Maven …/neoforge/{version}/).
 * Nadpisanie: CREATECRAFT_NEOFORGE_VERSION (lub SUPERSMP_NEOFORGE_VERSION).
 */
const NEOFORGE_INSTALLER_VERSION_DEFAULT = pack.neoForgeInstallerVersion;
const MINECRAFT_VERSION_PACK = pack.minecraftVersion;

/** MCLC zapisuje scalony JSON Forge/NeoForge w forge/{mcVersion}/ — przy zmianie instalatora trzeba czyścić, inaczej zostaje stary (np. 218). */
function clearMclcForgedVersionCache(gameRoot, onLog) {
  const p = path.join(gameRoot, 'forge', MINECRAFT_VERSION_PACK);
  try {
    fs.rmSync(p, { recursive: true, force: true });
    if (onLog) onLog(`[neoforge] Wyczyszczono cache MCLC: ${p}`);
  } catch (e) {
    if (onLog) onLog(`[neoforge] Czyszczenie cache (opcjonalne): ${e.message}`);
  }
}

function neoForgeInstallerMavenUrl(version) {
  const v = version;
  return `https://maven.neoforged.net/releases/net/neoforged/neoforge/${v}/neoforge-${v}-installer.jar`;
}

function cacheDir(gameRoot) {
  const next = path.join(gameRoot, 'launcher-cache');
  const legacy = path.join(gameRoot, '.super-smp-cache');
  try {
    if (!fs.existsSync(next) && fs.existsSync(legacy)) {
      fs.renameSync(legacy, next);
    }
  } catch {
    /* ignore */
  }
  return next;
}

function neoForgeReadyMarkerPath(gameRoot) {
  return path.join(cacheDir(gameRoot), 'neoforge-ready.version');
}

function readNeoForgeReadyMarker(gameRoot) {
  try {
    const p = neoForgeReadyMarkerPath(gameRoot);
    if (!fs.existsSync(p)) return null;
    return fs.readFileSync(p, 'utf8').trim();
  } catch {
    return null;
  }
}

function writeNeoForgeReadyMarker(gameRoot, version) {
  try {
    fs.mkdirSync(cacheDir(gameRoot), { recursive: true });
    fs.writeFileSync(neoForgeReadyMarkerPath(gameRoot), String(version), 'utf8');
  } catch {
    /* ignore */
  }
}

function neoForgeInstallerPath(gameRoot, version) {
  return path.join(cacheDir(gameRoot), `neoforge-${version}-installer.jar`);
}

function resolveNeoForgeInstallerVersion(onLog) {
  const env = (process.env.CREATECRAFT_NEOFORGE_VERSION || process.env.SUPERSMP_NEOFORGE_VERSION)?.trim();
  const v = env || NEOFORGE_INSTALLER_VERSION_DEFAULT;
  if (onLog) {
    if (env) onLog(`[neoforge] Wersja z env: ${v}`);
    else onLog(`[neoforge] Wersja NeoForge (Maven): ${v}`);
  }
  return v;
}

async function pruneOtherNeoForgeInstallers(gameRoot, keepVersion) {
  try {
    const dir = cacheDir(gameRoot);
    const keepName = `neoforge-${keepVersion}-installer.jar`;
    const ents = await fs.promises.readdir(dir, { withFileTypes: true });
    for (const e of ents) {
      if (!e.isFile() || !/^neoforge-21\.1\.\d+-installer\.jar$/i.test(e.name)) continue;
      if (e.name === keepName) continue;
      await fs.promises.unlink(path.join(dir, e.name)).catch(() => {});
    }
  } catch {
    /* ignore */
  }
}

function followRedirectFetch(url, maxRedirects = 12) {
  return new Promise((resolve, reject) => {
    if (maxRedirects < 0) {
      reject(new Error('Zbyt wiele przekierowań'));
      return;
    }
    const lib = url.startsWith('https:') ? https : http;
    const req = lib.get(
      url,
      { headers: { 'User-Agent': 'createcrafts-launcher-modpack/1' } },
      (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          const next = new URL(res.headers.location, url).href;
          res.resume();
          resolve(followRedirectFetch(next, maxRedirects - 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`HTTP ${res.statusCode}`));
          return;
        }
        resolve(res);
      }
    );
    req.on('error', reject);
    req.setTimeout(120000, () => {
      req.destroy();
      reject(new Error('Timeout'));
    });
  });
}

function headContentLength(url) {
  return new Promise((resolve) => {
    const lib = url.startsWith('https:') ? https : http;
    const u = new URL(url);
    const req = lib.request(
      {
        protocol: u.protocol,
        hostname: u.hostname,
        port: u.port || undefined,
        path: u.pathname + u.search,
        method: 'HEAD',
        headers: { 'User-Agent': 'createcrafts-launcher-modpack/1' },
        timeout: 30000,
      },
      (res) => {
        res.resume();
        const n = parseInt(res.headers['content-length'] || '0', 10);
        resolve(Number.isFinite(n) && n > 0 ? n : null);
      }
    );
    req.on('error', () => resolve(null));
    req.on('timeout', () => {
      req.destroy();
      resolve(null);
    });
    req.end();
  });
}

async function downloadToFile(url, destFile, onBytes) {
  await fs.promises.mkdir(path.dirname(destFile), { recursive: true });
  const tmp = `${destFile}.part`;
  const res = await followRedirectFetch(url);
  const total = parseInt(res.headers['content-length'] || '0', 10);
  let received = 0;
  const chunks = [];
  for await (const chunk of res) {
    chunks.push(chunk);
    received += chunk.length;
    if (typeof onBytes === 'function') onBytes(received, total);
  }
  await fs.promises.writeFile(tmp, Buffer.concat(chunks));
  await fs.promises.rename(tmp, destFile);
}

/**
 * Wyciąga nazwy plików .jar z listingu Apache (href="…jar") lub zwykłego HTML.
 * @param {string} html
 * @param {string} baseUrl — z końcowym /
 */
function parseJarLinks(html, baseUrl) {
  const out = new Map();
  const re = /href\s*=\s*["']([^"']+\.jar)["']/gi;
  let m;
  while ((m = re.exec(html))) {
    let href = m[1].trim();
    if (!href || href.includes('..')) continue;
    if (href.startsWith('//')) href = `https:${href}`;
    let full;
    if (href.startsWith('http')) {
      full = href;
    } else {
      full = new URL(href, baseUrl).href;
    }
    const name = decodeURIComponent(path.basename(new URL(full).pathname));
    if (!name.endsWith('.jar') || name.startsWith('.')) continue;
    out.set(name, full);
  }
  return out;
}

async function fetchModsManifest(onLog) {
  let lastErr;
  for (const base of MODS_INDEX_URLS) {
    const baseNorm = base.endsWith('/') ? base : `${base}/`;
    try {
      const res = await followRedirectFetch(baseNorm);
      const chunks = [];
      for await (const c of res) chunks.push(c);
      const html = Buffer.concat(chunks).toString('utf8');
      const map = parseJarLinks(html, baseNorm);
      if (map.size === 0) throw new Error('Brak linków .jar w odpowiedzi');
      if (onLog) onLog(`[mods] Indeks: ${baseNorm} (${map.size} plików)`);
      return { baseUrl: baseNorm, files: map };
    } catch (e) {
      lastErr = e;
      if (onLog) onLog(`[mods] Błąd ${baseNorm}: ${e.message}`);
    }
  }
  throw lastErr || new Error('Nie udało się pobrać listy modów');
}

/**
 * @param {string} gameRoot
 * @param {{ onLog?: (s:string)=>void, onProgress?: (pct:number)=>void, onPhase?: (phase: 'verify'|'download')=>void }} hooks
 */
async function syncCreateCraftsMods(gameRoot, hooks = {}) {
  const onLog = hooks.onLog || (() => {});
  const onProgress = hooks.onProgress || (() => {});
  const onPhase = hooks.onPhase || (() => {});
  const forceRedownload = Boolean(hooks.forceRedownload);

  onPhase('verify');
  onProgress(12);
  const { files } = await fetchModsManifest(onLog);
  const modsDir = path.join(gameRoot, 'mods');
  await fs.promises.mkdir(modsDir, { recursive: true });

  const list = [...files.entries()];
  const total = list.length;
  if (total === 0) {
    onProgress(100);
    onLog('[mods] Pusta lista z serwera.');
    return;
  }

  if (forceRedownload) {
    onLog('[mods] Wymuszona weryfikacja — ponowne pobranie wszystkich plików z listy.');
  }

  onProgress(18);
  const tasks = [];
  for (const [name, fileUrl] of list) {
    const dest = path.join(modsDir, name);
    const remoteLen = await headContentLength(fileUrl);
    let needs = true;
    if (!forceRedownload && fs.existsSync(dest) && remoteLen) {
      const st = await fs.promises.stat(dest);
      if (st.size === remoteLen) needs = false;
    }
    tasks.push({ name, fileUrl, dest, needs, remoteLen });
  }

  const needDl = tasks.filter((t) => t.needs);
  if (needDl.length === 0) {
    onProgress(100);
    onLog(`[mods] Weryfikacja: wszystkie ${total} plików OK (bez pobierania).`);
    return;
  }

  onPhase('download');
  onLog(`[mods] Do pobrania / aktualizacji: ${needDl.length} z ${total}`);
  const concurrency = 3;
  let done = 0;
  const toFetch = needDl.length;

  async function syncOne({ name, fileUrl, dest }) {
    onLog(`[mods] Pobieranie: ${name}`);
    await downloadToFile(fileUrl, dest, () => {});
  }

  for (let offset = 0; offset < needDl.length; offset += concurrency) {
    const slice = needDl.slice(offset, offset + concurrency);
    await Promise.all(
      slice.map(async (t) => {
        try {
          await syncOne(t);
        } catch (e) {
          throw new Error(`Mod ${t.name}: ${e.message || e}`);
        }
      })
    );
    done += slice.length;
    onProgress(Math.min(99, Math.round(20 + (done / toFetch) * 79)));
  }
  onProgress(100);
  onLog(`[mods] Zakończono: ${total} plików w ${modsDir}`);
}

/**
 * Lista modów z serwera + status lokalny (UI).
 * @param {string} gameRoot
 * @param {(s:string)=>void} [onLog]
 */
async function getCreateCraftsModsListForUi(gameRoot, onLog) {
  const log = onLog || (() => {});
  const { files, baseUrl } = await fetchModsManifest(log);
  const modsDir = path.join(gameRoot, 'mods');
  await fs.promises.mkdir(modsDir, { recursive: true });
  const mods = [];
  for (const [name, fileUrl] of files) {
    const dest = path.join(modsDir, name);
    let localSize = null;
    /** @type {'ok'|'missing'|'mismatch'|'unknown'} */
    let status = 'unknown';
    try {
      const remoteLen = await headContentLength(fileUrl);
      if (fs.existsSync(dest)) {
        const st = await fs.promises.stat(dest);
        localSize = st.size;
        if (remoteLen) {
          status = st.size === remoteLen ? 'ok' : 'mismatch';
        } else {
          status = 'unknown';
        }
      } else {
        status = 'missing';
      }
    } catch {
      status = 'unknown';
    }
    mods.push({ name, status, localSize, fileUrl });
  }
  return {
    gameRoot,
    modsDir,
    baseUrl,
    count: mods.length,
    mods,
  };
}

/**
 * Pobiera instalator NeoForge (JAR) do cache pod MCLC `options.forge`.
 */
async function ensureNeoForgeInstallerJar(gameRoot, onLog) {
  const version = resolveNeoForgeInstallerVersion(onLog);
  const dest = neoForgeInstallerPath(gameRoot, version);
  await fs.promises.mkdir(cacheDir(gameRoot), { recursive: true });
  const url = neoForgeInstallerMavenUrl(version);
  if (fs.existsSync(dest)) {
    const st = await fs.promises.stat(dest);
    if (st.size > 100000) {
      if (onLog) onLog(`[neoforge] Instalator już w cache: ${dest}`);
      await pruneOtherNeoForgeInstallers(gameRoot, version);
      return dest;
    }
  }
  if (onLog) onLog(`[neoforge] Pobieranie instalatora ${version}…`);
  await downloadToFile(url, dest);
  await pruneOtherNeoForgeInstallers(gameRoot, version);
  if (onLog) onLog(`[neoforge] Zapisano: ${dest}`);
  return dest;
}

/**
 * Mody + instalator NeoForge (jedna funkcja z main).
 * Cache MCLC `forge/<mc>/` czyścimy tylko gdy zmieni się wersja instalatora NeoForge.
 * @returns {Promise<{ neoForgeInstallerPath: string }>}
 */
async function ensureCreateCraftsModPack(gameRoot, hooks = {}) {
  const onLog = hooks.onLog || (() => {});
  const version = resolveNeoForgeInstallerVersion(onLog);
  const prev = readNeoForgeReadyMarker(gameRoot);
  if (prev !== version) {
    clearMclcForgedVersionCache(gameRoot, onLog);
    if (onLog) onLog(`[neoforge] Nowa wersja instalatora (${version}), czyszczenie cache profilu Forge.`);
  } else if (onLog) {
    onLog(`[neoforge] Ta sama wersja (${version}) — pomijam czyszczenie cache MCLC.`);
  }
  hooks.onProgress?.(8);
  const neoPath = await ensureNeoForgeInstallerJar(gameRoot, onLog);
  await syncCreateCraftsMods(gameRoot, hooks);
  writeNeoForgeReadyMarker(gameRoot, version);
  return { neoForgeInstallerPath: neoPath };
}

module.exports = {
  ensureCreateCraftsModPack,
  syncCreateCraftsMods,
  ensureNeoForgeInstallerJar,
  resolveNeoForgeInstallerVersion,
  clearMclcForgedVersionCache,
  getCreateCraftsModsListForUi,
  NEOFORGE_INSTALLER_VERSION_DEFAULT,
  MINECRAFT_VERSION_PACK,
  MODS_INDEX_URLS,
};
