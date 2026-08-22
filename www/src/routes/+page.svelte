<script>
	import { siGithub, siX } from 'simple-icons';
	import { Check, Copy } from 'lucide-static';
	import { base } from '$app/paths';
	import Ascii from '$lib/Ascii.svelte';
	import Brand from '$lib/Brand.svelte';
	import Gallery from '$lib/Gallery.svelte';
	import { docsHome, repo } from '$lib/catalog.js';

	let expanded = $state(false);

	const author = 'https://x.com/tianyi_gc';

	// Search only — the page itself does not repeat it.
	const description = 'Design tokens, motion, and materials for native Rust apps, built on gpui.';

	// What the library is, in its own terms — README and ARCHITECTURE already
	// argue these. Reasons to use it, not a list of what is in the box.
	const pillars = [
		{
			title: 'Style flows through the environment',
			body: 'One flat Theme installed as a gpui global and read at paint time — SwiftUI’s @Environment, not a parameter threaded through every call site.'
		},
		{
			title: 'Layers you can take alone',
			body: 'theme is useful to anyone writing their own gpui components, motion is the animation vocabulary, ui is the views. Each depends only downward.'
		},
		{
			title: 'Extracted, never invented ahead of need',
			body: 'Every component landed here the day it landed in a real application. The gallery composes each one exactly once, so the documentation cannot drift from the library.'
		},
		{
			title: 'Designed light and dark',
			body: 'WCAG-verified contrast pairing and oklch colour math, with system appearance switching — both palettes designed, neither derived from the other.'
		}
	];

	const jsonLd = {
		'@context': 'https://schema.org',
		'@type': 'SoftwareSourceCode',
		name: 'bezel',
		description,
		codeRepository: repo,
		programmingLanguage: 'Rust',
		license: 'https://opensource.org/licenses/MIT',
		keywords: [
			'gpui',
			'gpui components',
			'gpui component library',
			'Rust UI components',
			'Rust desktop UI',
			'native Rust GUI',
			'Zed gpui',
			'AI agent UI',
			'design tokens'
		]
	};
	const jsonLdHtml = `<script type="application/ld+json">${JSON.stringify(jsonLd)}<\/script>`;
</script>

<svelte:head>
	<title>bezel — a component library for gpui</title>
	<meta name="description" content={description} />
	<meta property="og:title" content="bezel — a component library for gpui" />
	<meta property="og:description" content={description} />
	{@html jsonLdHtml}
</svelte:head>

<section class="hero">
	<div class="say">
		<h1>Build the app, not the buttons.</h1>

		<!-- One dependency, and no `[patch.crates-io]` to repeat: `bezel` re-exports
		     the gpui it was built against, so a consumer cannot end up with a second
		     copy in the graph. -->
		<div class="install code-block">
			<span class="file">Cargo.toml</span>
			<code>bezel = &#123; git = "https://github.com/crabtalk/bezel" &#125;</code>
			<button class="copy" type="button" aria-label="Copy">
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				{@html Copy}{@html Check}
			</button>
		</div>

		<p class="facts">
			<span><strong>MIT</strong> licensed</span>
			<span>Built on <strong>gpui</strong></span>
		</p>
	</div>

	<div class="act">
		<Ascii />
		<div class="cta">
			{#if docsHome}
				<a class="button primary" href="{base}/docs/{docsHome}/">Read the docs</a>
			{/if}
			<button class="button" onclick={() => (expanded = true)}>Open the gallery</button>
		</div>
	</div>
</section>

<Gallery title="bezel" src="{base}/gallery/" bind:open={expanded} />

<!-- Not a screenshot of the library: the library, running. The window frame is
     the claim — this is what it looks like on the desktop it targets. -->
<section class="stage">
	<div class="window">
		<div class="titlebar">
			<span class="light close" aria-hidden="true"></span>
			<span class="light minimise" aria-hidden="true"></span>
			<span class="light zoom" aria-hidden="true"></span>
			<span class="title">bezel</span>
		</div>
		<iframe title="The bezel gallery, running" src="{base}/gallery/"></iframe>
	</div>
	<!-- The frame above is the claim, and it is wasm in an iframe — better to say
	     so here than to let a visitor judge gpui by it. -->
	<p class="note">
		Embedded as wasm — the desktop build is where it is at its best.
		<code>cargo run -p gallery</code>
	</p>
</section>

<section class="pillars">
	<div class="grid">
		{#each pillars as pillar (pillar.title)}
			<article>
				<h2>{pillar.title}</h2>
				<p>{pillar.body}</p>
			</article>
		{/each}
	</div>
</section>

<footer>
	<nav class="left">
		<a href="https://github.com/crabtalk">crabtalk</a>
		{#if docsHome}<a href="{base}/docs/{docsHome}/">Docs</a>{/if}
	</nav>
	<nav class="right">
		<a href={repo} aria-label="bezel on GitHub"><Brand icon={siGithub} size={15} /></a>
		<a href={author} target="_blank" rel="noreferrer" aria-label="The author on X">
			<Brand icon={siX} size={14} />
		</a>
	</nav>
</footer>

<style>
	.hero {
		display: grid;
		grid-template-columns: minmax(0, 1fr) minmax(0, 0.72fr);
		align-items: center;
		gap: 40px;
		max-width: 1180px;
		margin: 0 auto;
		padding: 112px 40px 56px;
	}

	.act {
		position: relative;
		display: flex;
		align-items: center;
		justify-content: flex-end;
		min-height: 220px;
	}

	.cta {
		position: relative;
		display: flex;
		gap: 10px;
		flex-wrap: wrap;
		justify-content: flex-end;
	}

	.button {
		display: inline-flex;
		align-items: center;
		border: 1px solid var(--line-strong);
		border-radius: 6px;
		padding: 0 16px;
		height: 40px;
		color: var(--text);
		font-family: inherit;
		font-size: 14px;
		font-weight: 500;
		background: var(--bg);
		cursor: pointer;
	}

	.button:hover {
		border-color: var(--text);
		text-decoration: none;
	}

	.button.primary {
		background: var(--text);
		border-color: var(--text);
		color: #000;
	}

	.button.primary:hover {
		background: #fff;
		border-color: #fff;
	}

	h1 {
		font-size: clamp(40px, 6vw, 68px);
		line-height: 1.05;
		margin: 0;
		letter-spacing: -0.045em;
		font-weight: 600;
		max-width: 14ch;
	}

	.install {
		display: flex;
		align-items: center;
		gap: 14px;
		max-width: 580px;
		margin: 32px 0 0;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: 8px;
		background: var(--panel);
		overflow-x: auto;
	}

	.install .file {
		flex: none;
		font-family: var(--mono);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		color: var(--faint);
		padding-right: 12px;
		border-right: 1px solid var(--line-strong);
	}

	.install code {
		font-size: 13px;
		white-space: nowrap;
		background: none;
		border: 0;
		padding: 0;
		color: var(--text);
	}

	/* `.copy` is authored for a code block, where it pins to the top-right of a
	   tall `pre`. This row is one line, so it centres instead. */
	.install .copy {
		top: 50%;
		right: 8px;
		transform: translateY(-50%);
	}

	.stage {
		max-width: 1180px;
		margin: 0 auto;
		padding: 0 40px 24px;
	}

	.window {
		border: 1px solid var(--line);
		border-radius: 12px;
		overflow: hidden;
		background: var(--panel);
		box-shadow: 0 40px 90px -30px rgb(0 0 0 / 0.9);
	}

	.titlebar {
		display: flex;
		align-items: center;
		gap: 8px;
		height: 36px;
		padding: 0 14px;
		border-bottom: 1px solid var(--line);
		background: var(--panel);
	}

	.light {
		width: 12px;
		height: 12px;
		border-radius: 50%;
	}

	.close {
		background: #ff5f57;
	}

	.minimise {
		background: #febc2e;
	}

	.zoom {
		background: #28c840;
	}

	.title {
		margin-left: auto;
		margin-right: auto;
		padding-right: 48px;
		color: var(--faint);
		font-size: 13px;
	}

	iframe {
		display: block;
		width: 100%;
		height: min(74vh, 760px);
		border: 0;
		background: var(--bg);
	}

	.note {
		margin: 20px 0 0;
		color: var(--faint);
		font-size: 13px;
		text-align: center;
	}

	.facts {
		display: flex;
		gap: 28px;
		flex-wrap: wrap;
		margin: 14px 0 0;
		color: var(--faint);
		font-size: 14px;
	}

	.facts strong {
		color: var(--muted);
		font-weight: 500;
	}

	/* Spacing lives out here, not on the grid: the grid's background is the rule
	   colour showing through its own gaps, so padding on it would paint a thick
	   border instead of leaving room around the box. */
	.pillars {
		max-width: 1180px;
		margin: 0 auto;
		padding: 80px 40px 128px;
	}

	.grid {
		display: grid;
		grid-template-columns: repeat(2, minmax(0, 1fr));
		gap: 1px;
		background: var(--line);
		border: 1px solid var(--line);
		border-radius: 12px;
		overflow: hidden;
	}

	/* The rule between panels is the container showing through a 1px gap, so
	   there is one line between neighbours rather than two borders meeting. */
	.pillars article {
		background: var(--bg);
		padding: 36px 32px;
	}

	.pillars h2 {
		font-size: 16px;
		font-weight: 500;
		margin: 0 0 10px;
		letter-spacing: -0.01em;
	}

	.pillars p {
		margin: 0;
		color: var(--muted);
		font-size: 15px;
		line-height: 1.65;
	}

	footer {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 20px;
		max-width: 1180px;
		margin: 0 auto;
		padding: 28px 40px 72px;
		border-top: 1px solid var(--line);
	}

	footer nav {
		display: flex;
		align-items: center;
		gap: 22px;
	}

	/* Drafting-label register: small, spaced and set in the mono face, so it
	   reads as an annotation on the page rather than more prose. */
	.left a {
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
	}

	@media (max-width: 900px) {
		.hero {
			grid-template-columns: 1fr;
			gap: 24px;
			padding: 72px 24px 40px;
		}

		.act {
			min-height: 0;
			justify-content: flex-start;
		}

		.cta {
			justify-content: flex-start;
		}
	}

	@media (max-width: 780px) {

		.stage {
			padding: 0 24px 16px;
		}

		.pillars {
			padding: 56px 24px 88px;
		}

		.grid {
			grid-template-columns: 1fr;
		}

		footer {
			flex-direction: column;
			align-items: flex-start;
			padding: 24px 24px 56px;
		}
	}
</style>
