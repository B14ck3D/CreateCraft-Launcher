/**
 * Java Manager — lokalne JRE (Eclipse Temurin) z Adoptium API.
 * Oddzielone od GUI; postęp: onProgress(0–100).
 *
 * Adoptium (przykład Windows x64):
 * https://api.adoptium.net/v3/binary/latest/21/ga/windows/x64/jdk/hotspot/normal/eclipse
 */

const fs = require('fs');
const path = require('path');
const https = require('https');
const http = require('http');
const { spawnSync } = require('child_process');
const { pipeline } = require('stream/promises');
const AdmZip = require('adm-zip');

const JAVA_MAJOR = 21;

function javaExeName() {
  return process.platform === 'win32' ? 'java.exe' : 'java';
}

function adoptiumBinaryUrl() {
  const arch =
    process.arch === 'x64'
      ? 'x64'
      : process.arch === 'arm64'
        ? 'aarch64'
        : 'x64';
  const plat = process.platform;
  if (plat === 'win32') {
    return `https://api.adoptium.net/v3/binary/latest/${JAVA_MAJOR}/ga/windows/${arch}/jdk/hotspot/normal/eclipse`;
  }
  if (plat === 'darwin') {
    return `https://api.adoptium.net/v3/binary/latest/${JAVA_MAJOR}/ga/mac/${arch}/jdk/hotspot/normal/eclipse`;
  }
  return `https://api.adoptium.net/v3/binary/latest/${JAVA_MAJOR}/ga/linux/${arch}/jdk/hotspot/normal/eclipse`;
}

function readMarker(runtimeBase) {
  const mp = path.join(runtimeBase, '.jdk-home');
  if (!fs.existsSync(mp)) return null;
  try {
    return fs.readFileSync(mp, 'utf8').trim();
  } catch {
    return null;
  }
}

function writeMarker(runtimeBase, jdkHome) {
  fs.writeFileSync(path.join(runtimeBase, '.jdk-home'), path.resolve(jdkHome), 'utf8');
}

/**
 * Czy w runtimeBase jest już java (layout: runtimeBase/jdk/bin/java[.exe]).
 * @returns {string|null} absolutna ścieżka do pliku wykonywalnego
 */
function findBundledJavaExecutable(runtimeBase) {
  const fixed = path.join(runtimeBase, 'jdk', 'bin', javaExeName());
  if (fs.existsSync(fixed)) return path.resolve(fixed);
  const marked = readMarker(runtimeBase);
  if (marked) {
    const exe = path.join(marked, 'bin', javaExeName());
    if (fs.existsSync(exe)) return path.resolve(exe);
  }
  return null;
}

/**
 * Po rozpakowaniu: Windows/Linux — jdk-21…/bin/java; macOS — …/Contents/Home/bin/java
 */
function findJdkRootInExtractDir(extractDir) {
  const entries = fs.readdirSync(extractDir, { withFileTypes: true });
  for (const ent of entries) {
    if (!ent.isDirectory()) continue;
    const base = path.join(extractDir, ent.name);
    const direct = path.join(base, 'bin', javaExeName());
    if (fs.existsSync(direct)) return base;
    const macHome = path.join(base, 'Contents', 'Home');
    const macBin = path.join(macHome, 'bin', javaExeName());
    if (fs.existsSync(macBin)) return macHome;
  }
  throw new Error(`Brak bin/${javaExeName()} po rozpakowaniu archiwum JDK.`);
}

function archiveExtensionForPlatform() {
  return process.platform === 'win32' ? '.zip' : '.tar.gz';
}

function extractArchive(archivePath, extractDir) {
  if (archivePath.endsWith('.zip')) {
    const zip = new AdmZip(archivePath);
    zip.extractAllTo(extractDir, true);
    return;
  }
  const r = spawnSync('tar', ['-xzf', archivePath, '-C', extractDir], {
    encoding: 'utf8',
    windowsHide: true,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (r.error) throw new Error(`Rozpakowywanie JDK: ${r.error.message}`);
  if (r.status !== 0) {
    throw new Error(`tar zakończył się kodem ${r.status}: ${(r.stderr || '').trim() || 'nieznany błąd'}`);
  }
}

function followRedirectGet(url, maxRedirects = 12) {
  return new Promise((resolve, reject) => {
    if (maxRedirects < 0) {
      reject(new Error('Zbyt wiele przekierowań HTTP'));
      return;
    }
    const lib = url.startsWith('https:') ? https : http;
    const req = lib.get(
      url,
      { headers: { 'User-Agent': 'createcrafts-launcher-java-manager/1' } },
      (res) => {
        if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
          const next = new URL(res.headers.location, url).href;
          res.resume();
          resolve(followRedirectGet(next, maxRedirects - 1));
          return;
        }
        if (res.statusCode !== 200) {
          res.resume();
          reject(new Error(`Pobieranie JDK: HTTP ${res.statusCode}`));
          return;
        }
        resolve(res);
      }
    );
    req.on('error', reject);
    req.setTimeout(180000, () => {
      req.destroy();
      reject(new Error('Timeout połączenia (brak sieci?)'));
    });
  });
}

/**
 * @param {string} url
 * @param {string} destFile
 * @param {(pct: number) => void} [onProgress] 0–85 podczas pobierania
 */
async function downloadToFile(url, destFile, onProgress) {
  await fs.promises.mkdir(path.dirname(destFile), { recursive: true });
  const tmp = `${destFile}.part`;
  const res = await followRedirectGet(url);
  const total = parseInt(res.headers['content-length'] || '0', 10);
  let received = 0;
  const out = fs.createWriteStream(tmp);
  res.on('data', (chunk) => {
    received += chunk.length;
    if (!onProgress) return;
    if (total > 0) {
      onProgress(Math.min(85, Math.round((received / total) * 85)));
      return;
    }
    const guessBytes = 220 * 1024 * 1024;
    onProgress(Math.min(82, Math.round((received / guessBytes) * 82)));
  });
  await pipeline(res, out);
  await fs.promises.rename(tmp, destFile);
}

class JavaManager {
  /**
   * @param {{ getRuntimeBaseDir: () => string, onProgress?: (n: number) => void }} opts
   */
  constructor(opts) {
    this.getRuntimeBaseDir = opts.getRuntimeBaseDir;
    this.onProgress = typeof opts.onProgress === 'function' ? opts.onProgress : () => {};
  }

  /**
   * Zwraca absolutną ścieżkę do java.exe (Windows) / java (Unix).
   */
  async ensureJava21() {
    const runtimeBase = path.resolve(this.getRuntimeBaseDir());
    await fs.promises.mkdir(runtimeBase, { recursive: true });

    const existing = findBundledJavaExecutable(runtimeBase);
    if (existing) {
      this.onProgress(100);
      return existing;
    }

    const url = adoptiumBinaryUrl();
    const dlDir = path.join(runtimeBase, '__download');
    const ext = archiveExtensionForPlatform();
    const archivePath = path.join(dlDir, `temurin-${JAVA_MAJOR}${ext}`);
    const extractDir = path.join(runtimeBase, '__extract');
    const finalJdk = path.join(runtimeBase, 'jdk');

    try {
      await fs.promises.rm(dlDir, { recursive: true, force: true });
      await fs.promises.rm(extractDir, { recursive: true, force: true });
      await fs.promises.rm(finalJdk, { recursive: true, force: true });

      this.onProgress(0);
      await downloadToFile(url, archivePath, (p) => this.onProgress(p));

      this.onProgress(86);
      await fs.promises.mkdir(extractDir, { recursive: true });
      extractArchive(archivePath, extractDir);
      this.onProgress(95);

      const jdkRoot = findJdkRootInExtractDir(extractDir);
      await fs.promises.rename(jdkRoot, finalJdk);
      writeMarker(runtimeBase, finalJdk);

      await fs.promises.rm(extractDir, { recursive: true, force: true }).catch(() => {});
      await fs.promises.rm(dlDir, { recursive: true, force: true }).catch(() => {});

      const exe = findBundledJavaExecutable(runtimeBase);
      if (!exe) throw new Error('JDK zainstalowany, ale nie znaleziono pliku java.');
      this.onProgress(100);
      return exe;
    } catch (err) {
      await fs.promises.rm(extractDir, { recursive: true, force: true }).catch(() => {});
      await fs.promises.rm(dlDir, { recursive: true, force: true }).catch(() => {});
      throw err;
    }
  }
}

module.exports = {
  JavaManager,
  adoptiumBinaryUrl,
  findBundledJavaExecutable,
  JAVA_MAJOR,
};
