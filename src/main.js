// Electron main — okno, IPC, MCLC.
const { app, BrowserWindow, ipcMain, shell } = require('electron');

const gotTheLock = app.requestSingleInstanceLock();
if (!gotTheLock) {
  process.exit(0);
}

let mainWindow = null;

app.on('second-instance', () => {
  if (mainWindow && !mainWindow.isDestroyed()) {
    if (mainWindow.isMinimized()) mainWindow.restore();
    mainWindow.show();
    mainWindow.focus();
  }
});

const path = require('path');
const fs = require('fs');
const crypto = require('crypto');
const os = require('os');
const child = require('child_process');
const { spawnSync, execSync } = child;
const { Auth, tokenUtils } = require('msmc');

const { ensureDefaultServerInServersDatSync } = require('./serversDat');
const MCLCInner = require('minecraft-launcher-core/components/launcher');
MCLCInner.prototype.startMinecraft = function patchedStartMinecraft(launchArguments) {
  const gameRootForServers = path.resolve(this.options.overrides?.cwd || this.options.root);
  try {
    const logPath = path.join(gameRootForServers, 'createcrafts-launcher.log');
    const onLog = (line) => {
      try {
        fs.appendFileSync(
          logPath,
          `[${new Date().toISOString()}] ${String(line).replace(/\r?\n/g, ' ')}\n`
        );
      } catch {
      }
    };
    ensureDefaultServerInServersDatSync(gameRootForServers, onLog);
  } catch (e) {
    try {
      const logPath = path.join(gameRootForServers, 'createcrafts-launcher.log');
      fs.appendFileSync(
        logPath,
        `[${new Date().toISOString()}] [servers.dat] przed spawnem MC: ${e}\n`
      );
    } catch {
    }
  }
  const minecraft = child.spawn(
    this.options.javaPath ? this.options.javaPath : 'java',
    launchArguments,
    {
      cwd: this.options.overrides.cwd || this.options.root,
      detached: this.options.overrides.detached,
      windowsHide: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    }
  );
  minecraft.stdout.on('data', (data) => this.emit('data', data.toString('utf-8')));
  minecraft.stderr.on('data', (data) => this.emit('data', data.toString('utf-8')));
  minecraft.on('close', (code) => this.emit('close', code));
  return minecraft;
};

const { Client, Authenticator } = require('minecraft-launcher-core');
const { JavaManager, JAVA_MAJOR, findBundledJavaExecutable } = require('./javaManager');
const { ensureCreateCraftsModPack, getCreateCraftsModsListForUi } = require('./createCraftsModsSync');
const { savePremiumMclc, loadPremiumMclc, deletePremiumMclc, migrateProfilesArray } = require('./profileStore');

function normalizeUuidToHex32(uuid) {
  const s = String(uuid || '')
    .replace(/-/g, '')
    .toLowerCase();
  if (s.length !== 32 || !/^[0-9a-f]{32}$/.test(s)) return null;
  return s;
}

function mineatarFaceUrlFromUuidHex32(hex32) {
  return `https://api.mineatar.io/face/${hex32}?scale=4`;
}

function offlinePlayerUuidFromName(playerName) {
  const s = `OfflinePlayer:${String(playerName)}`;
  const md5 = crypto.createHash('md5').update(s, 'utf8').digest();
  const b = Buffer.from(md5);
  b[6] = (b[6] & 0x0f) | 0x30;
  b[8] = (b[8] & 0x3f) | 0x80;
  const hex = b.toString('hex');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
}

function getDefaultGameRoot() {
  const appData = app.getPath('appData');
  const next = path.join(appData, 'CreateCrafts');
  const legacy = path.join(appData, '.super-smp-client');
  try {
    if (!fs.existsSync(next) && fs.existsSync(legacy)) {
      fs.renameSync(legacy, next);
    }
  } catch (e) {
    console.warn('Migracja folderu gry (.super-smp-client → CreateCrafts):', e.message);
  }
  return next;
}

const FORCE_MODS_RESYNC_FLAG = 'createcrafts-force-mods-resync.flag';
function getForceModsResyncFlagPath() {
  return path.join(getDefaultGameRoot(), FORCE_MODS_RESYNC_FLAG);
}

function windowsJvmOsSpoofFlags() {
  if (process.platform !== 'win32') return [];
  const ver = String(os.release() || '');
  if (!ver.startsWith('10.')) return [];
  return ['-Dos.name=Windows 10', '-Dos.version=10.0'];
}
function minecraftAppletTargetDirFlag(gameDirectory) {
  return [`-Dminecraft.applet.TargetDirectory=${path.resolve(gameDirectory)}`];
}
function serverConnectProgramArgs(serverHost, serverPort) {
  const host = String(serverHost || '').trim();
  if (!host) return [];
  const port = String(serverPort != null ? serverPort : '25565').trim();
  if (port && port !== '25565') {
    return ['--port', port, '--server', host];
  }
  return ['--server', host];
}
function jvmPerformanceArgs() {
  const cpus = Math.max(2, os.cpus().length);
  return [
    '-XX:+UseG1GC',
    '-XX:+ParallelRefProcEnabled',
    '-XX:MaxGCPauseMillis=200',
    `-XX:ConcGCThreads=${Math.min(8, Math.max(2, Math.ceil(cpus / 2)))}`,
    `-XX:ParallelGCThreads=${cpus}`,
  ];
}

function buildMclcJvmAugments({ gameRoot, serverHost, serverPort, modBranding }) {
  const root = path.resolve(gameRoot);
  const brand =
    modBranding ?
      ['-Dminecraft.launcher.brand=SuperSMP-Launcher', '-Dminecraft.launcher.name=CreateCrafts (NeoForge)']
    : [];
  const customArgs = [
    ...jvmPerformanceArgs(),
    ...brand,
    ...windowsJvmOsSpoofFlags(),
    ...minecraftAppletTargetDirFlag(root),
  ];
  const customLaunchArgs = serverConnectProgramArgs(serverHost, serverPort);
  return { customArgs, customLaunchArgs };
}

async function ensurePremiumAuthForMclc(storedMclc) {
  if (!storedMclc || typeof storedMclc.access_token !== 'string') {
    throw new Error('Brak tokena premium — zaloguj się przez Microsoft.');
  }
  if (!storedMclc.uuid || !storedMclc.name) {
    throw new Error('Niekompletny profil (uuid / nick). Zaloguj się ponownie przez Microsoft.');
  }
  const auth = new Auth('select_account');
  const mc = tokenUtils.fromMclcToken(auth, storedMclc, false);
  if (!mc.validate()) {
    if (!storedMclc.meta?.refresh) {
      throw new Error(
        'Sesja wygasła (Invalid session). Zaloguj się ponownie przez Microsoft — ten profil został zapisany bez odświeżania tokenu.'
      );
    }
    await mc.refresh(true);
  }
  const fresh = mc.mclc(true);
  if (!fresh.user_properties) fresh.user_properties = {};
  if (!fresh.meta) fresh.meta = { type: 'msa' };
  if (!fresh.meta.type) fresh.meta.type = 'msa';
  return fresh;
}

const MC_VERSION = '1.21.1';
const MIN_JAVA_MAJOR = 21;

function getBundledJavaRuntimeRoot() {
  return path.join(app.getPath('userData'), 'runtime', `java${JAVA_MAJOR}`);
}

function javaVersionText(javaExe) {
  const r = spawnSync(javaExe, ['-version'], { encoding: 'utf8', windowsHide: true });
  return `${r.stdout || ''}${r.stderr || ''}`;
}

function javaMajorFromText(text) {
  const legacy = text.match(/version "1\.(\d+)\./);
  if (legacy) return parseInt(legacy[1], 10);
  const modern = text.match(/version "(\d+)\./);
  if (modern) return parseInt(modern[1], 10);
  return 0;
}

function javaMajor(javaExe) {
  try {
    const t = javaVersionText(javaExe);
    if (!t.trim()) return 0;
    return javaMajorFromText(t);
  } catch {
    return 0;
  }
}

function collectJavaCandidates() {
  const out = new Set();
  const push = (p) => {
    if (!p) return;
    try {
      if (fs.existsSync(p)) out.add(path.resolve(p));
    } catch {
    }
  };

  if (process.env.JAVA_HOME) {
    const base = path.join(process.env.JAVA_HOME, 'bin');
    if (process.platform === 'win32') {
      push(path.join(base, 'javaw.exe'));
      push(path.join(base, 'java.exe'));
    } else {
      push(path.join(base, 'java'));
    }
  }

  if (process.platform === 'win32') {
    const pf = process.env.ProgramFiles || 'C:\\Program Files';
    const pfx86 = process.env['ProgramFiles(x86)'] || 'C:\\Program Files (x86)';
    for (const root of [
      path.join(pf, 'Java'),
      path.join(pf, 'Eclipse Adoptium'),
      path.join(pf, 'Microsoft'),
      path.join(pfx86, 'Java'),
    ]) {
      if (!fs.existsSync(root)) continue;
      let entries;
      try {
        entries = fs.readdirSync(root, { withFileTypes: true });
      } catch {
        continue;
      }
      for (const ent of entries) {
        if (!ent.isDirectory()) continue;
        push(path.join(root, ent.name, 'bin', 'javaw.exe'));
        push(path.join(root, ent.name, 'bin', 'java.exe'));
      }
    }
    for (const cmd of ['javaw', 'java']) {
      try {
        execSync(`where ${cmd}`, { encoding: 'utf8', shell: true, windowsHide: true })
          .trim()
          .split(/\r?\n/)
          .map((s) => s.trim())
          .filter(Boolean)
          .forEach(push);
      } catch {
      }
    }
  } else {
    try {
      push(execSync('command -v java', { encoding: 'utf8' }).trim());
    } catch {
    }
  }

  return [...out];
}

function resolveJavaPath() {
  const candidates = collectJavaCandidates();
  let best = null;
  let bestMajor = -1;
  for (const exe of candidates) {
    const m = javaMajor(exe);
    if (m >= MIN_JAVA_MAJOR && m > bestMajor) {
      bestMajor = m;
      best = exe;
    }
  }
  if (best) return { javaPath: best, major: bestMajor };
  for (const exe of candidates) {
    const m = javaMajor(exe);
    if (m > bestMajor) {
      bestMajor = m;
      best = exe;
    }
  }
  return { javaPath: best || (process.platform === 'win32' ? 'javaw' : 'java'), major: Math.max(0, bestMajor) };
}

function getWindowIconPath() {
  try {
    if (app.isPackaged) {
      const inRes = path.join(process.resourcesPath, 'icon.png');
      if (fs.existsSync(inRes)) return inRes;
      return undefined;
    }
    const dev = path.join(__dirname, '../../public/icon.png');
    return fs.existsSync(dev) ? dev : undefined;
  } catch {
    return undefined;
  }
}

const createWindow = () => {
  mainWindow = new BrowserWindow({
    width: 1100,
    height: 700,
    frame: false,
    backgroundColor: '#09090b',
    icon: getWindowIconPath(),
    webPreferences: {
      preload: path.join(__dirname, 'preload.js'),
      contextIsolation: true,
      nodeIntegration: false
    },
  });

  let devOrigin = '';
  try {
    const devUrl =
      typeof MAIN_WINDOW_VITE_DEV_SERVER_URL === 'string' ? MAIN_WINDOW_VITE_DEV_SERVER_URL : '';
    if (devUrl) devOrigin = new URL(devUrl).origin;
  } catch {
  }

  const wc = mainWindow.webContents;
  wc.setWindowOpenHandler(({ url }) => {
    try {
      const parsed = new URL(url);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
        return { action: 'deny' };
      }
      if (devOrigin && parsed.origin === devOrigin) {
        return { action: 'allow' };
      }
      shell.openExternal(url);
    } catch {
    }
    return { action: 'deny' };
  });

  wc.on('will-navigate', (event, navigationUrl) => {
    try {
      const parsed = new URL(navigationUrl);
      if (parsed.protocol !== 'http:' && parsed.protocol !== 'https:') {
        return;
      }
      if (devOrigin && parsed.origin === devOrigin) {
        return;
      }
      event.preventDefault();
      shell.openExternal(navigationUrl);
    } catch {
    }
  });

  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    mainWindow.loadFile(path.join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`));
  }
};

app.on('ready', createWindow);

app.on('window-all-closed', () => {
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

ipcMain.on('window-minimize', () => mainWindow?.minimize());
ipcMain.on('window-maximize', () => {
  if (mainWindow?.isMaximized()) mainWindow.unmaximize();
  else mainWindow?.maximize();
});
ipcMain.on('window-close', () => mainWindow?.close());

ipcMain.handle('createcrafts-mods-info', async () => {
  const gameRoot = getDefaultGameRoot();
  try {
    fs.mkdirSync(gameRoot, { recursive: true });
  } catch {
  }
  const userDataDir = app.getPath('userData');
  try {
    const data = await getCreateCraftsModsListForUi(gameRoot, () => {}, { userDataDir });
    return { ok: true, ...data };
  } catch (e) {
    return {
      ok: false,
      error: String(e?.message || e),
      gameRoot,
      modsDir: path.join(gameRoot, 'mods'),
      baseUrl: '',
      count: 0,
      mods: [],
    };
  }
});

ipcMain.handle('createcrafts-force-mod-resync-next', async () => {
  const gameRoot = getDefaultGameRoot();
  try {
    fs.mkdirSync(gameRoot, { recursive: true });
    fs.writeFileSync(getForceModsResyncFlagPath(), new Date().toISOString(), 'utf8');
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e?.message || e) };
  }
});

ipcMain.handle('createcrafts-force-mod-resync-pending', async () => {
  try {
    return { pending: fs.existsSync(getForceModsResyncFlagPath()) };
  } catch {
    return { pending: false };
  }
});

ipcMain.handle('open-path-in-explorer', async (_e, dirPath) => {
  const p = String(dirPath || '').trim();
  if (!p) return { ok: false, error: 'Brak ścieżki' };
  try {
    await fs.promises.mkdir(p, { recursive: true });
  } catch {
  }
  const err = await shell.openPath(p);
  return err ? { ok: false, error: err } : { ok: true };
});

ipcMain.handle('open-external-url', async (_e, url) => {
  const u = String(url || '').trim();
  if (!/^https?:\/\//i.test(u)) {
    return { ok: false, error: 'Dozwolone są tylko adresy http(s).' };
  }
  try {
    await shell.openExternal(u);
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e?.message || e) };
  }
});

ipcMain.handle('profiles-migrate-localstorage', async (_e, payload) => {
  try {
    const rawJson = typeof payload === 'string' ? payload : payload?.rawJson;
    const lastProfileId = typeof payload === 'object' && payload ? payload.lastProfileId : null;
    const arr = JSON.parse(String(rawJson || '[]'));
    if (!Array.isArray(arr)) return { ok: false, error: 'profiles musi być tablicą' };
    const { profiles, changed, newLastProfileId } = migrateProfilesArray(arr, lastProfileId);
    return {
      ok: true,
      profilesJson: JSON.stringify(profiles),
      changed,
      newLastProfileId: newLastProfileId || null,
    };
  } catch (e) {
    return { ok: false, error: String(e?.message || e) };
  }
});

ipcMain.handle('profiles-delete-premium', async (_e, profileId) => {
  try {
    deletePremiumMclc(String(profileId || ''));
    return { ok: true };
  } catch (e) {
    return { ok: false, error: String(e?.message || e) };
  }
});

ipcMain.handle('mineatar-face-url', async (_e, payload) => {
  const rawUuid = String(payload?.uuid || '').trim();
  const offlineName = String(payload?.offlineName || '').trim();
  let canonical = null;
  if (rawUuid) {
    const hex = normalizeUuidToHex32(rawUuid);
    if (hex) canonical = `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20, 32)}`;
  } else if (offlineName) {
    canonical = offlinePlayerUuidFromName(offlineName);
  }
  if (!canonical) return { url: null, playerUuid: null };
  const hex32 = normalizeUuidToHex32(canonical);
  if (!hex32) return { url: null, playerUuid: null };
  return {
    url: mineatarFaceUrlFromUuidHex32(hex32),
    playerUuid: canonical,
  };
});

ipcMain.handle('login-microsoft', async () => {
  try {
    const authManager = new Auth("select_account");
    const xboxManager = await authManager.launch("electron");
    const token = await xboxManager.getMinecraft();
    const mcToken = token.mclc(true);
    if (!mcToken.uuid) {
      throw new Error('Brak UUID profilu Minecraft — zaloguj się ponownie.');
    }
    savePremiumMclc(mcToken.uuid, mcToken);

    const hex32 = normalizeUuidToHex32(mcToken.uuid);
    const avatar = hex32 ? mineatarFaceUrlFromUuidHex32(hex32) : '';

    return {
      id: mcToken.uuid,
      name: mcToken.name,
      type: "premium",
      label: mcToken.name,
      avatar,
    };
  } catch (error) {
    console.error("Błąd logowania Microsoft:", error);
    throw error;
  }
});

ipcMain.on('start-game', async (event, authData) => {
  const gameRoot = getDefaultGameRoot();
  try {
    fs.mkdirSync(gameRoot, { recursive: true });
  } catch {
  }
  const launchLogPath = path.join(gameRoot, 'createcrafts-launcher.log');
  const appendLaunchLog = (msg) => {
    try {
      fs.appendFileSync(launchLogPath, `[${new Date().toISOString()}] ${String(msg).replace(/\r?\n/g, ' ')}\n`);
    } catch {
    }
  };

  const launcher = new Client();
  const debugLines = [];
  const pushDebug = (msg) => {
    const line = String(msg).trim();
    if (!line) return;
    debugLines.push(line);
    if (debugLines.length > 80) debugLines.shift();
  };

  launcher.on('debug', (m) => {
    console.log(m);
    pushDebug(m);
    appendLaunchLog(m);
    event.sender.send('launcher-log', m);
  });

  launcher.once('package-extract', () => {
    try {
      ensureDefaultServerInServersDatSync(gameRoot, appendLaunchLog);
    } catch (e) {
      appendLaunchLog(`[servers.dat] package-extract: ${e?.message || e}`);
    }
  });
  launcher.once('arguments', () => {
    try {
      ensureDefaultServerInServersDatSync(gameRoot, appendLaunchLog);
    } catch (e) {
      appendLaunchLog(`[servers.dat] arguments: ${e?.message || e}`);
    }
  });

  try {
    event.sender.send('launcher-state', 'verifying');

    const launchPayload = authData || {};
    let authorization;
    if (launchPayload.type === 'offline') {
      const nick = String(launchPayload.offlineName || launchPayload.name || '').trim();
      if (!nick) throw new Error('Brak nicku offline');
      authorization = Authenticator.getAuth(nick);
    } else if (launchPayload.type === 'premium') {
      const sid = String(launchPayload.profileId || '').trim();
      if (!sid) throw new Error('Brak profileId — zaloguj się ponownie przez Microsoft.');
      const stored = loadPremiumMclc(sid);
      if (!stored) {
        throw new Error('Brak zapisanej sesji premium — zaloguj się ponownie przez Microsoft.');
      }
      try {
        authorization = await ensurePremiumAuthForMclc(stored);
        savePremiumMclc(authorization.uuid || sid, authorization);
      } catch (e) {
        appendLaunchLog(`Premium auth: ${e}`);
        throw e;
      }
    } else {
      throw new Error("Brak prawidłowych danych autoryzacji");
    }

    const runtimeRoot = getBundledJavaRuntimeRoot();
    let javaPath = null;
    let major = 0;
    const cachedJava = findBundledJavaExecutable(runtimeRoot);
    if (cachedJava) {
      major = javaMajor(cachedJava);
      if (major >= MIN_JAVA_MAJOR) {
        javaPath = cachedJava;
        event.sender.send('launcher-progress', 100);
      }
    }

    if (!javaPath) {
      event.sender.send('launcher-state', 'java-download');
      event.sender.send('launcher-progress', 0);
      try {
        const jm = new JavaManager({
          getRuntimeBaseDir: () => getBundledJavaRuntimeRoot(),
          onProgress: (p) => event.sender.send('launcher-progress', p),
        });
        javaPath = await jm.ensureJava21();
        major = javaMajor(javaPath);
        if (major < MIN_JAVA_MAJOR) {
          throw new Error(`Lokalne JDK zgłasza wersję ${major || '?'}`);
        }
      } catch (e) {
        appendLaunchLog(`JavaManager (Adoptium / lokalne JDK): ${e}`);
        const resolved = resolveJavaPath();
        javaPath = resolved.javaPath;
        major = resolved.major;
      }
    }

    if (major < MIN_JAVA_MAJOR) {
      const hint =
        `Znaleziono Java ${major || '?'}, a Minecraft ${MC_VERSION} wymaga JDK ${MIN_JAVA_MAJOR}.\n` +
        `Launcher próbował pobrać JDK ${MIN_JAVA_MAJOR} do folderu:\n${getBundledJavaRuntimeRoot()}\n` +
        `Sprawdź połączenie z internetem lub ustaw JAVA_HOME na JDK ${MIN_JAVA_MAJOR}.\n` +
        `Ostatnia próba ścieżki: ${javaPath}\n\nPełny log: ${launchLogPath}`;
      appendLaunchLog(`BŁĄD: ${hint}`);
      event.sender.send('launcher-crash', hint);
      event.sender.send('launcher-state', 'idle');
      return;
    }

    const useCreateCraftsModpack =
      process.env.CREATECRAFT_DISABLE_MODPACK !== '1' && process.env.SUPERSMP_DISABLE_MODPACK !== '1';
    let neoForgeInstallerPath = null;
    if (useCreateCraftsModpack) {
      let forceModsResync = false;
      try {
        forceModsResync = fs.existsSync(getForceModsResyncFlagPath());
      } catch {
        forceModsResync = false;
      }
      event.sender.send('launcher-state', 'checking-files');
      event.sender.send('launcher-progress', 5);
      try {
        const pack = await ensureCreateCraftsModPack(gameRoot, {
          onLog: appendLaunchLog,
          onProgress: (p) => event.sender.send('launcher-progress', p),
          onPhase: (phase) => {
            if (phase === 'verify') {
              event.sender.send('launcher-state', 'checking-files');
            } else if (phase === 'download') {
              event.sender.send('launcher-state', 'mods-sync');
            }
          },
          forceRedownload: forceModsResync,
          userDataDir: app.getPath('userData'),
        });
        if (forceModsResync) {
          try {
            fs.unlinkSync(getForceModsResyncFlagPath());
          } catch {
          }
        }
        neoForgeInstallerPath = pack.neoForgeInstallerPath;
      } catch (e) {
        appendLaunchLog(`CreateCrafts modpack: ${e}`);
        event.sender.send(
          'launcher-crash',
          `Nie udało się zsynchronizować modów CreateCrafts.\n${e}\n\nPełny log: ${launchLogPath}`
        );
        event.sender.send('launcher-state', 'idle');
        return;
      }
    }

    event.sender.send('launcher-state', 'downloading');

    const logHistory = [];
    const addLog = (msg) => {
      const text = String(msg).trim();
      if (text) {
        logHistory.push(text);
        if (logHistory.length > 40) logHistory.shift();
      }
    };

    launcher.on('progress', (e) => {
      if (e.task && e.total) {
        const percentage = (e.task / e.total) * 100;
        event.sender.send('launcher-progress', percentage);
      }
    });

    launcher.on('package-extract', () => {
      event.sender.send('launcher-state', 'launching');
    });

    launcher.on('data', (e) => {
      console.log(e);
      addLog(e);
      appendLaunchLog(`[MC] ${e}`);
      event.sender.send('launcher-log', e);
    });

    launcher.on('arguments', () => {
      event.sender.send('launcher-state', 'connected');
    });

    launcher.on('close', (code) => {
      appendLaunchLog(`Proces Minecraft zakończony kodem ${code}`);
      if (code !== 0) {
        const body = logHistory.length ? logHistory.join('\n') : debugLines.slice(-25).join('\n');
        appendLaunchLog(`Crash / kod ${code}: ${(body || '').slice(0, 2000)}`);
        event.sender.send(
          'launcher-crash',
          (body || `Proces Minecraft zakończył się kodem ${code}.`) + `\n\nPlik logu: ${launchLogPath}`
        );
      }
      event.sender.send('launcher-state', 'idle');
    });

    const serverHost =
      process.env.CREATECRAFT_SERVER_HOST || process.env.SUPERSMP_SERVER_HOST || 'main.createcrafts.pl';
    const serverPort =
      process.env.CREATECRAFT_SERVER_PORT || process.env.SUPERSMP_SERVER_PORT || '25565';

    const tlAugment = buildMclcJvmAugments({
      gameRoot,
      serverHost,
      serverPort,
      modBranding: useCreateCraftsModpack,
    });

    appendLaunchLog(
      `Start gry ${MC_VERSION} (${useCreateCraftsModpack ? 'NeoForge + CreateCrafts mods' : 'vanilla'}) java=${javaPath} serwer=${serverHost}:${serverPort}`
    );

    ensureDefaultServerInServersDatSync(gameRoot, appendLaunchLog);

    const opts = {
      clientPackage: null,
      authorization,
      javaPath,
      root: gameRoot,
      ...(useCreateCraftsModpack && neoForgeInstallerPath ? { forge: neoForgeInstallerPath } : {}),
      version: {
        number: MC_VERSION,
        type: "release"
      },
      memory: {
        max: launchPayload.ramSize || '6G',
        min: '1G',
      },
      customArgs: tlAugment.customArgs,
      customLaunchArgs: tlAugment.customLaunchArgs,
      overrides: {
        detached: false,
        cwd: path.resolve(gameRoot),
      },
    };

    const childProc = await launcher.launch(opts);
    if (!childProc) {
      const tail = debugLines.slice(-30).join('\n');
      appendLaunchLog('launch() zwrócił null');
      event.sender.send(
        'launcher-crash',
        (tail ||
          'Nie udało się uruchomić (launch zwrócił null). Sprawdź log MCLC.') + `\n\nPlik logu: ${launchLogPath}`
      );
      event.sender.send('launcher-state', 'idle');
    }
  } catch (error) {
    console.error("Błąd podczas uruchamiania gry:", error);
    appendLaunchLog(`Wyjątek: ${error}`);
    const extra = debugLines.length ? `\n\n--- log MCLC ---\n${debugLines.slice(-25).join('\n')}` : '';
    event.sender.send('launcher-crash', String(error) + extra + `\n\nPlik logu: ${launchLogPath}`);
    event.sender.send('launcher-state', 'idle');
  }
});
