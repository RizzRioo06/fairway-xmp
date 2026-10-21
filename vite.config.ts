import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: { include: ['@xterm/xterm', '@xterm/addon-fit', '@xterm/addon-search'] },
  // Failed modulepreload requests can poison WebKit's resource cache across
  // reloads (webkit.org/b/270357). Keep lazy import() and CSS preloading, and
  // let an explicit reload retry failed JavaScript without modulepreload.
  build: { modulePreload: false },
  server: {
    port: 1420, strictPort: true,
    watch: { ignored: ['**/src-tauri/**', '**/.runtime-work/**', '**/.build-tools/**', '**/artifacts/**', '**/playwright-report/**', '**/test-results/**'] },
  },
  clearScreen: false,
});
