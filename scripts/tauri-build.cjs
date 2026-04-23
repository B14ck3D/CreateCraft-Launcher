#!/usr/bin/env node
const { spawnSync } = require('node:child_process');

if (process.platform === 'linux') {
  process.env.APPIMAGE_EXTRACT_AND_RUN = '1';
  process.env.NO_STRIP = '1';
}

const extra = process.argv.slice(2);
const env = { ...process.env };
if (env.CI === '1') {
  delete env.CI;
}

const r = spawnSync('npx', ['tauri', 'build', ...extra], {
  stdio: 'inherit',
  env,
  shell: true,
});

process.exit(r.status ?? 1);
