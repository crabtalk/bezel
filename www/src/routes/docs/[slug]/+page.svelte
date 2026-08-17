<script>
	import { base } from '$app/paths';
	import { repo } from '$lib/catalog.js';

	let { data } = $props();

	const src = $derived(`${base}/gallery/?s=${data.section.key}`);

	let dialog = $state(null);
	// The dialog's iframe is a second wasm instance, so it is not created until
	// asked for — and it is torn down on close rather than left running behind
	// the page.
	let expanded = $state(false);

	function expand() {
		expanded = true;
		dialog?.showModal();
	}

	function collapse() {
		dialog?.close();
	}

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
		<!-- A dialog, not a link: navigating to the gallery left the reader in a
		     static page with no way back to the doc they were reading. -->
		<button class="button" onclick={expand}>Expand</button>
		{#if data.section.source}
			<a class="source" href="{repo}/blob/main/{data.section.source}">
				<code>{data.section.source}</code>
			</a>
		{/if}
	</p>

	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html data.html}
</article>

<dialog bind:this={dialog} onclose={() => (expanded = false)}>
	<div class="frame">
		<div class="bar">
			<span class="what">{data.section.title}</span>
			<button class="close" onclick={collapse} aria-label="Close">Esc</button>
		</div>
		{#if expanded}
			<iframe class="big" title="{data.section.title}, running" {src}></iframe>
		{/if}
	</div>
</dialog>

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

	dialog {
		width: min(1240px, 94vw);
		max-width: none;
		height: min(88vh, 900px);
		max-height: none;
		padding: 0;
		border: 1px solid var(--line-strong);
		border-radius: 12px;
		background: var(--bg);
		color: var(--text);
		overflow: hidden;
	}

	dialog::backdrop {
		background: rgb(0 0 0 / 0.72);
		backdrop-filter: blur(3px);
	}

	.frame {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 0 14px;
		height: 40px;
		border-bottom: 1px solid var(--line);
		flex: none;
	}

	.what {
		color: var(--muted);
		font-size: 13px;
	}

	.close {
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		color: var(--muted);
		background: none;
		border: 1px solid var(--line-strong);
		border-radius: 5px;
		padding: 3px 8px;
	}

	.close:hover {
		color: var(--text);
		border-color: var(--text);
	}

	iframe.big {
		flex: 1;
		height: auto;
		margin: 0;
		border: 0;
		border-radius: 0;
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
