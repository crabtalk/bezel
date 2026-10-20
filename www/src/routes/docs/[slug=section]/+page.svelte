<script>
	import { base } from '$app/paths';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.section.title} — bezel, a component library for gpui</title>
	{#if data.description}<meta name="description" content={data.description} />{/if}
	<link rel="alternate" type="text/markdown" href="{base}/docs/{data.section.key}.md" />
</svelte:head>

{#if data.ask}
	<section class="ask">
		<h2>Ask your agent</h2>
		<p class="step">Install the bezel skill once</p>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html data.ask.install}
		<p class="step">Then ask</p>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html data.ask.prompt}
	</section>
{/if}

<article>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html data.html}
</article>

<style>
	.ask {
		margin: 0 0 32px;
	}

	.ask h2 {
		margin: 0 0 12px;
		font-size: 18px;
	}

	.step {
		margin: 12px 0 6px;
		color: var(--muted);
		font-size: 13px;
	}

	/* A prompt is prose: it wraps rather than scrolls. */
	.ask :global(pre) {
		white-space: pre-wrap;
	}
</style>
