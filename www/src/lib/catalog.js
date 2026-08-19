import catalog from './catalog.json';

export { catalog };

/** Every section, flattened, each remembering the tab and group it came from. */
export const sections = catalog.flatMap((tab) =>
	tab.groups.flatMap((group) =>
		group.sections.map((section) => ({
			...section,
			tab: tab.title,
			group: group.title,
			// A pattern is a screen, not a control: it wants the whole pane in the
			// gallery, and it wants a taller frame on the page that documents it.
			fullBleed: tab.fullBleed
		}))
	)
);

export const bySlug = new Map(sections.map((section) => [section.key, section]));

const FRONTMATTER = /^---\r?\n([\s\S]*?)\r?\n---\r?\n?/;

/** `description:` becomes the page's meta description — one unique line each. */
function split(raw) {
	const match = raw.match(FRONTMATTER);
	if (!match) return { meta: {}, body: raw };
	const meta = Object.fromEntries(
		match[1]
			.split('\n')
			.filter((line) => line.includes(':'))
			.map((line) => {
				const at = line.indexOf(':');
				return [line.slice(0, at).trim(), line.slice(at + 1).trim()];
			})
	);
	return { meta, body: raw.slice(match[0].length) };
}

/** Prose keyed by slug. The catalog says what exists; this says what is written. */
export const prose = Object.fromEntries(
	Object.entries(
		import.meta.glob('/docs/*.md', { query: '?raw', import: 'default', eager: true })
	).map(([path, raw]) => [path.slice('/docs/'.length, -'.md'.length), split(raw)])
);

// The catalog is the source of truth, so prose with no section behind it is a
// build error rather than an orphaned page nobody can reach.
for (const slug of Object.keys(prose)) {
	if (!bySlug.has(slug)) {
		throw new Error(`www/docs/${slug}.md has no matching section in the gallery catalog`);
	}
}

/**
 * Only sections that have been written. A rail of "no prose yet" placeholders
 * reads as an abandoned site, so the sidebar grows as the docs do rather than
 * advertising everything still missing.
 */
export const documented = catalog
	.map((tab) => ({
		...tab,
		groups: tab.groups
			.map((group) => ({
				...group,
				sections: group.sections.filter((section) => section.key in prose)
			}))
			.filter((group) => group.sections.length > 0)
	}))
	.filter((tab) => tab.groups.length > 0);

export const documentedSections = documented.flatMap((tab) =>
	tab.groups.flatMap((group) => group.sections)
);

/** Where the "Docs" link points, so it never lands on a page we have not written. */
export const docsHome = documentedSections[0]?.key;

export const repo = 'https://github.com/crabtalk/bezel';

export const repoApi = repo.replace('https://github.com/', 'https://api.github.com/repos/');

/** Canonical URL, used for share links. Swap to bezel.gallery once its DNS is on. */
export const site = 'https://bezel.pages.dev';
