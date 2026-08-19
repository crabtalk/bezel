<script>
	import { base } from '$app/paths';
	import Gallery from '$lib/Gallery.svelte';
	import { repo } from '$lib/catalog.js';

	let { data } = $props();

	const src = $derived(`${base}/gallery/?s=${data.section.key}`);

	let expanded = $state(false);
</script>

<svelte:head>
	<title>{data.section.title} — bezel, a component library for gpui</title>
	{#if data.description}<meta name="description" content={data.description} />{/if}
</svelte:head>

<article>
	<p class="crumb">{data.section.tab} / {data.section.group}</p>
	<h1>{data.section.title}</h1>

	<!-- The component itself, not the whole browser: one shared wasm app, told
	     which section to render. `loading="lazy"` keeps 17MB off the page until
	     the reader scrolls to it. -->
	<iframe
		class:tall={data.section.fullBleed}
		title="{data.section.title}, running"
		{src}
		loading="lazy"
	></iframe>

	<p class="actions">
		<button class="button" onclick={() => (expanded = true)}>Expand</button>
		{#if data.section.source}
			<a class="source" href="{repo}/blob/main/{data.section.source}">
				<code>{data.section.source}</code>
			</a>
		{/if}
	</p>

	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html data.html}
</article>

<Gallery title={data.section.title} {src} bind:open={expanded} />

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

	iframe {
		display: block;
		width: 100%;
		height: 460px;
		border: 1px solid var(--line);
		border-radius: 10px;
		background: var(--panel);
		margin: 0 0 14px;
	}

	/* A pattern is a whole screen. At 460px the music player's transport and its
	   queue fight for the same rows, which is not what the pattern looks like. */
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
		border-radius: 7px;
		padding: 6px 12px;
		color: var(--text);
	}

	.button:hover {
		border-color: var(--line-strong);
		text-decoration: none;
	}

	.source code {
		color: var(--muted);
	}

</style>
