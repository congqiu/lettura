import { defineConfig } from 'vitest/config'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'
import path from 'path'
import { VitePWA } from 'vite-plugin-pwa'

export default defineConfig({
  plugins: [
    react(),
    tailwindcss(),
    VitePWA({
      registerType: 'autoUpdate',
      injectRegister: false,
      workbox: {
        skipWaiting: true,
        clientsClaim: false,
        navigateFallback: 'index.html',
        navigateFallbackDenylist: [/^\/api\//, /^\/feed\//, /^\/metrics/, /^\/p\//],
        globPatterns: ['**/*.{js,css,html,woff2,svg,png,ico,webmanifest}'],
        runtimeCaching: [
          {
            urlPattern: /^https?:\/\/.+\.(?:png|jpg|jpeg|gif|webp|avif|svg)(\?.*)?$/i,
            handler: 'StaleWhileRevalidate',
            options: {
              cacheName: 'images',
              expiration: { maxEntries: 200, maxAgeSeconds: 30 * 24 * 60 * 60 },
            },
          },
          {
            urlPattern: /\/api\/v1\/entries(?:\/|$)/,
            handler: 'NetworkFirst',
            options: {
              cacheName: 'api-entries',
              expiration: { maxEntries: 100, maxAgeSeconds: 7 * 24 * 60 * 60 },
              networkTimeoutSeconds: 5,
            },
          },
          {
            urlPattern: /\/api\/v1\/tags(?:\/|$)/,
            handler: 'NetworkFirst',
            options: {
              cacheName: 'api-tags',
              expiration: { maxEntries: 20, maxAgeSeconds: 7 * 24 * 60 * 60 },
              networkTimeoutSeconds: 5,
            },
          },
          {
            urlPattern: /\/api\/v1\/annotations(?:\/|$)/,
            handler: 'NetworkFirst',
            options: {
              cacheName: 'api-annotations',
              expiration: { maxEntries: 200, maxAgeSeconds: 7 * 24 * 60 * 60 },
              networkTimeoutSeconds: 5,
            },
          },
          {
            urlPattern: /\/api\/v1\/memos(?:\/|$)/,
            handler: 'NetworkFirst',
            options: {
              cacheName: 'api-memos',
              expiration: { maxEntries: 50, maxAgeSeconds: 7 * 24 * 60 * 60 },
              networkTimeoutSeconds: 5,
            },
          },
        ],
        cleanupOutdatedCaches: true,
      },
      manifest: {
        name: 'Lettura',
        short_name: 'Lettura',
        description: 'Self-hosted read-it-later app',
        theme_color: '#fafaf9',
        background_color: '#fafaf9',
        display: 'standalone',
        start_url: '/',
        scope: '/',
        icons: [
          { src: 'pwa-64x64.png', sizes: '64x64', type: 'image/png' },
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
          {
            src: 'maskable-icon-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
        share_target: {
          action: '/share-target',
          method: 'GET',
          enctype: 'application/x-www-form-urlencoded',
          params: {
            url: 'url',
            text: 'text',
          },
        },
      },
    }),
  ],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
  server: {
    proxy: {
      '/api': 'http://localhost:3330',
      '/feed': 'http://localhost:3330',
      '/p': 'http://localhost:3330',
    },
  },
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: './src/test-setup.ts',
  },
})
