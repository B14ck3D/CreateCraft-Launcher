import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// https://vitejs.dev/config
export default defineConfig({
  /** Wymagane pod Electron `loadFile` — ścieżki absolutne `/x.png` nie działają z protokołem `file://`. */
  base: './',
  plugins: [react()],
});
