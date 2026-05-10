import { defineConfig } from 'vitest/config';
import path from 'path';

export default defineConfig({
  test: {
    environment: 'jsdom',
    globals: true,
    include: ['src/**/*.test.{ts,tsx}'],
    setupFiles: './src/test-setup.ts',
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
      'virtual:pwa-register': path.resolve(__dirname, 'src/pwa/__mocks__/pwa-register.ts'),
    },
  },
});