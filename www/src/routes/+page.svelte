<script>
	import { base } from '$app/paths';
	import { sections, repo } from '$lib/catalog.js';
</script>

<svelte:head>
	<title>bezel — a component library for gpui</title>
</svelte:head>

<section class="hero">
	<h1>The bezel holds the glass.</h1>
	<p>
		A component library for <a href="https://github.com/zed-industries/zed">gpui</a> —
		{sections.length} components, tokens and patterns, each composed exactly once and rendered
		here by the same code that runs natively.
	</p>
	<p class="cta">
		<a class="button primary" href="{base}/gallery/" data-sveltekit-reload>Open the gallery</a>
		<a class="button" href="{base}/docs/color/">Read the docs</a>
		<a class="button" href={repo}>GitHub</a>
	</p>
</section>

<style>
	.hero {
		max-width: 720px;
		margin: 0 auto;
		padding: 120px 24px;
		text-align: center;
	}

	h1 {
		font-size: 46px;
		margin: 0 0 20px;
		letter-spacing: -0.03em;
	}

	.hero p {
		color: var(--muted);
		font-size: 17px;
	}

	.cta {
		display: flex;
		gap: 12px;
		justify-content: center;
		margin-top: 32px;
		flex-wrap: wrap;
	}

	.button {
		background: var(--panel);
		border: 1px solid var(--line);
		border-radius: 8px;
		padding: 9px 16px;
		color: var(--text);
		font-size: 15px;
	}

	.button:hover {
		border-color: var(--accent);
		text-decoration: none;
	}

	.button.primary {
		background: var(--accent);
		border-color: var(--accent);
		color: #0b0c0e;
		font-weight: 500;
	}
</style>
