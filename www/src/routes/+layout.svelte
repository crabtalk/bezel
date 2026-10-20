<script>
	import { siDiscord, siGithub, siX } from 'simple-icons';
	import '../app.css';
	import { base } from '$app/paths';
	import { page } from '$app/state';
	import Brand from '$lib/Brand.svelte';
	import { discord, docsHome, documented, repo } from '$lib/catalog.js';

	const author = 'https://x.com/tianyi_gc';

	let { children } = $props();

	// One header for the whole site: there is no marketing/docs split here to
	// signal, so docs get the same chrome with the catalog's tabs added to it.
	//
	// Matched on the route id, not the pathname: `paths.relative` makes `base` a
	// per-page prefix like `../..`, so comparing against it never matches.
	const inDocs = $derived(page.route.id?.startsWith('/docs') ?? false);

	// Same axis the gallery uses: the tabs are the *kind* of thing, the rail is
	// what is in it. This is a docs site, so they are the site's nav and sit in
	// the header everywhere — a "Docs" link beside them would point at the first
	// tab's first page, which is the first tab.
	const tabs = $derived(documented);

	const activeTab = $derived(
		inDocs
			? (documented.find((tab) =>
					tab.groups.some((group) => group.sections.some((s) => s.key === page.params.slug))
				) ?? documented[0])
			: null
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

<svelte:head>
	<link rel="alternate" type="text/plain" href="{base}/llms.txt" title="Documentation for LLMs" />
</svelte:head>

<header>
	<div class="bar" class:wide={inDocs}>
		<a class="wordmark" href="{base}/">
			<img src="{base}/favicon.svg" alt="" width="22" height="22" />
			bezel
		</a>

		<!-- Nav sits with the wordmark, not opposite it: the left group is where you
		     are in the project, the right group is what you can do about it. -->
		<nav class="nav">
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
			<a class="mark" href={discord} target="_blank" rel="noreferrer" aria-label="bezel on Discord">
				<Brand icon={siDiscord} size={15} />
			</a>
			<a class="mark" href={repo} aria-label="bezel on GitHub">
				<Brand icon={siGithub} size={15} />
			</a>
		</nav>
	</div>
</header>

<!-- At least the window's height under the bar, so a short page keeps its foot
     at the bottom of the window. -->
<div class="page">
	<main>
		{@render children()}
	</main>

	{#if !inDocs}
		<footer>
			<nav class="left">
				<a href="https://github.com/crabtalk">crabtalk</a>
				{#if docsHome}<a href="{base}/docs/{docsHome}/">Docs</a>{/if}
				<a href="{repo}/tree/main/skills">Skills</a>
			</nav>
			<nav class="right">
				<a href={repo} aria-label="bezel on GitHub"><Brand icon={siGithub} size={15} /></a>
				<a href={discord} target="_blank" rel="noreferrer" aria-label="bezel on Discord">
					<Brand icon={siDiscord} size={15} />
				</a>
				<a href={author} target="_blank" rel="noreferrer" aria-label="The author on X">
					<Brand icon={siX} size={14} />
				</a>
			</nav>
		</footer>
	{/if}
</div>

<style>
	/* A band of glass across the window, with the page's own column inside it. */
	header {
		position: sticky;
		top: 0;
		z-index: 20;
		height: var(--header);
		border-bottom: 1px solid var(--line);
		background: color-mix(in srgb, var(--bg) 60%, transparent);
		backdrop-filter: blur(20px) saturate(1.8);
		-webkit-backdrop-filter: blur(20px) saturate(1.8);
		/* The lit edge glass has along its rim. */
		box-shadow: inset 0 -1px 0 rgb(255 255 255 / 0.04);
	}

	.bar {
		display: flex;
		align-items: center;
		gap: 26px;
		height: 100%;
		max-width: calc(1400px + 48px);
		margin: 0 auto;
		padding: 0 24px;
	}

	/* The docs' rail is full bleed, so the bar is too. */
	.bar.wide {
		max-width: none;
	}

	.page {
		display: flex;
		flex-direction: column;
		min-height: calc(100dvh - var(--header));
	}

	main {
		flex: 1;
	}

	/* Full bleed; its content keeps the home page's 1200px column. */
	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		padding: 14px max(var(--gutter), calc((100% - 1200px) / 2));
		border-top: 1px solid var(--line);
		background: color-mix(in srgb, var(--bg) 88%, transparent);
		backdrop-filter: blur(12px);
	}

	footer nav {
		display: flex;
		align-items: center;
		gap: 22px;
	}

	/* Drafting-label register: small, spaced and set in the mono face, so it
	   reads as an annotation on the page rather than more prose. */
	footer .left a {
		font-family: var(--mono);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.16em;
	}

	footer a {
		color: var(--faint);
	}

	footer a:hover {
		color: var(--text);
		text-decoration: none;
	}

	.wordmark {
		display: flex;
		align-items: center;
		gap: 8px;
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

	.mark {
		display: flex;
		padding: 4px;
	}


	header a:hover {
		color: var(--text);
		text-decoration: none;
	}

	@media (max-width: 720px) {
		.bar {
			gap: 16px;
			padding: 0 16px;
		}

		footer {
			flex-direction: column;
			align-items: flex-start;
			gap: 10px;
			padding-block: 12px;
		}

		/* The tabs never fit beside the wordmark on a phone, and `min-width: 0`
		   lets them spill over the button on the right rather than clip. Scroll
		   the row instead, so every tab stays reachable. */
		.nav {
			gap: 16px;
			overflow-x: auto;
			scrollbar-width: none;
		}

		.nav::-webkit-scrollbar {
			display: none;
		}
	}
</style>
