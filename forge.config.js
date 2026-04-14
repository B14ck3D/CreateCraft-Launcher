const path = require('path');
const fs = require('fs');
const { FusesPlugin } = require('@electron-forge/plugin-fuses');
const { FuseV1Options, FuseVersion } = require('@electron/fuses');

module.exports = {
  packagerConfig: {
    asar: true,
    name: 'createcrafts-installer',
    executableName: 'CreateCrafts Launcher',
    extraResource: (() => {
      const icon = path.join(__dirname, 'public', 'icon.png');
      const srv = path.join(__dirname, 'build', 'createcrafts-servers-default.dat');
      const plainKey = path.join(__dirname, 'build', 'launcher-mods-key');
      const encKey = path.join(__dirname, 'build', 'launcher-mods-key.enc');
      const out = [];
      if (fs.existsSync(icon)) out.push(icon);
      if (fs.existsSync(srv)) out.push(srv);
      if (fs.existsSync(plainKey)) out.push(plainKey);
      if (fs.existsSync(encKey)) out.push(encKey);
      return out;
    })(),
  },
  rebuildConfig: {},
  makers: [],
  plugins: [
    {
      name: '@electron-forge/plugin-vite',
      config: {
        build: [
          {
            entry: 'src/main.js',
            config: 'vite.main.config.mjs',
            target: 'main',
          },
          {
            entry: 'src/preload.js',
            config: 'vite.preload.config.mjs',
            target: 'preload',
          },
        ],
        renderer: [
          {
            name: 'main_window',
            config: 'vite.renderer.config.mjs',
          },
        ],
      },
    },
    new FusesPlugin({
      version: FuseVersion.V1,
      [FuseV1Options.RunAsNode]: false,
      [FuseV1Options.EnableCookieEncryption]: true,
      [FuseV1Options.EnableNodeOptionsEnvironmentVariable]: false,
      [FuseV1Options.EnableNodeCliInspectArguments]: false,
      [FuseV1Options.EnableEmbeddedAsarIntegrityValidation]: true,
      [FuseV1Options.OnlyLoadAppFromAsar]: true,
    }),
  ],
  hooks: {
    packageAfterCopy: async (config, buildPath, electronVersion, platform, arch) => {
      const fs = require('fs');
      const path = require('path');
      const { execSync } = require('child_process');

      console.log('Restoring node_modules in buildPath:', buildPath);

      fs.copyFileSync(path.join(__dirname, 'package.json'), path.join(buildPath, 'package.json'));

      execSync('npm install --omit=dev --legacy-peer-deps', {
        cwd: buildPath,
        stdio: 'inherit',
        shell: true,
      });
    },
  },
};
