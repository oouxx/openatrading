import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from 'vite-plugin-wasm'
import topLevelAwait from 'vite-plugin-top-level-await'

export default defineConfig({
  plugins: [
    vue(),
    wasm(),
    topLevelAwait(),
  ],
  define: {
    __OPENROUTER_KEY__: JSON.stringify(process.env.OPENROUTER_API_KEY || ''),
  },
})
