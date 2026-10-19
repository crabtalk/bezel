<script>
	import { base } from '$app/paths';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.section.title} — bezel, a component library for gpui</title>
	{#if data.description}<meta name="description" content={data.description} />{/if}
	<link rel="alternate" type="text/markdown" href="{base}/docs/{data.section.key}.md" />
</svelte:head>

<article>
	<!-- eslint-disable-next-line svelte/no-at-html-tags -->
	{@html data.html}
</article>
