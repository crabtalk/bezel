import { documentedSections, site } from '$lib/catalog.js';

export const prerender = true;
export const trailingSlash = 'never';

export function GET() {
	// Pages only. The `.md` twins are the same prose under another URL, which is
	// a duplicate to a search engine; llms.txt is where they are listed.
	const urls = [`${site}/`, ...documentedSections.map((section) => `${site}/docs/${section.key}/`)];
	const xml = [
		'<?xml version="1.0" encoding="UTF-8"?>',
		'<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">',
		...urls.map((loc) => `\t<url><loc>${loc}</loc></url>`),
		'</urlset>'
	].join('\n');

	return new Response(`${xml}\n`, {
		headers: { 'content-type': 'application/xml; charset=utf-8' }
	});
}
