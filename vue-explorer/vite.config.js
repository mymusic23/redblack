// vite.config.js
import {defineConfig} from 'vite'
import vue from '@vitejs/plugin-vue'
import wasm from "vite-plugin-wasm"
import topLevelAwait from "vite-plugin-top-level-await"

export default defineConfig({
    plugins: [vue(), wasm(), topLevelAwait()],
    server: {
        proxy: {
            '/api': 'http://localhost:16481'
        }
    },
    optimizeDeps: {
        exclude: ['redgold_gui.wasm']
    },
    build: {
        target: 'esnext',
    },
    resolve: {
        alias: {
            '@': '/src'
        }
    }
})
