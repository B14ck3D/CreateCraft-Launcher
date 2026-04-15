/**
 * Tauri ↔ electronAPI compatibility shim.
 *
 * Sets `window.electronAPI` to a surface that mirrors the old preload.js
 * contextBridge API so that App.jsx works unchanged with Tauri invoke/listen.
 *
 * Imported once at the top of renderer.jsx before React renders.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';

const win = getCurrentWindow();

window.electronAPI = {
  // ----------------------------------------------------------------
  // Game
  // ----------------------------------------------------------------
  startGame: (payload) => invoke('start_game', { payload }),

  // ----------------------------------------------------------------
  // Auth
  // ----------------------------------------------------------------
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

  // ----------------------------------------------------------------
  // Events (one-time listener registration on first call)
  // ----------------------------------------------------------------
  onStateChange: (callback) => {
    listen('launcher-state', (event) => callback(event.payload));
  },
  onProgress: (callback) => {
    listen('launcher-progress', (event) => callback(event.payload));
  },
  onLauncherLog: (callback) => {
    listen('launcher-log', (event) => callback(event.payload));
  },
  onLauncherCrash: (callback) => {
    listen('launcher-crash', (event) => callback(event.payload));
  },

  // ----------------------------------------------------------------
  // Mods
  // ----------------------------------------------------------------
  createcraftsModsInfo: () => invoke('get_mods_info'),
  createcraftsForceModResyncNext: () => invoke('force_mod_resync_next'),
  createcraftsForceModResyncPending: () => invoke('force_mod_resync_pending'),

  // ----------------------------------------------------------------
  // Shell
  // ----------------------------------------------------------------
  openPathInExplorer: (dirPath) =>
    invoke('open_path_in_explorer', { dirPath }),
  openExternalUrl: (url) => invoke('open_external_url', { url }),

  // ----------------------------------------------------------------
  // Window controls
  // ----------------------------------------------------------------
  minimize: () => win.minimize(),
  maximize: () =>
    win.isMaximized().then((maximized) => {
      if (maximized) win.unmaximize();
      else win.maximize();
    }),
  close: () => win.close(),
};
