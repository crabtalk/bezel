import { error } from '@sveltejs/kit';
import { marked } from 'marked';
import { codeBlock } from '$lib/server/highlight.js';
import { bySlug, documentedSections, prose, skillInstall } from '$lib/catalog.js';

// A server load, so marked runs at build time and never ships to a reader.
marked.use({
	renderer: {
		code({ text, lang }) {
			return codeBlock(text, lang);
		}
	}
});

export function entries() {
	return documentedSections.map((section) => ({ slug: section.key }));
}

export function load({ params }) {
	const section = bySlug.get(params.slug);
	const doc = prose[params.slug];
	if (!section || !doc) {
		error(404, `no documented component "${params.slug}"`);
	}
	return {
		section,
		description: doc.meta.description ?? null,
		ask: doc.meta.prompt
			? { install: codeBlock(skillInstall, 'bash'), prompt: codeBlock(doc.meta.prompt, null) }
			: null,
		html: marked.parse(doc.body)
	};
}
