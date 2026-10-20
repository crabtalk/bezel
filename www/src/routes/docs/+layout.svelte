<script>
	import { untrack } from 'svelte';
	import { base } from '$app/paths';
	import { page } from '$app/state';
	import Gallery from '$lib/Gallery.svelte';
	import { engage } from '$lib/engage.js';
	import { bySlug, docsUrl, documented, sourceUrl } from '$lib/catalog.js';

	let { children } = $props();

	const section = $derived(bySlug.get(page.params.slug));

	// One running gallery for every docs page. The layout outlives a client
	// navigation, so the embed is loaded once at the page it opened on and is
	// told about every page after that by message rather than by a new `src`.
	const src = `${base}/gallery/?s=${untrack(() => page.params.slug)}`;
	let frame = $state(null);
	let expanded = $state(false);

	const show = () =>
		frame?.contentWindow?.postMessage(
			new URLSearchParams({ s: page.params.slug }).toString(),
			location.origin
		);

	$effect(show);

	// The gallery says `ready` once it listens; a page reached while it loaded
	// is sent again then.
	$effect(() => {
		const ready = (event) => {
			if (event.source === frame?.contentWindow && event.data === 'ready') show();
		};
		window.addEventListener('message', ready);
		return () => window.removeEventListener('message', ready);
	});

	// The tabs live in the site header; the rail is whatever is inside the one
	// you are on.
	const activeTab = $derived(
		documented.find((tab) =>
			tab.groups.some((group) => group.sections.some((s) => s.key === page.params.slug))
		) ?? documented[0]
	);
</script>

<div class="shell">
	<aside>
		{#if activeTab}
			{#each activeTab.groups as group (group.title)}
				<h2>{group.title}</h2>
				<ul>
					{#each group.sections as item (item.key)}
						<li>
							<a
								href="{base}/docs/{item.key}/"
								class:current={page.params.slug === item.key}
								aria-current={page.params.slug === item.key ? 'page' : undefined}
							>
								{item.title}
							</a>
						</li>
					{/each}
				</ul>
			{/each}
		{/if}
	</aside>
	<main>
		<div class="reading">
			{#if section}
				<p class="crumb">{section.tab} / {section.group}</p>
				<h1>{section.title}</h1>

				<!-- The component itself, not the whole browser: one shared wasm app,
				     told which section to render. `loading="lazy"` keeps 17MB off the
				     page until the reader scrolls to it. -->
				<div class="embed" use:engage={() => (expanded = true)}>
					<iframe
						bind:this={frame}
						class:tall={section.fullBleed}
						title="{section.title}, running"
						{src}
						loading="lazy"
					></iframe>
					<button class="cover" aria-label="Use {section.title}"></button>
				</div>

				<p class="actions">
					<button class="button" onclick={() => (expanded = true)}>Expand</button>
					<!-- How you hand this page to an agent: the same prose, as its source. -->
					<a class="markdown" href="{base}/docs/{section.key}.md">Markdown</a>
					{#if section.source}
						<a class="source" href={sourceUrl(section)}>
							<code>{section.source}</code>
						</a>
					{/if}
					<!-- The full surface. This page carries the part worth reading first;
					     rustdoc has every method, in alphabetical order, with no view on
					     which of them matter. -->
					{#if docsUrl(section)}
						<a class="markdown" href={docsUrl(section)}>docs.rs</a>
					{/if}
				</p>

				<Gallery title={section.title} src="{base}/gallery/?s={section.key}" bind:open={expanded} />
			{/if}
			{@render children()}
		</div>
	</main>
</div>

<style>

	.crumb {
		color: var(--muted);
		font-size: 13px;
		margin: 0;
	}

	h1 {
		margin: 4px 0 16px;
		font-size: 32px;
	}

	.embed {
		position: relative;
		margin: 0 0 14px;
	}

	iframe {
		display: block;
		width: 100%;
		height: 460px;
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
		background: var(--panel);
	}

	/* A full-bleed page is a whole screen. At 460px the music player's transport
	   and its queue fight for the same rows, which is not what the screen looks
	   like. */
	iframe.tall {
		height: min(76vh, 720px);
	}

	.actions {
		display: flex;
		align-items: center;
		gap: 14px;
		flex-wrap: wrap;
		margin: 0 0 32px;
	}

	.button {
		font: inherit;
		font-size: 14px;
		cursor: pointer;
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: var(--radius-control);
		padding: 6px 12px;
		color: var(--text);
	}

	.button:hover {
		border-color: var(--line-strong);
		text-decoration: none;
	}

	.markdown {
		font-size: 14px;
		color: var(--muted);
	}

	.source code {
		color: var(--muted);
	}

	/* Full bleed on purpose: the rail belongs to the window, not to the column
	   it sits beside. Only the prose gets a reading width, and it centres in
	   whatever space is left rather than dragging the rail inward with it. */
	.shell {
		display: flex;
		align-items: flex-start;
		min-height: calc(100vh - var(--header));
	}

	aside {
		position: sticky;
		top: var(--header);
		flex: 0 0 204px;
		height: calc(100vh - var(--header));
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: transparent transparent;
		border-right: 1px solid var(--line);
		padding: 24px 12px 48px;
	}

	/* The rail scrolls, but a permanent track down the page reads as a border
	   rather than as chrome. */
	aside:hover {
		scrollbar-color: var(--line) transparent;
	}

	aside h2 {
		font-size: 12px;
		font-weight: 600;
		text-transform: uppercase;
		letter-spacing: 0.06em;
		color: var(--muted);
		margin: 22px 0 8px;
	}

	aside h2:first-child {
		margin-top: 0;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		display: grid;
		gap: 2px;
	}

	li a {
		display: block;
		padding: 5px 8px;
		border-radius: var(--radius-control);
		color: var(--muted);
		font-size: 14px;
	}

	li a:hover {
		background: var(--panel);
		color: var(--text);
		text-decoration: none;
	}

	li a.current {
		background: var(--panel);
		color: var(--text);
		font-weight: 500;
	}

	main {
		flex: 1;
		min-width: 0;
	}

	/* The embed, code and tables take the whole column; text keeps a reading
	   measure inside it. */
	.reading {
		max-width: 1120px;
		margin: 0 auto;
		padding: 32px 40px 96px;
	}

	.reading :global(:is(p, ul, ol, blockquote, h1, h2, h3, h4)) {
		max-width: 72ch;
	}

	@media (max-width: 900px) {
		.shell {
			display: block;
			min-height: 0;
		}

		aside {
			position: static;
			height: auto;
			width: auto;
			border-right: 0;
			border-bottom: 1px solid var(--line);
			padding: 20px 24px;
		}

		.reading {
			padding: 24px 24px 72px;
		}
	}
</style>
