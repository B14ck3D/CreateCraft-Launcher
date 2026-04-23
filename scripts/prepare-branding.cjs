#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const buildDir = path.join(root, 'build');
const publicDir = path.join(root, 'public');
const brandingDir = path.join(root, 'branding');

const ICON_SIDE = 1024;
const ICON_BG = { r: 15, g: 19, b: 26, alpha: 1 };

function copyIfExists(src, dest, label) {
  if (!fs.existsSync(src)) return false;
  fs.mkdirSync(path.dirname(dest), { recursive: true });
  fs.copyFileSync(src, dest);
  console.log(`OK: ${label}`);
  return true;
}

async function writeSquareAppIcon(logoPath, iconPath) {
  const sharp = require('sharp');
  let pipeline = sharp(logoPath).ensureAlpha();
  try {
    pipeline = pipeline.trim();
  } catch {
    /* ignore trim if unsupported for format */
  }
  await pipeline
    .resize({
      width: ICON_SIDE,
      height: ICON_SIDE,
      fit: 'cover',
      position: 'centre',
    })
    .png()
    .toFile(iconPath);
  console.log(`OK: ${iconPath} (square ${ICON_SIDE}px, cover+trim from logo)`);
}

(async () => {
  fs.mkdirSync(buildDir, { recursive: true });

  const logoSrc = fs.existsSync(path.join(publicDir, 'mainlogo.png'))
    ? path.join(publicDir, 'mainlogo.png')
    : path.join(publicDir, 'logomain.png');
  const iconOut = path.join(buildDir, 'icon.png');
  const iconPublicOut = path.join(publicDir, 'icon.png');
  if (!fs.existsSync(logoSrc)) {
    throw new Error(`Missing logo: add public/mainlogo.png or public/logomain.png`);
  }
  await writeSquareAppIcon(logoSrc, iconOut);
  await writeSquareAppIcon(logoSrc, iconPublicOut);

  const serversDatSrc = path.join(brandingDir, 'servers.dat');
  const serversDatOut = path.join(buildDir, 'createcrafts-servers-default.dat');
  if (!fs.existsSync(serversDatSrc)) {
    throw new Error(`Missing servers list template (NBT): ${serversDatSrc}`);
  }
  copyIfExists(serversDatSrc, serversDatOut, `${serversDatOut} (servers.dat)`);

  const plainKeySrc = path.join(brandingDir, 'launcher-mods-key');
  const plainKeyOut = path.join(buildDir, 'launcher-mods-key');
  copyIfExists(plainKeySrc, plainKeyOut, `${plainKeyOut} (do not commit)`);

  const encKeySrc = path.join(brandingDir, 'launcher-mods-key.enc');
  const encKeyOut = path.join(buildDir, 'launcher-mods-key.enc');
  const hasEnc = copyIfExists(encKeySrc, encKeyOut, `${encKeyOut}`);
  if (!hasEnc && !fs.existsSync(plainKeySrc)) {
    console.log(
      'INFO: Missing branding/launcher-mods-key.enc and branding/launcher-mods-key - set LAUNCHER_MODS_API_KEY or use embed script.'
    );
  }
})().catch((e) => {
  console.error(e);
  process.exit(1);
});
