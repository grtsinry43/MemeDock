import { sveltekit } from '@sveltejs/kit/vite';
import { defineConfig } from 'vite';
import adapter from '@sveltejs/adapter-static';
import tailwindcss from '@tailwindcss/vite';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';

const themeScript = readFileSync(new URL('./src/app.html', import.meta.url), 'utf8').match(/<script id="theme-init">([\s\S]*?)<\/script>/)?.[1];
if (!themeScript) throw new Error('Missing theme initialization script');
const themeHash: `sha256-${string}` = `sha256-${createHash('sha256').update(themeScript).digest('base64')}`;

export default defineConfig({
  plugins: [tailwindcss(), sveltekit({
    adapter: adapter({ pages: 'dist', assets: 'dist', fallback: 'index.html' }),
    // Bits UI positions floating content and locks scrolling with dynamic styles.
    // Scripts retain hash-based CSP; fonts are emitted as same-origin files.
    csp: { mode: 'hash', directives: { 'default-src': ['self'], 'script-src': ['self', themeHash], 'style-src': ['self', 'unsafe-inline'], 'base-uri': ['self'], 'form-action': ['self'] } }
  })],
  build: { assetsInlineLimit: 0 },
  server: { proxy: { '/api': 'http://127.0.0.1:8787' } }
});
