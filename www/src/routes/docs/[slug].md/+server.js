import { error } from '@sveltejs/kit';
import { bySlug, documentedSections, prose, site, sourceUrl } from '$lib/catalog.js';

// The page's own source, served as itself. A reader who is a program gets the
// markdown that `+page.server.js` renders to HTML, not the HTML.
export const prerender = true;
// The root layout asks for `always`, which would make this a directory.
export const trailingSlash = 'never';

export function entries() {
	return documentedSections.map((section) => ({ slug: section.key }));
}

export function GET({ params }) {
	const section = bySlug.get(params.slug);
	const doc = prose[params.slug];
	if (!section || !doc) {
		error(404, `no documented component "${params.slug}"`);
	}
	const front = Object.entries({ ...doc.meta, source: sourceUrl(section) })
		.map(([key, value]) => `${key}: ${value}`)
		.join('\n');
	// Fetched on its own, a page still has to say where the rest of them are.
	const text = [
		`---\n${front}\n---`,
		`> One page of the bezel documentation. The index is ${site}/llms.txt`,
		`# ${section.title}`,
		doc.body.trim()
	].join('\n\n');
	return new Response(`${text}\n`, {
		headers: { 'content-type': 'text/markdown; charset=utf-8' }
	});
}
