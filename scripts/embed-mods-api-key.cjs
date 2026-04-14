'use strict';
/**
 * Zapisuje zaszyfrowany klucz API modów do build/launcher-mods-key.enc.
 * Opcja --branding: dodatkowo zapis branding/launcher-mods-key.enc (do commita — MSI działa u wszystkich bez sekretu na maszynie buildującej).
 * --stdin: czytaj klucz ze stdin (jedna linia); zawsze nadpisuje branding/build (bez USE_ENV_LAUNCHER_MODS_KEY).
 * prepackage: ustaw LAUNCHER_MODS_API_KEY w CI albo polegaj na pliku z brandingu (kopiowanym wcześniej przez prep:brand).
 */
const fs = require('fs');
const path = require('path');
const { encryptModsApiKeyToBuffer } = require('../src/launcherModsKeyEmbed.js');

const writeBranding = process.argv.includes('--branding');
const fromStdin = process.argv.includes('--stdin');
const outDir = path.join(__dirname, '..', 'build');
const outFile = path.join(outDir, 'launcher-mods-key.enc');
const brandEnc = path.join(__dirname, '..', 'branding', 'launcher-mods-key.enc');

let key = '';
if (fromStdin) {
  try {
    key = fs.readFileSync(0, 'utf8').trim();
  } catch {
    key = '';
  }
}
if (!key) {
  const raw = process.env.LAUNCHER_MODS_API_KEY;
  key = raw ? String(raw).trim() : '';
}
if (!key || key.length < 16) {
  if (fs.existsSync(outFile)) {
    console.log(
      'embed-mods-api-key: LAUNCHER_MODS_API_KEY pominięty — zostaje',
      outFile,
      '(np. skopiowany z branding/ przez prep:brand).'
    );
  } else {
    console.log(
      'embed-mods-api-key: brak LAUNCHER_MODS_API_KEY i brak',
      outFile,
      '— dodaj branding/launcher-mods-key.enc (npm run embed:mods-key -- --branding) albo ustaw zmienną przy buildzie.'
    );
  }
  process.exit(0);
}

if (
  !fromStdin &&
  fs.existsSync(outFile) &&
  fs.existsSync(brandEnc) &&
  process.env.USE_ENV_LAUNCHER_MODS_KEY !== '1'
) {
  console.log(
    'embed-mods-api-key: LAUNCHER_MODS_API_KEY ignorowany — zostaje klucz z branding/ (prep:brand). Ustaw USE_ENV_LAUNCHER_MODS_KEY=1 aby nadpisać z env (CI / rotacja klucza).'
  );
  process.exit(0);
}

fs.mkdirSync(outDir, { recursive: true });
const buf = encryptModsApiKeyToBuffer(key);
fs.writeFileSync(outFile, buf);
console.log('embed-mods-api-key: zapisano', outFile);

if (writeBranding) {
  const brandDir = path.join(__dirname, '..', 'branding');
  const brandFile = path.join(brandDir, 'launcher-mods-key.enc');
  fs.mkdirSync(brandDir, { recursive: true });
  fs.copyFileSync(outFile, brandFile);
  console.log('embed-mods-api-key: zapisano', brandFile, '(zacommituj — wtedy make bez env też wbuduje klucz w MSI)');
}
