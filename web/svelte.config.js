import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
  kit: {
    adapter: adapter({
      pages: 'dist',
      assets: 'dist',
      fallback: 'index.html', // SPA mode
    }),
    paths: {
      // When embedded in rpodder, the UI is served from /ui/
      // When standalone, it's served from /
      base: process.env.RPODDER_UI_BASE || ''
    }
  }
};

export default config;
