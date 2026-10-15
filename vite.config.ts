import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: { include: ['@xterm/xterm', '@xterm/addon-fit'] },
  server: { port: 1420, strictPort: true },
  clearScreen: false,
});
