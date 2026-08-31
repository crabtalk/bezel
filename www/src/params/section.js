import { documentedSections } from '$lib/catalog.js';

// The slugs `entries` prerenders, and so the only ones `/docs/<slug>` is a page
// for. Without this the client router claims `/docs/<slug>.md` as well: the
// markdown endpoint is not in its manifest, `[slug]` matches a dotted segment
// as happily as a bare one, and the __data.json it then fetches was never
// built. The catalog is already in the client bundle for the header's tabs.
const KEYS = new Set(documentedSections.map((section) => section.key));

export function match(param) {
	return KEYS.has(param);
}
