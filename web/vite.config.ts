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
      '/api': 'http://localhost:3005',
      '/subscriptions': 'http://localhost:3005',
      '/search.json': 'http://localhost:3005',
      '/toplist': 'http://localhost:3005',
      '/health': 'http://localhost:3005',
      '/suggestions': 'http://localhost:3005',
    }
  }
});
