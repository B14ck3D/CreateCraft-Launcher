import React, { useState, useEffect, useCallback } from 'react';
import {
  Play,
  Loader2,
  Globe,
  Minus,
  Square,
  X,
  ShieldCheck,
  User,
  Key,
  LogOut,
  Trash2,
  Cpu,
  RefreshCw,
  FolderOpen,
  Sparkles,
} from 'lucide-react';
import gamePack from './gamePackConstants.json';
import SteampunkNavbar from './components/forge/SteampunkNavbar.jsx';
import SteampunkFooter from './components/forge/SteampunkFooter.jsx';
import LauncherMainPanel from './components/forge/LauncherMainPanel.jsx';
import GearDecoration from './components/forge/GearDecoration.jsx';
import CogwheelFrame from './components/forge/CogwheelFrame.jsx';

const pub = (file) => `${import.meta.env.BASE_URL}${file}`;

const PROFILES_STORAGE_KEY = 'createcraft_profiles_v1';
const LAST_PROFILE_STORAGE_KEY = 'createcraft_last_profile_id';
const PROFILES_LEGACY_KEY = 'supersmp_profiles_v1';
const LAST_PROFILE_LEGACY_KEY = 'supersmp_last_profile_id';
const LAUNCHER_SETTINGS_KEY = 'createcraft_launcher_settings_v1';

const ADOPTIUM_JDK21_URL =
  'https://adoptium.net/temurin/releases/?package=jdk&version=21';

function clampRamGb(n) {
  const x = typeof n === 'number' ? n : parseInt(String(n), 10);
  if (!Number.isFinite(x)) return 6;
  return Math.min(32, Math.max(2, Math.round(x)));
}

function loadLauncherSettings() {
  try {
    const raw = localStorage.getItem(LAUNCHER_SETTINGS_KEY);
    if (!raw) return { ramGb: 6 };
    const o = JSON.parse(raw);
    return { ramGb: clampRamGb(o.ramGb ?? 6) };
  } catch {
    return { ramGb: 6 };
  }
}

function saveLauncherSettings(partial) {
  const cur = loadLauncherSettings();
  const next = {
    ...cur,
    ...partial,
    ramGb: clampRamGb(partial.ramGb ?? cur.ramGb),
  };
  localStorage.setItem(LAUNCHER_SETTINGS_KEY, JSON.stringify(next));
}

function loadStoredProfiles() {
  try {
    let raw = localStorage.getItem(PROFILES_STORAGE_KEY);
    if (!raw && localStorage.getItem(PROFILES_LEGACY_KEY)) {
      raw = localStorage.getItem(PROFILES_LEGACY_KEY);
      localStorage.setItem(PROFILES_STORAGE_KEY, raw);
      localStorage.removeItem(PROFILES_LEGACY_KEY);
    }
    if (!raw) return [];
    const parsed = JSON.parse(raw);
    return Array.isArray(parsed) ? parsed : [];
  } catch {
    return [];
  }
}

function saveStoredProfiles(profiles) {
  localStorage.setItem(PROFILES_STORAGE_KEY, JSON.stringify(profiles));
}

function setLastProfileId(id) {
  if (id) localStorage.setItem(LAST_PROFILE_STORAGE_KEY, id);
  else localStorage.removeItem(LAST_PROFILE_STORAGE_KEY);
}

function persistNewOrUpdatedProfile(userPayload) {
  const { token: _t, ...rest } = userPayload;
  const list = loadStoredProfiles();
  const idx = list.findIndex((p) => p.id === rest.id);
  if (idx >= 0) list[idx] = rest;
  else list.push(rest);
  saveStoredProfiles(list);
  setLastProfileId(rest.id);
}

export default function App() {
  const [introFinished, setIntroFinished] = useState(false);
  const [user, setUser] = useState(null);
  const [activeTab, setActiveTab] = useState('home');
  const [connectionState, setConnectionState] = useState('idle');
  const [progress, setProgress] = useState(0);
  const [ramSize, setRamSize] = useState(() => loadLauncherSettings().ramGb);
  const [launchError, setLaunchError] = useState(null);
  const [forceModResyncPending, setForceModResyncPending] = useState(false);
  const [forceModResyncNotice, setForceModResyncNotice] = useState(null);

  useEffect(() => {
    const blockReloadShortcuts = (e) => {
      const t = e.target;
      const tag = t && t.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || t?.isContentEditable) return;

      if (e.key === 'F5' || e.code === 'F5') {
        e.preventDefault();
        return;
      }
      const isR = e.key === 'r' || e.key === 'R';
      if ((e.ctrlKey || e.metaKey) && e.shiftKey && isR) {
        e.preventDefault();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && isR) {
        e.preventDefault();
      }
    };
    window.addEventListener('keydown', blockReloadShortcuts, { capture: true });
    return () => window.removeEventListener('keydown', blockReloadShortcuts, { capture: true });
  }, []);

  useEffect(() => {
    const timer = setTimeout(() => {
      setIntroFinished(true);
    }, 1200); 
    return () => clearTimeout(timer);
  }, []);

  useEffect(() => {
    if (!introFinished) return;
    let cancelled = false;
    (async () => {
      if (!localStorage.getItem(LAST_PROFILE_STORAGE_KEY) && localStorage.getItem(LAST_PROFILE_LEGACY_KEY)) {
        localStorage.setItem(LAST_PROFILE_STORAGE_KEY, localStorage.getItem(LAST_PROFILE_LEGACY_KEY));
        localStorage.removeItem(LAST_PROFILE_LEGACY_KEY);
      }
      const rawProfiles = localStorage.getItem(PROFILES_STORAGE_KEY);
      if (rawProfiles && window.launcher?.migrateProfilesFromLocalStorage) {
        try {
          const lastIdBefore = localStorage.getItem(LAST_PROFILE_STORAGE_KEY);
          const r = await window.launcher.migrateProfilesFromLocalStorage({
            rawJson: rawProfiles,
            lastProfileId: lastIdBefore,
          });
          if (!cancelled && r?.ok && typeof r.profilesJson === 'string') {
            localStorage.setItem(PROFILES_STORAGE_KEY, r.profilesJson);
            if (typeof r.newLastProfileId === 'string' && r.newLastProfileId) {
              setLastProfileId(r.newLastProfileId);
            }
          }
        } catch {}
      }
      if (cancelled) return;
      const lastId = localStorage.getItem(LAST_PROFILE_STORAGE_KEY);
      if (!lastId) return;
      const profiles = loadStoredProfiles();
      const found = profiles.find((p) => p.id === lastId);
      if (found) setUser(found);
    })();
    return () => {
      cancelled = true;
    };
  }, [introFinished]);

  const updateRamGb = useCallback((value) => {
    const v = clampRamGb(value);
    setRamSize(v);
    saveLauncherSettings({ ramGb: v });
  }, []);

  useEffect(() => {
    if (!introFinished) return undefined;
    if (!window.launcher?.createcraftsForceModResyncPending) return undefined;
    let cancelled = false;
    window.launcher.createcraftsForceModResyncPending().then((r) => {
      if (!cancelled) setForceModResyncPending(Boolean(r?.pending));
    });
    return () => {
      cancelled = true;
    };
  }, [introFinished, activeTab]);

  useEffect(() => {
    if (!window.launcher) return undefined;
    let cancelled = false;
    const unsubs = [];

    void (async () => {
      try {
        const u1 = await window.launcher.onStateChange((newState) => {
          if (!cancelled) setConnectionState(newState);
        });
        if (cancelled) {
          u1();
          return;
        }
        unsubs.push(u1);

        const u2 = await window.launcher.onProgress((newProgress) => {
          if (!cancelled) setProgress(newProgress);
        });
        if (cancelled) {
          u2();
          return;
        }
        unsubs.push(u2);

        if (window.launcher.onLauncherCrash) {
          const u3 = await window.launcher.onLauncherCrash((msg) => {
            if (!cancelled) {
              setLaunchError(String(msg || 'Nieznany błąd uruchomienia.'));
            }
          });
          if (cancelled) {
            u3();
            return;
          }
          unsubs.push(u3);
        }
      } catch {
        /* ignore */
      }
    })();

    return () => {
      cancelled = true;
      for (const u of unsubs) {
        if (typeof u === 'function') u();
      }
    };
  }, []);

  const openAdoptiumJdkPage = useCallback(() => {
    if (window.launcher?.openExternalUrl) {
      void window.launcher.openExternalUrl(ADOPTIUM_JDK21_URL);
    }
  }, []);

  const handleConnectClick = () => {
    setLaunchError(null);
    if (window.launcher) {
      if (user.type === 'offline') {
        window.launcher.startGame({
          type: 'offline',
          offlineName: user.name,
          ramSize: `${ramSize}G`,
        });
      } else {
        window.launcher.startGame({
          type: 'premium',
          profileId: user.id,
          ramSize: `${ramSize}G`,
        });
      }
    } else {
      simulateConnection();
    }
  };

  const handleScheduleForceModResync = async () => {
    setForceModResyncNotice(null);
    if (!window.launcher?.createcraftsForceModResyncNext) {
      setForceModResyncNotice('Dostępne po zbudowaniu aplikacji (Tauri).');
      return;
    }
    const r = await window.launcher.createcraftsForceModResyncNext();
    if (r?.ok) {
      setForceModResyncPending(true);
      setForceModResyncNotice('Przy następnym uruchomieniu gry wszystkie mody z listy zostaną pobrane ponownie.');
    } else {
      setForceModResyncNotice(r?.error || 'Nie udało się zapisać żądania.');
    }
  };

  const simulateConnection = () => {
    setConnectionState('verifying');
    setProgress(0);
    let currentProgress = 0;
    
    const interval = setInterval(() => {
      currentProgress += (Math.random() * 8);
      
      if (currentProgress >= 100) {
        clearInterval(interval);
        setConnectionState('connected');
        setProgress(100);
        setTimeout(() => {
          setConnectionState('idle');
          setProgress(0);
        }, 3000);
        return;
      }
      
      setProgress(currentProgress);
      if (currentProgress < 30) setConnectionState('verifying');
      else if (currentProgress < 75) setConnectionState('downloading');
      else setConnectionState('launching');
    }, 200);
  };

  const getStatusText = () => {
    switch (connectionState) {
      case 'verifying':
        return 'Weryfikacja konta i sesji...';
      case 'checking-files':
        return 'Sprawdzanie plików modpacku (serwer CreateCrafts)...';
      case 'checking-java':
        return 'Sprawdzanie Java (wymagane JDK 21)...';
      case 'mods-sync':
        return `Pobieranie / aktualizacja modów (NeoForge ${gamePack.neoForgeInstallerVersion})…`;
      case 'downloading':
        return 'Pobieranie bibliotek Minecraft (launcher)...';
      case 'launching':
        return 'Uruchamianie gry...';
      case 'connected':
        return 'Gra wystartowała. Miłej zabawy!';
      default:
        return 'Gotowy do gry.';
    }
  };

  const handleLogout = () => {
    setUser(null);
    setActiveTab('home');
  };

  const handleProfileLogin = useCallback((profile) => {
    setLastProfileId(profile.id);
    setUser(profile);
  }, []);

  if (!introFinished) {
    return <IntroAnimation />;
  }

  if (!user) {
    return (
      <div className="flex min-h-screen flex-col overflow-hidden bg-background font-sans text-foreground antialiased selection:bg-primary/25">
        <TitleBar />
        <div className="relative min-h-0 flex-1">
          <LoginScreen onProfileLogin={handleProfileLogin} />
        </div>
        <SteampunkFooter />
      </div>
    );
  }

  return (
    <div className="flex min-h-screen flex-col overflow-hidden bg-background font-sans text-foreground antialiased selection:bg-primary/25">
      {launchError && (
        <div
          className="fixed inset-0 z-[100] flex items-center justify-center bg-black/70 p-6 backdrop-blur-sm"
          role="alertdialog"
          aria-modal="true"
        >
          <div className="glass-card max-w-lg border-red-500/40 p-6 shadow-2xl">
            <h3 className="mb-2 text-lg font-black text-destructive">Minecraft się nie uruchomił</h3>
            <pre className="mb-4 max-h-64 overflow-y-auto whitespace-pre-wrap break-words rounded-lg border border-glass-border bg-background/80 p-3 font-mono text-xs text-muted-foreground [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
              {launchError}
            </pre>
            {launchError && /JDK|Java|java/.test(launchError) && window.launcher?.openExternalUrl && (
              <button
                type="button"
                onClick={() => openAdoptiumJdkPage()}
                className="mb-3 w-full rounded-xl border border-glass-border bg-card/60 py-3 text-sm font-bold text-foreground transition-colors hover:bg-muted/80"
              >
                Pobierz JDK 21 (Temurin)
              </button>
            )}
            <button
              type="button"
              onClick={() => {
                setLaunchError(null);
              }}
              className="w-full rounded-xl border border-glass-border bg-muted py-3 text-sm font-bold text-foreground transition-colors hover:bg-muted/80"
            >
              Zamknij
            </button>
          </div>
        </div>
      )}
      <TitleBar />
      <SteampunkNavbar activeTab={activeTab} onSelectTab={setActiveTab} />

      <main className="relative flex-1 overflow-y-auto pt-[calc(2rem+3.5rem)] [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
        <GearDecoration size={160} className="pointer-events-none absolute -right-16 top-24 opacity-15" />
        <GearDecoration size={100} className="pointer-events-none absolute -left-10 bottom-32 opacity-10" reverse />

        {activeTab === 'home' && (
          <div className="pb-16">
            <LauncherMainPanel
              user={user}
              connectionState={connectionState}
              progress={progress}
              statusText={getStatusText()}
              onPlay={handleConnectClick}
            />
          </div>
        )}

        {activeTab === 'mods' && (
          <div className="mx-auto w-full max-w-4xl px-4 pb-16 pt-8 lg:px-8">
            <ModsListPanel />
          </div>
        )}

        {activeTab === 'settings' && (
          <div className="mx-auto w-full max-w-4xl px-4 pb-16 pt-8 lg:px-8">
            <div className="mb-8 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
              <div>
                <p className="mb-1 text-[10px] font-bold uppercase tracking-[0.2em] text-brass-light">CreateCrafts</p>
                <h2 className="text-3xl font-black text-foreground">Ustawienia</h2>
              </div>
              <div className="flex flex-wrap items-center gap-2">
                <span className="inline-flex items-center gap-1.5 rounded-full border border-glass-border bg-muted/50 px-3 py-1.5 text-[11px] font-semibold text-muted-foreground">
                  <Sparkles size={12} className="text-primary" />
                  MC {gamePack.minecraftVersion}
                </span>
                <span className="inline-flex items-center rounded-full border border-primary/30 bg-primary/12 px-3 py-1.5 text-[11px] font-bold text-primary">
                  NeoForge {gamePack.neoForgeInstallerVersion}
                </span>
              </div>
            </div>

            {forceModResyncPending && (
              <div className="mb-6 rounded-xl border border-primary/35 bg-primary/10 px-4 py-3 text-sm text-primary">
                Zaplanowano ponowną synchronizację modów przy <strong>następnym</strong> starcie gry.
              </div>
            )}
            {forceModResyncNotice && (
              <div className="mb-6 rounded-xl border border-glass-border bg-muted/40 px-4 py-3 text-sm text-muted-foreground">
                {forceModResyncNotice}
              </div>
            )}

            <CogwheelFrame>
              <div className="divide-y divide-glass-border/70">
                <section className="px-5 py-6 sm:px-7 sm:py-7">
                  <p className="mb-4 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">Java</p>
                  <p className="mb-3 text-sm leading-relaxed text-muted-foreground">
                    Wymagane JDK 21 — launcher używa Javy ze środowiska (np. PATH, JAVA_HOME).
                  </p>
                  <div className="flex flex-wrap gap-2">
                    {window.launcher?.openExternalUrl && (
                      <button
                        type="button"
                        onClick={() => openAdoptiumJdkPage()}
                        className="rounded-xl border border-glass-border bg-muted/50 px-4 py-2.5 text-sm font-bold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
                      >
                        Pobierz JDK 21 (Temurin)
                      </button>
                    )}
                  </div>
                </section>
                <section className="px-5 py-6 sm:px-7 sm:py-7">
                  <p className="mb-4 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                    Wydajność
                  </p>
                  <div className="flex flex-col gap-5 sm:flex-row sm:items-start sm:justify-between">
                    <div className="flex min-w-0 items-start gap-3">
                      <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-glass-border bg-primary/10 text-primary">
                        <Cpu size={22} strokeWidth={2} />
                      </div>
                      <div className="min-w-0">
                        <h3 className="text-lg font-bold text-foreground">Pamięć RAM (heap)</h3>
                        <p className="mt-1 text-sm leading-relaxed text-muted-foreground">
                          Przydział dla JVM Minecrafta. Większy modpack zwykle 6–12 GB. Wartość jest zapisywana od razu.
                        </p>
                      </div>
                    </div>
                    <div className="flex shrink-0 items-center gap-2 self-start rounded-2xl border border-glass-border bg-card/50 px-4 py-2.5 sm:self-center">
                      <input
                        type="number"
                        min={2}
                        max={32}
                        value={ramSize}
                        onChange={(e) => {
                          const raw = e.target.value;
                          const n = parseInt(raw, 10);
                          if (raw === '' || Number.isNaN(n)) return;
                          updateRamGb(n);
                        }}
                        onBlur={() => updateRamGb(ramSize)}
                        className="w-14 bg-transparent text-center text-xl font-black tabular-nums text-primary focus:outline-none focus:ring-0"
                      />
                      <span className="text-xs font-bold uppercase tracking-wider text-muted-foreground">GB</span>
                    </div>
                  </div>
                  <input
                    type="range"
                    min={2}
                    max={32}
                    step={1}
                    value={ramSize}
                    onChange={(e) => updateRamGb(parseInt(e.target.value, 10))}
                    className="settings-range-slider mt-5 h-3 w-full cursor-pointer"
                  />
                  <p className="mt-3 text-xs text-muted-foreground">
                    <span className="font-semibold text-foreground">{ramSize} GB</span> — używane przy następnym
                    uruchomieniu gry.
                  </p>
                </section>

                <section className="flex flex-col gap-5 px-5 py-6 sm:flex-row sm:items-center sm:justify-between sm:px-7 sm:py-7">
                  <div className="flex min-w-0 items-start gap-3">
                    <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border border-glass-border bg-secondary/10 text-secondary">
                      <RefreshCw size={22} strokeWidth={2} />
                    </div>
                    <div>
                      <p className="mb-1 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">
                        Paczka z serwera
                      </p>
                      <h3 className="text-lg font-bold text-foreground">Synchronizacja modów</h3>
                      <p className="mt-1 max-w-md text-sm leading-relaxed text-muted-foreground">
                        Wymusza ponowne pobranie plików z indeksu createcrafts.pl przy kolejnym starcie gry (nie kasuje
                        świata).
                      </p>
                    </div>
                  </div>
                  <button
                    type="button"
                    onClick={handleScheduleForceModResync}
                    className="inline-flex shrink-0 items-center justify-center gap-2 rounded-xl border border-primary/40 bg-primary/15 px-5 py-3 text-sm font-bold text-primary transition-colors hover:bg-primary/25"
                  >
                    <RefreshCw size={18} />
                    Wymuś weryfikację
                  </button>
                </section>

                <section className="px-5 py-6 sm:px-7 sm:py-7">
                  <p className="mb-4 text-[10px] font-bold uppercase tracking-[0.2em] text-muted-foreground">Konto</p>
                  <div className="flex flex-col gap-6 sm:flex-row sm:items-center sm:justify-between">
                    <div className="flex items-center gap-4">
                      <ProfileAvatar profile={user} className="h-16 w-16 rounded-2xl border border-glass-border text-lg" />
                      <div>
                        <p className="text-xs font-semibold uppercase tracking-wider text-muted-foreground">
                          Aktywna sesja
                        </p>
                        <p className="text-lg font-bold text-foreground">{user?.name ?? '—'}</p>
                        <p className="mt-0.5 flex items-center gap-1.5 text-sm text-muted-foreground">
                          {user?.type === 'premium' ? (
                            <ShieldCheck size={14} className="text-primary" />
                          ) : (
                            <User size={14} />
                          )}
                          {user?.type === 'premium' ? 'Konto Microsoft' : 'Gracz offline'}
                        </p>
                      </div>
                    </div>
                    <button
                      type="button"
                      onClick={handleLogout}
                      className="inline-flex items-center justify-center gap-2 rounded-xl border border-destructive/35 bg-destructive/10 px-6 py-3 text-sm font-bold text-destructive transition-colors hover:bg-destructive/20"
                    >
                      <LogOut size={18} />
                      Wyloguj się
                    </button>
                  </div>
                </section>
              </div>
            </CogwheelFrame>
          </div>
        )}
      </main>
      <SteampunkFooter />
    </div>
  );
}

function formatBytes(n) {
  if (n == null || typeof n !== 'number' || !Number.isFinite(n)) return '—';
  const units = ['B', 'KB', 'MB', 'GB'];
  let i = 0;
  let v = n;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i += 1;
  }
  const shown = v < 10 && i > 0 ? v.toFixed(1) : Math.round(v);
  return `${shown} ${units[i]}`;
}

function ModsListPanel() {
  const [info, setInfo] = useState(null);
  const [loading, setLoading] = useState(true);
  const [err, setErr] = useState(null);

  const load = useCallback(async () => {
    if (!window.launcher?.createcraftsModsInfo) {
      setErr(null);
      setLoading(false);
      setInfo({ modsDir: '', gameRoot: '', count: 0, mods: [], mock: true });
      return;
    }
    setLoading(true);
    setErr(null);
    try {
      const data = await window.launcher.createcraftsModsInfo();
      if (data && data.ok === false) {
        setErr(data.error || 'Błąd listy modów');
        setInfo({
          gameRoot: data.gameRoot,
          modsDir: data.modsDir,
          count: data.count ?? 0,
          mods: data.mods ?? [],
          baseUrl: data.baseUrl,
        });
      } else {
        setErr(null);
        setInfo(data);
      }
    } catch (e) {
      setErr(String(e?.message || e));
      setInfo(null);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const openModsDir = () => {
    if (info?.modsDir && window.launcher?.openPathInExplorer) {
      window.launcher.openPathInExplorer(info.modsDir);
    }
  };

  const openGameRoot = () => {
    if (info?.gameRoot && window.launcher?.openPathInExplorer) {
      window.launcher.openPathInExplorer(info.gameRoot);
    }
  };

  return (
    <>
      <div className="mb-8 flex flex-col gap-4 sm:flex-row sm:items-end sm:justify-between">
        <div>
          <p className="mb-1 text-[10px] font-bold uppercase tracking-[0.2em] text-brass-light">CreateCrafts</p>
          <h2 className="text-3xl font-black text-foreground">Mody z serwera</h2>
        </div>
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            onClick={load}
            disabled={loading}
            className="inline-flex items-center gap-2 rounded-xl border border-glass-border bg-muted/60 px-4 py-2.5 text-sm font-bold text-foreground transition-colors hover:bg-muted disabled:opacity-50"
          >
            <RefreshCw size={16} className={loading ? 'animate-spin' : ''} />
            Odśwież
          </button>
          <button
            type="button"
            onClick={openModsDir}
            disabled={!info?.modsDir}
            className="inline-flex items-center gap-2 rounded-xl border border-primary/40 bg-primary/15 px-4 py-2.5 text-sm font-bold text-primary transition-colors hover:bg-primary/25 disabled:cursor-not-allowed disabled:opacity-40"
          >
            <FolderOpen size={16} />
            Folder modów
          </button>
          <button
            type="button"
            onClick={openGameRoot}
            disabled={!info?.gameRoot}
            className="inline-flex items-center gap-2 rounded-xl border border-glass-border bg-card/50 px-4 py-2.5 text-sm font-bold text-muted-foreground transition-colors hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40"
          >
            Katalog gry
          </button>
        </div>
      </div>

      {info?.modsDir ? (
        <p className="mb-4 break-all font-mono text-[11px] text-muted-foreground/90">{info.modsDir}</p>
      ) : null}

      {loading && (
        <div className="flex items-center gap-2 text-sm text-muted-foreground">
          <Loader2 className="animate-spin" size={18} />
          Ładowanie listy z serwera…
        </div>
      )}
      {err && (
        <div className="rounded-xl border border-destructive/40 bg-destructive/10 px-4 py-3 text-sm text-destructive">
          {err}
        </div>
      )}

      {info?.mock && (
        <p className="text-sm text-muted-foreground">Podgląd w przeglądarce — pełna lista w zbudowanej aplikacji.</p>
      )}

      {!loading && !err && info?.mods?.length === 0 && !info?.mock && (
        <p className="text-muted-foreground">Brak pozycji na liście.</p>
      )}

      {!loading && info?.mods && info.mods.length > 0 && (
        <div className="glass-card overflow-hidden rounded-2xl border border-brass-dim/20">
          <div className="grid grid-cols-[minmax(0,1fr)_auto_auto] gap-2 border-b border-glass-border bg-card/40 px-4 py-2.5 text-[10px] font-bold uppercase tracking-wider text-muted-foreground">
            <span>Plik</span>
            <span className="text-right">Rozmiar</span>
            <span className="text-right">Status</span>
          </div>
          <ul className="max-h-[min(60vh,520px)] overflow-y-auto [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
            {info.mods.map((m) => (
              <li
                key={m.name}
                className="grid grid-cols-[minmax(0,1fr)_auto_auto] items-center gap-2 border-b border-glass-border/60 px-4 py-2.5 text-sm last:border-0 hover:bg-card/20"
              >
                <span className="min-w-0 truncate font-mono text-xs text-foreground" title={m.name}>
                  {m.name}
                </span>
                <span className="shrink-0 text-right tabular-nums text-xs text-muted-foreground">
                  {formatBytes(m.localSize)}
                </span>
                <span className="shrink-0 text-right">
                  {m.status === 'ok' && <span className="text-xs font-semibold text-primary">OK</span>}
                  {m.status === 'missing' && <span className="text-xs font-semibold text-amber-500">Brak</span>}
                  {m.status === 'mismatch' && (
                    <span className="text-xs font-semibold text-destructive">Inny plik</span>
                  )}
                  {m.status === 'unknown' && <span className="text-xs text-muted-foreground">?</span>}
                </span>
              </li>
            ))}
          </ul>
          <div className="border-t border-glass-border bg-card/30 px-4 py-2 text-center text-[11px] text-muted-foreground">
            {info.count} plików
            {info.baseUrl ? (
              <>
                {' '}
                · manifest: <span className="break-all">{info.baseUrl}</span>
              </>
            ) : null}
          </div>
        </div>
      )}
    </>
  );
}

function ProfileAvatar({ profile, className = '' }) {
  const [imgErr, setImgErr] = useState(false);
  const name = String(profile?.name || profile?.label || '?').trim() || '?';
  const initials = name.slice(0, 2).toUpperCase();
  const url = profile?.avatar?.trim();
  if (url && !imgErr) {
    return (
      <img
        src={url}
        alt=""
        className={`object-cover ${className}`}
        onError={() => setImgErr(true)}
      />
    );
  }
  return (
    <div
      className={`flex items-center justify-center bg-primary/20 font-black text-primary ${className}`}
      aria-hidden
    >
      {initials.slice(0, 1)}
    </div>
  );
}

function LoginScreen({ onProfileLogin }) {
  const [mode, setMode] = useState('select');
  const [offlineNick, setOfflineNick] = useState('');
  const [loading, setLoading] = useState(false);
  const [loginError, setLoginError] = useState(null);
  const [savedProfiles, setSavedProfiles] = useState(() => loadStoredProfiles());

  const refreshProfiles = () => setSavedProfiles(loadStoredProfiles());

  const removeStoredProfile = (e, id) => {
    e.stopPropagation();
    void (async () => {
      const list = loadStoredProfiles();
      const victim = list.find((p) => p.id === id);
      if (victim?.type === 'premium' && window.launcher?.deletePremiumSession) {
        try {
          await window.launcher.deletePremiumSession(id);
        } catch {}
      }
      const next = list.filter((p) => p.id !== id);
      saveStoredProfiles(next);
      if (localStorage.getItem(LAST_PROFILE_STORAGE_KEY) === id) setLastProfileId(next[0]?.id ?? null);
      refreshProfiles();
    })();
  };

  const handleSelectSavedProfile = (p) => {
    onProfileLogin(p);
  };

  const finishLogin = (payload) => {
    const withId = { ...payload, id: payload.id || crypto.randomUUID() };
    persistNewOrUpdatedProfile(withId);
    refreshProfiles();
    onProfileLogin(withId);
  };

  const handlePremiumLogin = async () => {
    setLoginError(null);
    setLoading(true);
    try {
      if (window.launcher) {
        const data = await window.launcher.loginMicrosoft();
        finishLogin({
          ...data,
          label: data.name,
        });
      } else {
        await new Promise((r) => setTimeout(r, 500));
        finishLogin({
          name: 'ProGamer',
          type: 'premium',
          label: 'ProGamer',
          avatar: '',
        });
      }
    } catch (err) {
      const msg =
        typeof err === 'string'
          ? err
          : err?.message || err?.toString?.() || 'Logowanie Microsoft nie powiodło się.';
      setLoginError(msg);
    } finally {
      setLoading(false);
    }
  };

  const handleOfflineSubmit = async (e) => {
    e.preventDefault();
    if (!offlineNick.trim()) return;
    setLoading(true);
    const nick = offlineNick.trim();
    try {
      let avatar = '';
      let id = crypto.randomUUID();
      if (window.launcher?.mineatarFaceUrl) {
        const r = await window.launcher.mineatarFaceUrl({ offlineName: nick });
        if (r?.url) avatar = r.url;
        if (r?.playerUuid) id = r.playerUuid;
      }
      finishLogin({
        name: nick,
        type: 'offline',
        label: nick,
        id,
        avatar,
      });
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="absolute inset-0 z-40 flex items-center justify-center p-4 pt-12">
      <div className="absolute inset-0 z-0">
        <img
          src={pub('hero-bg-ObJtS6DH.jpg')}
          alt=""
          className="h-full w-full object-cover"
        />
        <div className="absolute inset-0 bg-background/80 backdrop-blur-[1px]" />
        <div className="absolute inset-0 bg-gradient-to-t from-background via-background/50 to-background/70" />
      </div>
      <GearDecoration size={140} className="pointer-events-none absolute right-[5%] top-[18%] z-[1] opacity-20" />
      <GearDecoration size={100} className="pointer-events-none absolute bottom-[12%] left-[8%] z-[1] opacity-15" reverse />

      <div className="glass-card relative z-10 flex w-full max-w-md flex-col items-center rounded-3xl border border-brass-dim/25 p-8 shadow-2xl">
        <div className="mb-6 flex h-16 w-16 items-center justify-center rounded-2xl border border-glass-border bg-card/80 shadow-inner transition-all hover:border-primary/40">
          <Key size={28} className="text-primary transition-transform group-hover:scale-110" />
        </div>

        <h2 className="mb-2 text-3xl font-black tracking-tight text-foreground">AUTORYZACJA</h2>
        <p className="mb-6 px-4 text-center text-sm text-muted-foreground">
          Wybierz zapisaną sesję albo dodaj nowe konto.
        </p>

        {loginError && (
          <div
            role="alert"
            className="mb-4 w-full rounded-xl border border-destructive/40 bg-destructive/10 px-3 py-2 text-center text-sm text-destructive"
          >
            {loginError}
          </div>
        )}

        {savedProfiles.length > 0 && mode === 'select' && (
          <div className="mb-6 w-full space-y-2">
            <p className="pl-1 text-[10px] font-black uppercase tracking-widest text-brass-light">Zapisane sesje</p>
            <div className="flex max-h-44 flex-col gap-2 overflow-y-auto pr-0.5 [scrollbar-width:none] [&::-webkit-scrollbar]:hidden">
              {savedProfiles.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  onClick={() => handleSelectSavedProfile(p)}
                  className="group/row flex w-full items-center gap-3 rounded-xl border border-glass-border bg-background/70 px-3 py-2.5 text-left transition-colors hover:border-primary/35 hover:bg-card/50"
                >
                  <ProfileAvatar profile={p} className="h-10 w-10 shrink-0 rounded-lg" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-bold text-foreground">{p.label || p.name}</p>
                    <p className="truncate text-[11px] text-muted-foreground">
                      {p.name} · {p.type === 'premium' ? 'Microsoft' : 'Offline'}
                    </p>
                  </div>
                  <span
                    role="button"
                    tabIndex={0}
                    onClick={(ev) => removeStoredProfile(ev, p.id)}
                    onKeyDown={(ev) => ev.key === 'Enter' && removeStoredProfile(ev, p.id)}
                    className="rounded-lg p-2 text-muted-foreground opacity-0 transition-opacity hover:bg-destructive/15 hover:text-destructive group-hover/row:opacity-100"
                    title="Usuń sesję"
                  >
                    <Trash2 size={16} />
                  </span>
                </button>
              ))}
            </div>
            <div className="my-4 h-px w-full bg-glass-border/80" />
          </div>
        )}

        {mode === 'select' && (
          <div className="flex w-full flex-col gap-4">
            <button
              type="button"
              onClick={handlePremiumLogin}
              disabled={loading}
              className="group relative flex w-full items-center justify-center gap-3 overflow-hidden rounded-xl bg-primary py-4 font-black text-primary-foreground transition-all hover:brightness-110 hover:shadow-[0_0_30px_hsl(142_69%_58%_/_0.35)] disabled:cursor-not-allowed disabled:opacity-50"
            >
              <div className="absolute inset-0 translate-y-full bg-white/20 transition-transform duration-300 ease-out group-hover:translate-y-0" />
              {loading ? (
                <Loader2 size={20} className="relative z-10 animate-spin" />
              ) : (
                <img
                  src="https://upload.wikimedia.org/wikipedia/commons/4/44/Microsoft_logo.svg"
                  className="relative z-10 h-5 w-5 opacity-90 transition-transform group-hover:scale-110"
                  alt="MS"
                />
              )}
              <span className="relative z-10">KONTO PREMIUM</span>
            </button>

            <button
              type="button"
              onClick={() => setMode('offline')}
              disabled={loading}
              className="group flex w-full items-center justify-center gap-3 rounded-xl border border-glass-border bg-muted/40 py-4 font-bold text-muted-foreground transition-all hover:border-primary/30 hover:bg-muted/70 hover:text-foreground disabled:opacity-50"
            >
              <User size={20} className="transition-colors group-hover:text-foreground" />
              <span>GRACZ OFFLINE</span>
            </button>
          </div>
        )}

        {mode === 'offline' && (
          <form
            onSubmit={handleOfflineSubmit}
            className="flex w-full flex-col gap-4 duration-300 animate-in fade-in slide-in-from-right-4"
          >
            <div className="flex flex-col gap-2">
              <label className="pl-1 text-xs font-bold uppercase tracking-wider text-muted-foreground">Nick w grze</label>
              <div className="relative">
                <User size={18} className="absolute left-4 top-1/2 -translate-y-1/2 text-muted-foreground" />
                <input
                  type="text"
                  value={offlineNick}
                  onChange={(e) => setOfflineNick(e.target.value)}
                  placeholder="TWÓJ NICK"
                  className="w-full rounded-xl border border-glass-border bg-background/90 py-3.5 pl-11 pr-4 font-medium text-foreground transition-all placeholder:text-muted-foreground/50 focus:border-primary focus:outline-none focus:ring-1 focus:ring-primary"
                  autoFocus
                />
              </div>
            </div>
            <div className="mt-4 flex gap-3">
              <button
                type="button"
                onClick={() => setMode('select')}
                className="rounded-xl border border-glass-border bg-muted/50 px-5 py-3.5 font-bold text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
              >
                Powrót
              </button>
              <button
                type="submit"
                disabled={loading || !offlineNick.trim()}
                className="group flex flex-1 items-center justify-center gap-2 rounded-xl bg-primary py-3.5 font-black text-primary-foreground shadow-[0_0_15px_hsl(142_69%_58%_/_0.2)] transition-all hover:brightness-110 disabled:cursor-not-allowed disabled:opacity-50"
              >
                {loading ? (
                  <Loader2 size={18} className="animate-spin" />
                ) : (
                  <Play size={18} className="fill-current transition-transform group-hover:scale-110" />
                )}
                <span>Zapisz i wejdź</span>
              </button>
            </div>
          </form>
        )}
      </div>
    </div>
  );
}

function TitleBar() {
  return (
    <div
      data-tauri-drag-region
      className="relative z-50 flex h-8 select-none items-center justify-between border-b border-brass-dim/30 bg-background/95 pl-4 pr-2 backdrop-blur-md"
    >
      <div className="flex items-center gap-2 text-xs font-semibold tracking-wider text-muted-foreground" data-tauri-drag-region>
        <img src={pub('logomain.png')} alt="" className="h-5 w-5 shrink-0 object-contain" width={20} height={20} />
        <span>CREATECRAFTS LAUNCHER</span>
      </div>
      <div className="flex items-center gap-1">
        <button
          type="button"
          onClick={() => window.launcher?.minimize()}
          className="rounded p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <Minus size={14} />
        </button>
        <button
          type="button"
          onClick={() => window.launcher?.maximize()}
          className="rounded p-1.5 text-muted-foreground transition-colors hover:bg-muted hover:text-foreground"
        >
          <Square size={12} />
        </button>
        <button
          type="button"
          onClick={() => window.launcher?.close()}
          className="rounded p-1.5 text-muted-foreground transition-colors hover:bg-destructive hover:text-destructive-foreground"
        >
          <X size={14} />
        </button>
      </div>
    </div>
  );
}

function IntroAnimation() {
  return (
    <div className="fixed inset-0 z-50 flex flex-col items-center justify-center bg-background">
      <div className="mb-10 flex items-center gap-4">
        <Globe size={56} className="animate-[pulse_2s_ease-in-out_infinite] text-primary" />
        <div className="flex flex-col">
          <span className="text-4xl font-black leading-none tracking-widest text-foreground">CREATE</span>
          <span className="text-xl font-black tracking-[0.25em] text-gradient-emerald">CRAFT</span>
        </div>
      </div>
      <div className="h-1.5 w-64 overflow-hidden rounded-full border border-glass-border bg-muted">
        <div className="h-full w-0 animate-[loadingBar_1.2s_ease-in-out_forwards] bg-primary shadow-[0_0_10px_hsl(142_69%_58%_/_0.5)]" />
      </div>
      <style>{`
        @keyframes loadingBar {
          0% { width: 0%; }
          30% { width: 40%; }
          100% { width: 100%; }
        }
      `}</style>
    </div>
  );
}

