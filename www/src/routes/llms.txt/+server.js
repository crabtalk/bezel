import { documented, documentedSections, prose, site, tagline } from '$lib/catalog.js';

// llmstxt.org: an H1, a blockquote, then H2 sections of links with notes. The
// notes are the `description` each page already carries for search.
export const prerender = true;
export const trailingSlash = 'never';

export function GET() {
	const tabs = documented.map((tab) => {
		const links = tab.groups.flatMap((group) =>
			group.sections.map(
				(section) =>
					`- [${section.title}](${site}/docs/${section.key}.md): ${prose[section.key].meta.description}`
			)
		);
		return `## ${tab.title}\n\n${links.join('\n')}`;
	});

	const text = [
		'# bezel',
		`> ${tagline}`,
		'Every page below is served as Markdown — append `.md` to any docs URL.',
		'## Start here',
		[
			`- [README](${site}/readme.md): the dependency line, the pinned gpui fork, and the bootstrap no snippet can skip`,
			`- [Complete documentation](${site}/llms-full.txt): the README and all ${documentedSections.length} pages in one file`,
			`- [Homepage](${site}/): what the library is, in its own terms`
		].join('\n'),
		...tabs
	].join('\n\n');

	return new Response(`${text}\n`, {
		headers: { 'content-type': 'text/plain; charset=utf-8' }
	});
}
