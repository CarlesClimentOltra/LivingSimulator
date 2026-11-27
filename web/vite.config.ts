import { defineConfig } from 'vite';

// Relative base: the site works under https://<user>.github.io/<repo>/ and locally.
export default defineConfig({
  base: './',
  worker: { format: 'es' },
  build: { target: 'es2022', chunkSizeWarningLimit: 1500 },
  server: { fs: { allow: ['..'] } },
});
