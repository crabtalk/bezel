import adapter from '@sveltejs/adapter-static';

// No `paths.base` on purpose: SvelteKit emits relative links by default, so the
// same build serves from crabtalk.github.io/bezel and from a local preview
// without a prefix to keep in sync.
export default {
	kit: {
		adapter: adapter({ fallback: '404.html' }),
		prerender: {
			handleHttpError: ({ path, message }) => {
				// The gallery is the wasm app under `static/`, not a route. Pages
				// resolves the directory to its index.html; the crawler does not,
				// so it is the one 404 that means nothing. Every other one is real.
				if (path === '/gallery/') return;
				throw new Error(message);
			}
		}
	}
};
