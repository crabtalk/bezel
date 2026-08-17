<script>
	import { siGithub } from 'simple-icons';
	import '../app.css';
	import { base } from '$app/paths';
	import { page } from '$app/state';
	import Brand from '$lib/Brand.svelte';
	import { documented, docsHome, repo } from '$lib/catalog.js';

	let { children, data } = $props();

	// One header for the whole site: there is no marketing/docs split here to
	// signal, so docs get the same chrome with the catalog's tabs added to it.
	//
	// Matched on the route id, not the pathname: `paths.relative` makes `base` a
	// per-page prefix like `../..`, so comparing against it never matches.
	const inDocs = $derived(page.route.id?.startsWith('/docs') ?? false);

	// Same axis the gallery uses: the tabs are the *kind* of thing, the rail is
	// what is in it. One tab is not a choice, so the row waits until there are two.
	const tabs = $derived(inDocs && documented.length > 1 ? documented : []);

	const activeTab = $derived(
		documented.find((tab) =>
			tab.groups.some((group) => group.sections.some((s) => s.key === page.params.slug))
		) ?? documented[0]
	);

	const home = (tab) => tab.groups[0].sections[0].key;

	// One delegated handler for the whole site. The copy buttons in docs are
	// generated inside `{@html}`, so there is no markup to bind an `onclick` to;
	// the hero's install block reuses the same classes and gets it for free.
	async function copy(event) {
		const button = event.target.closest?.('.copy');
		if (!button) return;
		const code = button.parentElement?.querySelector('code');
		if (!code) return;
		try {
			await navigator.clipboard.writeText(code.textContent ?? '');
			button.classList.add('copied');
			setTimeout(() => button.classList.remove('copied'), 1400);
		} catch (error) {
			console.warn('copy failed', error);
		}
	}

	$effect(() => {
		document.addEventListener('click', copy);
		return () => document.removeEventListener('click', copy);
	});
</script>

<header>
	<a class="wordmark" href="{base}/">bezel</a>

	<!-- Nav sits with the wordmark, not opposite it: the left group is where you
	     are in the project, the right group is what you can do about it. -->
	<nav class="nav">
		{#if docsHome}
			<a href="{base}/docs/{docsHome}/" class:current={inDocs}>Docs</a>
		{/if}
		{#each tabs as tab (tab.title)}
			<a
				href="{base}/docs/{home(tab)}/"
				class:current={tab.title === activeTab?.title}
				aria-current={tab.title === activeTab?.title ? 'page' : undefined}
			>
				{tab.title}
			</a>
		{/each}
	</nav>

	<nav class="links">
		<a class="action" href={repo}>
			Star
			<Brand icon={siGithub} size={14} />
			{#if data.stars !== null}<span class="count">{data.stars}</span>{/if}
		</a>
	</nav>
</header>

{@render children()}

<style>
	header {
		display: flex;
		align-items: center;
		gap: 26px;
		padding: 0 24px;
		height: 56px;
		border-bottom: 1px solid var(--line);
		position: sticky;
		top: 0;
		background: color-mix(in srgb, var(--bg) 88%, transparent);
		backdrop-filter: blur(12px);
		z-index: 20;
	}

	.wordmark {
		display: flex;
		align-items: center;
		color: var(--text);
		font-weight: 600;
		letter-spacing: -0.02em;
	}

	/* No underline on the active item — colour alone carries it. An indicator
	   rail here would read as tabs owning the page, which they do not: the rail
	   below already says where you are. */
	.nav {
		display: flex;
		align-items: center;
		gap: 20px;
		min-width: 0;
	}

	.nav a {
		color: var(--muted);
		font-size: 14px;
		font-weight: 500;
		white-space: nowrap;
	}

	.nav a.current {
		color: var(--text);
	}

	.links {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-left: auto;
	}

	.links a {
		color: var(--muted);
		font-size: 14px;
	}


	/* The label names the verb, the mark names the destination — so spelling the
	   destination out as well would say it twice. */
	.action {
		display: inline-flex;
		align-items: center;
		gap: 7px;
		height: 32px;
		padding: 0 11px;
		margin-left: 4px;
		border: 1px solid var(--line-strong);
		border-radius: 6px;
		color: var(--text);
	}

	.action:hover {
		border-color: var(--text);
		text-decoration: none;
	}

	.count {
		font-variant-numeric: tabular-nums;
		color: var(--muted);
		border-left: 1px solid var(--line-strong);
		padding-left: 8px;
	}

	header a:hover {
		color: var(--text);
		text-decoration: none;
	}
</style>
