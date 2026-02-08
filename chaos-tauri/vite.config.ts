import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import { fileURLToPath, URL } from 'node:url'

export default defineConfig({
  plugins: [
    vue({
      template: {
        compilerOptions: {
          // Treat Fluent custom elements as native elements.
          isCustomElement: (tag) => tag.startsWith('fluent-')
        }
      }
    })
  ],
  clearScreen: false,
  server: {
    strictPort: true,
    port: 5173
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
