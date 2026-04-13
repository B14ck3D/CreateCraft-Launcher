const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  startGame: (payload) => ipcRenderer.send('start-game', payload),
  loginMicrosoft: () => ipcRenderer.invoke('login-microsoft'),
  migrateProfilesFromLocalStorage: (rawJson) =>
    ipcRenderer.invoke('profiles-migrate-localstorage', rawJson),
  deletePremiumSession: (profileId) => ipcRenderer.invoke('profiles-delete-premium', profileId),
  mineatarFaceUrl: (payload) => ipcRenderer.invoke('mineatar-face-url', payload),

  onStateChange: (callback) => ipcRenderer.on('launcher-state', (_event, state) => callback(state)),
  onProgress: (callback) => ipcRenderer.on('launcher-progress', (_event, progress) => callback(progress)),
  onLauncherLog: (callback) => ipcRenderer.on('launcher-log', (_event, value) => callback(value)),
  onLauncherCrash: (callback) => ipcRenderer.on('launcher-crash', (_event, value) => callback(value)),

  minimize: () => ipcRenderer.send('window-minimize'),
  maximize: () => ipcRenderer.send('window-maximize'),
  close: () => ipcRenderer.send('window-close'),

  createcraftsModsInfo: () => ipcRenderer.invoke('createcrafts-mods-info'),
  createcraftsForceModResyncNext: () => ipcRenderer.invoke('createcrafts-force-mod-resync-next'),
  createcraftsForceModResyncPending: () => ipcRenderer.invoke('createcrafts-force-mod-resync-pending'),
  openPathInExplorer: (dirPath) => ipcRenderer.invoke('open-path-in-explorer', dirPath),
  openExternalUrl: (url) => ipcRenderer.invoke('open-external-url', url),
});
