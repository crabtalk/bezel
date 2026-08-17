import { error } from '@sveltejs/kit';
import { marked } from 'marked';
import { bySlug, sections } from '$lib/catalog.js';

const prose = import.meta.glob('/docs/*.md', { query: '?raw', import: 'default', eager: true });

// The catalog is the source of truth, so prose with no section behind it is a
// build error rather than an orphaned page nobody can reach.
for (const path of Object.keys(prose)) {
	const slug = path.slice('/docs/'.length, -'.md'.length);
	if (!bySlug.has(slug)) {
		throw new Error(`www/docs/${slug}.md has no matching section in the gallery catalog`);
	}
}

export function entries() {
	return sections.map((section) => ({ slug: section.key }));
}

export function load({ params }) {
	const section = bySlug.get(params.slug);
	if (!section) {
		error(404, `no component "${params.slug}" in the catalog`);
	}
	const raw = prose[`/docs/${params.slug}.md`];
	return { section, html: raw ? marked.parse(raw) : null };
}
