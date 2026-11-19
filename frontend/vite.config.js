import { defineConfig } from 'vite'
import { svelte } from '@sveltejs/vite-plugin-svelte'
import { precompress } from './vite-plugin-precompress.js'

// https://vite.dev/config/
export default defineConfig({
  plugins: [svelte(), precompress()],
  server: {
    hmr: true
  }
})
