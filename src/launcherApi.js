import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

const win = getCurrentWindow();

window.launcher = {
  startGame: (payload) => invoke('start_game', { payload }),

  loginMicrosoft: () => invoke('login_microsoft'),
  migrateProfilesFromLocalStorage: (payload) => {
    const rawJson =
      typeof payload === 'string' ? payload : payload?.rawJson ?? null;
    const lastProfileId =
      typeof payload === 'object' && payload ? payload.lastProfileId : null;
    return invoke('migrate_profiles_from_localstorage', {
      rawJson,
      lastProfileId: lastProfileId ?? null,
    });
  },
  deletePremiumSession: (profileId) =>
    invoke('delete_premium_session', { profileId }),
  mineatarFaceUrl: (payload) =>
    invoke('mineatar_face_url', {
      uuid: payload?.uuid ?? null,
      offlineName: payload?.offlineName ?? null,
    }),

  onStateChange: (callback) =>
    listen('launcher-state', (event) => callback(event.payload)),
  onProgress: (callback) =>
    listen('launcher-progress', (event) => callback(event.payload)),
  onLauncherLog: (callback) =>
    listen('launcher-log', (event) => callback(event.payload)),
  onLauncherCrash: (callback) =>
    listen('launcher-crash', (event) => callback(event.payload)),

  createcraftsModsInfo: () => invoke('get_mods_info'),
  createcraftsForceModResyncNext: () => invoke('force_mod_resync_next'),
  createcraftsForceModResyncPending: () => invoke('force_mod_resync_pending'),

  openPathInExplorer: (dirPath) =>
    invoke('open_path_in_explorer', { dirPath }),
  openExternalUrl: (url) => invoke('open_external_url', { url }),

  getAppVersion: () => invoke('get_app_version'),
  checkLauncherUpdate: () => invoke('check_launcher_update'),
  downloadAndInstallLauncherUpdate: (downloadUrl, expectedSha256Hex) =>
    invoke('download_and_install_launcher_update', { downloadUrl, expectedSha256Hex }),

  minimize: () => win.minimize(),
  maximize: () =>
    win.isMaximized().then((maximized) => {
      if (maximized) win.unmaximize();
      else win.maximize();
    }),
  close: () => win.close(),
};
