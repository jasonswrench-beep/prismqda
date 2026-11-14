import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// `npm run dev` proxies /rpc to the development bridge (`prismqda serve`, port 4891 by default), so
// the screens can be tried against the real engine. A Tauri build ignores the proxy.
export default defineConfig({
  base: './',
  plugins: [react()],
  build: {
    outDir: 'dist',
    sourcemap: false,
    chunkSizeWarningLimit: 900,
  },
  server: {
    // The brand artwork lives in ../../assets, outside this folder, so the dev server has to be allowed to read it.
    fs: { allow: ['../..'] },
    proxy: {
      '/rpc': { target: process.env.PRISMQDA_BRIDGE ?? 'http://127.0.0.1:4891', changeOrigin: false },
    },
  },
});
