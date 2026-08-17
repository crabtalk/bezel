import catalog from './catalog.json';

export { catalog };

/** Every section, flattened, each remembering the tab and group it came from. */
export const sections = catalog.flatMap((tab) =>
	tab.groups.flatMap((group) =>
		group.sections.map((section) => ({ ...section, tab: tab.title, group: group.title }))
	)
);

export const bySlug = new Map(sections.map((section) => [section.key, section]));

export const repo = 'https://github.com/crabtalk/bezel';
