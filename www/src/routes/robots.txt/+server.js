import { site } from '$lib/catalog.js';

export const prerender = true;
export const trailingSlash = 'never';

export function GET() {
	// The gallery is a 17MB wasm app behind an iframe, and there is no text in
	// it — a crawler that pulls it repeatedly gets nothing and costs bandwidth.
	const text = [
		`# Documentation for agents: ${site}/llms.txt`,
		'',
		'User-agent: *',
		'Allow: /',
		'Disallow: /gallery/',
		'',
		`Sitemap: ${site}/sitemap.xml`
	].join('\n');

	return new Response(`${text}\n`, {
		headers: { 'content-type': 'text/plain; charset=utf-8' }
	});
}
