const { contextBridge, ipcRenderer } = require('electron');

contextBridge.exposeInMainWorld('electronAPI', {
  // Główne funkcje launchera
  startGame: (authData) => ipcRenderer.send('start-game', authData),
  loginMicrosoft: () => ipcRenderer.invoke('login-microsoft'),
  
  // Nasłuchiwanie stanów od Main procesu
  onStateChange: (callback) => ipcRenderer.on('launcher-state', (_event, state) => callback(state)),
  onProgress: (callback) => ipcRenderer.on('launcher-progress', (_event, progress) => callback(progress)),
  onLauncherLog: (callback) => ipcRenderer.on('launcher-log', (_event, value) => callback(value)),
  onLauncherCrash: (callback) => ipcRenderer.on('launcher-crash', (_event, value) => callback(value)),
  onProfileTokenRefreshed: (callback) =>
    ipcRenderer.on('profile-token-refreshed', (_event, payload) => callback(payload)),

  // Przyciski belki (Window Controls)
  minimize: () => ipcRenderer.send('window-minimize'),
  maximize: () => ipcRenderer.send('window-maximize'),
  close: () => ipcRenderer.send('window-close'),

  createcraftsModsInfo: () => ipcRenderer.invoke('createcrafts-mods-info'),
  createcraftsForceModResyncNext: () => ipcRenderer.invoke('createcrafts-force-mod-resync-next'),
  createcraftsForceModResyncPending: () => ipcRenderer.invoke('createcrafts-force-mod-resync-pending'),
  openPathInExplorer: (dirPath) => ipcRenderer.invoke('open-path-in-explorer', dirPath),
  openExternalUrl: (url) => ipcRenderer.invoke('open-external-url', url),
});
