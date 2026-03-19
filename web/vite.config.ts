import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [
    tailwindcss(),
    sveltekit(),
  ],
  server: {
    proxy: {
      // Proxy API calls to rpodder backend during development
      '/api': 'http://localhost:3006',
      '/subscriptions': 'http://localhost:3006',
      '/search.json': 'http://localhost:3006',
      '/toplist': 'http://localhost:3006',
      '/health': 'http://localhost:3006',
      '/suggestions': 'http://localhost:3006',
      '/auth': 'http://localhost:3006',
    }
  }
});
