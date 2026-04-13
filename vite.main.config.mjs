import { copyFileSync, mkdirSync } from 'fs';
import { dirname, join } from 'path';
import { fileURLToPath } from 'url';
import { defineConfig } from 'vite';

const __dirname = dirname(fileURLToPath(import.meta.url));

export default defineConfig({
  plugins: [
    {
      name: 'copy-java-manager-beside-main',
      closeBundle() {
        const outDir = join(__dirname, '.vite/build');
        const src = join(__dirname, 'src', 'javaManager.js');
        const dest = join(outDir, 'javaManager.js');
        mkdirSync(outDir, { recursive: true });
        copyFileSync(src, dest);
        const modsSrc = join(__dirname, 'src', 'createCraftsModsSync.js');
        copyFileSync(modsSrc, join(outDir, 'createCraftsModsSync.js'));
        copyFileSync(join(__dirname, 'src', 'serversDat.js'), join(outDir, 'serversDat.js'));
        copyFileSync(join(__dirname, 'src', 'profileStore.js'), join(outDir, 'profileStore.js'));
        copyFileSync(
          join(__dirname, 'src', 'gamePackConstants.json'),
          join(outDir, 'gamePackConstants.json')
        );
      },
    },
  ],
});
