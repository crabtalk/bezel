import { repoApi } from '$lib/catalog.js';

// The count baked into the page at build time; the layout refreshes it live on
// hydration and falls back to this when that fetch fails.
//
// A server load, so it never runs in a browser and can read a token. The
// unauthenticated GitHub API allows 60 requests an hour per IP — enough until
// you rebuild in a loop or share an IP, and then the count silently disappears
// because this fails soft. `GITHUB_TOKEN` raises that to 5000; `gh auth token`
// prints one.
export async function load() {
	const token = process.env.GITHUB_TOKEN;
	try {
		const response = await fetch(repoApi, {
			headers: token ? { authorization: `Bearer ${token}` } : {}
		});
		if (!response.ok) {
			console.warn(`[stars] GitHub returned ${response.status}; the count will be hidden`);
			return { stars: null };
		}
		const { stargazers_count } = await response.json();
		return { stars: stargazers_count ?? null };
	} catch (error) {
		console.warn(`[stars] ${error}; the count will be hidden`);
		return { stars: null };
	}
}
