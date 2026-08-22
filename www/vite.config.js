import { sveltekit } from '@sveltejs/kit/vite';

// `vite dev` serves static files by exact path, so the link that leaves
// SvelteKit for the wasm app needs what Cloudflare does for it in production:
// redirect the bare path — the loader's `./web.js` resolves against the
// trailing slash — and then serve the directory index.
const gallery = {
	name: 'gallery-dev-host',
	configureServer(server) {
		server.middlewares.use((req, res, next) => {
			const [path, query] = req.url.split('?');
			const suffix = query ? `?${query}` : '';
			if (path === '/gallery') {
				res.writeHead(308, { location: `/gallery/${suffix}` });
				res.end();
				return;
			}
			if (path.startsWith('/gallery/') && path.endsWith('/')) {
				req.url = `${path}index.html${suffix}`;
			}
			next();
		});
	}
};

export default {
	plugins: [gallery, sveltekit()],
	// The README is served as itself and heads `llms-full.txt`, and it lives a
	// directory above: bun.lock is in here, so Vite roots the dev server at www.
	server: { fs: { allow: ['..'] } }
};
