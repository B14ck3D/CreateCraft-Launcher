#!/usr/bin/env node
const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');

function parseTargetTriple() {
  const i = process.argv.indexOf('--target');
  if (i !== -1 && process.argv[i + 1]) return process.argv[i + 1].trim();
  const prefixed = process.argv.find((a) => a.startsWith('--target='));
  if (prefixed) return prefixed.slice('--target='.length).trim();
  return '';
}

const targetTriple = parseTargetTriple();
const bundleDir = targetTriple
  ? path.join(root, 'src-tauri', 'target', targetTriple, 'release', 'bundle')
  : path.join(root, 'src-tauri', 'target', 'release', 'bundle');

function firstFile(dir, predicate) {
  if (!fs.existsSync(dir)) return null;
  const entries = fs.readdirSync(dir, { withFileTypes: true });
  for (const entry of entries) {
    if (!entry.isFile()) continue;
    if (predicate(entry.name)) return path.join(dir, entry.name);
  }
  return null;
}

function renameInDir(dirName, ext, targetName) {
  const dir = path.join(bundleDir, dirName);
  const target = path.join(dir, targetName);
  if (!fs.existsSync(dir)) return;

  const source = firstFile(
    dir,
    (name) => name.endsWith(ext) && name !== targetName
  );
  if (!source) {
    if (fs.existsSync(target)) {
      console.log(`OK: ${target} (already named correctly)`);
    } else {
      console.log(`INFO: no ${ext} artifact in ${dir}`);
    }
    return;
  }

  if (fs.existsSync(target)) fs.rmSync(target, { force: true });
  fs.renameSync(source, target);
  console.log(`OK: ${target}`);
}

renameInDir('nsis', '.exe', 'CreateCrafts-installer.exe');
renameInDir('msi', '.msi', 'CreateCrafts-installer.msi');
renameInDir('appimage', '.AppImage', 'CreateCrafts-installer.AppImage');
