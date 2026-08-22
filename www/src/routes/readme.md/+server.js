import readme from '../../../../README.md?raw';

// The install line, the pinned gpui fork and the bootstrap live here and only
// here. Served verbatim so there is no second copy to go stale.
export const prerender = true;
export const trailingSlash = 'never';

export function GET() {
	return new Response(readme, {
		headers: { 'content-type': 'text/markdown; charset=utf-8' }
	});
}
