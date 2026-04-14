// servers.dat — dokladna kopia pliku z brandingu (prepare-branding -> build/createcrafts-servers-default.dat -> extraResource).
// Bez parsowania NBT i bez nadpisywania hosta — tylko copyFileSync do katalogu gry przed startem MC.
const fs = require('fs');
const path = require('path');

function ensureDefaultServerInServersDatSync(gameRoot, onLog, templatePath) {
  const log = onLog || (() => {});
  const rootResolved = path.resolve(gameRoot);
  const dest = path.join(rootResolved, 'servers.dat');

  if (!templatePath || !fs.existsSync(templatePath)) {
    log('[servers.dat] Brak szablonu createcrafts-servers-default.dat — pomijam.');
    return;
  }

  try {
    fs.mkdirSync(rootResolved, { recursive: true });
    fs.copyFileSync(templatePath, dest);
    log(`[servers.dat] Skopiowano 1:1 z launchera do ${dest}`);
  } catch (e) {
    log(`[servers.dat] Kopiowanie nieudane: ${e.message}`);
  }
}

module.exports = {
  ensureDefaultServerInServersDatSync,
};
