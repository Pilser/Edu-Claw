import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
  ],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
  },
  server: {
    // Proxy API and WS calls to the backend during development
    proxy: {
      '/auth':     'http://localhost:3000',
      '/sessions': {
        target:    'http://localhost:3000',
        ws:        true,
      },
      '/files':    'http://localhost:3000',
    },
  },
})
