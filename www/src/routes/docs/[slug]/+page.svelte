<script>
	import { base } from '$app/paths';
	import { repo } from '$lib/catalog.js';

	let { data } = $props();
</script>

<svelte:head>
	<title>{data.section.title} — bezel</title>
</svelte:head>

<article>
	<p class="crumb">{data.section.tab} / {data.section.group}</p>
	<h1>{data.section.title}</h1>

	<p class="actions">
		<a class="button" href="{base}/gallery/">Open in gallery</a>
		{#if data.section.source}
			<a class="source" href="{repo}/blob/main/{data.section.source}">
				<code>{data.section.source}</code>
			</a>
		{/if}
	</p>

	{#if data.html}
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html data.html}
	{:else}
		<p class="todo">
			No prose yet. Write <code>www/docs/{data.section.key}.md</code> and it lands here.
		</p>
	{/if}
</article>

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

	.actions {
		display: flex;
		align-items: center;
		gap: 14px;
		flex-wrap: wrap;
		margin: 0 0 32px;
	}

	.button {
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 7px;
		padding: 6px 12px;
		color: var(--text);
		font-size: 14px;
	}

	.button:hover {
		border-color: var(--accent);
		text-decoration: none;
	}

	.source code {
		color: var(--muted);
	}

	.todo {
		color: var(--muted);
		border: 1px dashed var(--line);
		border-radius: 8px;
		padding: 16px;
	}
</style>
