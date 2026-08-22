import readme from '../../../../README.md?raw';
import { documented, prose, site, sourceUrl, tagline } from '$lib/catalog.js';

// The whole corpus, one fetch — 88KB of prose does not need a search endpoint.
export const prerender = true;
export const trailingSlash = 'never';

export function GET() {
	const docs = documented.flatMap((tab) =>
		tab.groups.flatMap((group) =>
			group.sections.map((section) =>
				[
					`# ${section.title}`,
					`Source: ${sourceUrl(section)}`,
					prose[section.key].body.trim()
				].join('\n\n')
			)
		)
	);

	const text = [
		[
			'# bezel documentation for LLMs',
			`> ${tagline}`,
			`The README, then every page in the order the site presents them. Each is also served on its own at ${site}/docs/<slug>.md`
		].join('\n\n'),
		readme.trim(),
		...docs
	].join('\n\n---\n\n');

	return new Response(`${text}\n`, {
		headers: { 'content-type': 'text/plain; charset=utf-8' }
	});
}
