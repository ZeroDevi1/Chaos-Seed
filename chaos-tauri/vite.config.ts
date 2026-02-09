import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { vitePreprocess } from '@sveltejs/vite-plugin-svelte'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [
    svelte({
      preprocess: vitePreprocess()
    })
  ],
  clearScreen: false,
  optimizeDeps: {
    // libmedia packages are large and can be incompatible with Vite's dep optimizer on some
    // Windows setups (missing chunk files under `node_modules/.vite/deps/*`).
    // Keep them excluded so the dev server serves them as-is.
    include: ['hls.js'],
    exclude: [
      '@libmedia/avplayer',
      '@libmedia/avutil',
      '@libmedia/avcodec',
      '@libmedia/avformat',
      '@libmedia/avpipeline',
      '@libmedia/avnetwork',
      '@libmedia/avrender',
      '@libmedia/avprotocol',
      '@libmedia/common',
      '@libmedia/cheap',
      '@libmedia/audioresample',
      '@libmedia/audiostretchpitch'
    ]
  },
  server: {
    strictPort: true,
    port: 5173,
    // Avoid IPv6/localhost resolution differences across WebView2 instances on Windows.
    host: '127.0.0.1'
  },
  resolve: {
    alias: {
      '@': fileURLToPath(new URL('./src', import.meta.url))
    },
    // pnpm can otherwise result in multiple FAST copies in the bundle; keep it single to avoid
    // token/observable weirdness at runtime.
    dedupe: ['@microsoft/fast-element', '@microsoft/fast-foundation', '@fluentui/web-components']
  },
  build: {
    target: 'es2022'
  }
})
