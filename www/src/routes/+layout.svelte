<script>
	import '../app.css';
	import { base } from '$app/paths';
	import { repo } from '$lib/catalog.js';

	let { children } = $props();
</script>

<header>
	<a class="wordmark" href="{base}/">bezel</a>
	<nav>
		<a href="{base}/docs/color/">Docs</a>
		<a href="{base}/gallery/" data-sveltekit-reload>Gallery</a>
		<a href={repo}>GitHub</a>
	</nav>
</header>

{@render children()}

<style>
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 24px;
		padding: 0 24px;
		height: 56px;
		border-bottom: 1px solid var(--line);
		position: sticky;
		top: 0;
		background: color-mix(in srgb, var(--bg) 88%, transparent);
		backdrop-filter: blur(12px);
		z-index: 10;
	}

	.wordmark {
		color: var(--text);
		font-weight: 600;
		letter-spacing: -0.02em;
	}

	nav {
		display: flex;
		gap: 20px;
	}

	nav a {
		color: var(--muted);
	}

	nav a:hover {
		color: var(--text);
		text-decoration: none;
	}
</style>
