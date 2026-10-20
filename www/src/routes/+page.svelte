<script>
	import { Check, Copy } from 'lucide-static';
	import { base } from '$app/paths';
	import Gallery from '$lib/Gallery.svelte';
	import Rims from '$lib/Rims.svelte';
	import { engage } from '$lib/engage.js';
	import diagrams from '$lib/diagrams.json';
	import stats from '$lib/stats.json';
	import { catalog, repo, skillInstall, tagline as description } from '$lib/catalog.js';

	let { data } = $props();

	let expanded = $state(false);

	// Counted from the source at build time: every gallery page outside
	// Foundations, the `bezel-theme` light and dark variants and the
	// `bezel-mermaid` diagram kinds.
	const components = catalog
		.filter((tab) => tab.title !== 'Foundations')
		.flatMap((tab) => tab.groups)
		.reduce((sum, group) => sum + group.sections.length, 0);
	const figures = [
		{ value: components, label: 'Components' },
		{ value: stats.themes, label: 'Themes' },
		{ value: stats.diagrams, label: 'Diagram types' }
	];

	// Each chapter's diagram is drawn by bezel-mermaid at build time, from
	// `www/diagrams/<id>.mmd`.
	const chapters = [
		{
			id: 'theme',
			title: 'Theme',
			heading: 'Styled the way SwiftUI is',
			text: 'Components read the theme from the environment at paint time, never from parameters. A Brand of three numbers re-tints every one of them, light and dark, at the contrast the shipped palette was checked at.',
			href: 'theme'
		},
		{
			id: 'editor',
			title: 'Editor',
			heading: "Notion's editor, over a markdown file",
			text: "Notion's document model, a flat list of blocks, with its slash menu, block handles and marks. Markdown is the wire form: parse it, serialize it, and nothing moves.",
			href: 'editor'
		},
		{
			id: 'mermaid',
			title: 'Diagrams',
			heading: 'Mermaid, drawn natively',
			text: `A Rust port of Pele 1.6.0: all ${stats.diagrams} diagram types, no JavaScript, and at least as fast as Pele on every benchmark case, 1.06× to 2.7×. These diagrams are drawn by it.`,
			note: 'Apple M1, opt-level 3, Pele in Node 26. Mermaid.js at 100× slower than Pele, from Pele’s published 0.05–0.3 ms against 6–82 ms per render.',
			href: 'mermaid'
		},
		{
			id: 'agent',
			title: 'Agent',
			heading: 'The screens Cydonia runs on',
			text: "Activity, tool calls, composer, transcript and diff are what Cydonia's agent sessions are made of, each a page you can copy.",
			href: 'agent-activity'
		}
	];

	let current = $state(chapters[0].id);

	// The chapter whose middle band of the window it is in lights its entry.
	$effect(() => {
		const seen = new IntersectionObserver(
			(entries) => {
				for (const entry of entries) if (entry.isIntersecting) current = entry.target.id;
			},
			{ rootMargin: '-45% 0px -45% 0px' }
		);
		for (const chapter of chapters) {
			const element = document.getElementById(chapter.id);
			if (element) seen.observe(element);
		}
		return () => seen.disconnect();
	});

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

<!-- The logo's grain, over the whole page and behind everything on it. -->
<div class="grain" aria-hidden="true"></div>
<Rims />

<section class="hero">
	<div class="say">
		<h1>Build the app, not the buttons.</h1>

		<div class="install code-block">
			<span class="file">Agent skill</span>
			<code>{skillInstall}</code>
			<button class="copy" type="button" aria-label="Copy">
				<!-- eslint-disable-next-line svelte/no-at-html-tags -->
				{@html Copy}{@html Check}
			</button>
		</div>
	</div>

	<!-- The code an app writes, beside the line it adds: the live editor below is
	     what this makes. -->
	<div class="act">
		<span class="file">Excerpt · bezel-editor</span>
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html data.sample}
	</div>
</section>

<section class="figures" aria-label="In the library">
	{#each figures as figure (figure.label)}
		<div>
			<strong>{figure.value}</strong>
			<span>{figure.label}</span>
		</div>
	{/each}
</section>

<Gallery title="bezel" src="{base}/gallery/?open=editor" bind:open={expanded} />

<!-- Not a screenshot of the library: the library, running. The window frame is
     the claim — this is what it looks like on the desktop it targets. -->
<section class="stage">
	<div class="window" data-own-light use:engage={() => (expanded = true)}>
		<div class="titlebar">
			<span class="light close" aria-hidden="true"></span>
			<span class="light minimise" aria-hidden="true"></span>
			<span class="light zoom" aria-hidden="true"></span>
			<span class="title">Bezel — Native desktop UI for Rust, on <a href="https://www.gpui.rs">gpui</a>.</span>
		</div>
		<iframe title="The bezel gallery, running" src="{base}/gallery/?open=editor"></iframe>
		<button class="cover" aria-label="Use the gallery"></button>
	</div>

	<!-- The frame above is wasm in an iframe — better to say so here than to let
	     a visitor judge gpui by it. -->
	<p class="note">
		Embedded as wasm — the desktop build is where it is at its best.
		<code>cargo run -p gallery</code>
	</p>
</section>

<!-- The page scrolls through the chapters; the contents stay beside them. -->
<section class="tour">
	<nav class="toc" aria-label="On this page">
		{#each chapters as chapter (chapter.id)}
			<a href="#{chapter.id}" class:current={current === chapter.id}>{chapter.title}</a>
		{/each}
	</nav>
	<div class="chapters">
		{#each chapters as chapter (chapter.id)}
			<article id={chapter.id}>
				<h2>{chapter.heading}</h2>
				<p>{chapter.text}</p>
				<div class="diagram">
					<!-- eslint-disable-next-line svelte/no-at-html-tags -->
					{@html diagrams[chapter.id]}
				</div>
				{#if chapter.note}<p class="fine">{chapter.note}</p>{/if}
				<a class="more" href="{base}/docs/{chapter.href}/">Read the docs →</a>
			</article>
		{/each}
	</div>
</section>

<!-- The proof the gallery is real software: an app people use, built on it. -->
<section class="built">
	<a href="https://cydonia.sh">
		<img src="{base}/cydonia.png" alt="Cydonia, a desktop workspace for coding agents" width="1200" height="630" loading="lazy" />
	</a>
	<p>
		<a href="https://cydonia.sh">Cydonia</a> is built on bezel — a desktop workspace for coding agents.
	</p>
</section>

<style>
	.hero {
		display: grid;
		grid-template-columns: minmax(0, 1.1fr) minmax(0, 0.9fr);
		align-items: center;
		gap: 112px;
		max-width: calc(1200px + 2 * var(--gutter));
		margin: 0 auto;
		padding: 112px var(--gutter) 144px;
	}

	/* The install line below is a scroll container, but a grid item still sizes to
	   its min-content — without this the column takes the unbreakable code line's
	   width and the whole page scrolls sideways on a phone. */
	.say {
		min-width: 0;
	}

	.act {
		min-width: 0;
	}

	/* Says what the block is: part of a view, not a program. */
	.act .file {
		display: block;
		margin-bottom: 8px;
		font-family: var(--mono);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		color: var(--faint);
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
		margin: 40px 0 0;
		padding: 10px 12px;
		border: 1px solid var(--line);
		border-radius: var(--radius-panel);
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

	.figures {
		display: grid;
		grid-template-columns: repeat(3, minmax(0, 1fr));
		max-width: calc(1200px + 2 * var(--gutter));
		margin: -64px auto 0;
		padding: 0 var(--gutter) 96px;
	}

	.figures div {
		display: grid;
		gap: 6px;
		padding: 4px 0 4px 24px;
		border-left: 1px solid var(--line);
	}

	.figures strong {
		font-size: clamp(32px, 4.5vw, 52px);
		font-weight: 600;
		letter-spacing: -0.04em;
		line-height: 1;
		font-variant-numeric: tabular-nums;
	}

	.figures span {
		font-family: var(--mono);
		font-size: 11px;
		text-transform: uppercase;
		letter-spacing: 0.12em;
		color: var(--faint);
	}

	/* The noise the favicon's `grain` filter draws, tiled. */
	.grain {
		position: fixed;
		inset: 0;
		z-index: -1;
		pointer-events: none;
		opacity: 0.07;
		background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='240' height='240'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='.85' numOctaves='3' seed='12' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
	}

	.stage {
		position: relative;
		isolation: isolate;
		max-width: calc(1200px + 2 * var(--gutter));
		margin: 0 auto;
		padding: 0 var(--gutter) 96px;
	}

	/* Light on the window, in the logo's silver: brightest at its top edge and
	   gone well before the page's sides. */
	.stage::before {
		content: '';
		position: absolute;
		z-index: -1;
		inset: -160px 0 30%;
		pointer-events: none;
		background:
			radial-gradient(50% 45% at 50% 30%, rgb(233 231 232 / 0.13), transparent 70%),
			radial-gradient(70% 60% at 50% 40%, rgb(160 161 163 / 0.07), transparent 75%);
	}

	.window {
		position: relative;
		border: 1px solid var(--line);
		border-radius: var(--radius-frame);
		overflow: hidden;
		background: var(--panel);
		box-shadow: 0 40px 90px -30px rgb(0 0 0 / 0.9);
	}

	/* Above the cover, so the title's link stays pressable. */
	.titlebar {
		position: relative;
		z-index: 2;
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

	.title a {
		color: var(--muted);
	}

	iframe {
		display: block;
		width: 100%;
		height: min(74vh, 760px);
		border: 0;
		background: var(--bg);
	}

	.note {
		margin: 28px 0 0;
		color: var(--faint);
		font-size: 13px;
		text-align: center;
	}

	.tour {
		display: grid;
		grid-template-columns: 200px minmax(0, 1fr);
		gap: 64px;
		max-width: calc(1200px + 2 * var(--gutter));
		margin: 0 auto;
		padding: 0 var(--gutter) 96px;
	}

	/* Glass: the rims and the page scroll under it, blurred and lifted. */
	.toc {
		position: sticky;
		top: calc(var(--header) + 48px);
		align-self: start;
		display: grid;
		gap: 4px;
		padding: 12px 14px;
		border-radius: var(--radius-frame);
		background: color-mix(in srgb, var(--panel) 45%, transparent);
		backdrop-filter: blur(20px) saturate(1.8);
		-webkit-backdrop-filter: blur(20px) saturate(1.8);
		box-shadow:
			inset 0 1px 0 rgb(255 255 255 / 0.1),
			inset 0 0 0 1px rgb(255 255 255 / 0.06);
	}

	.toc a {
		padding: 6px 0 6px 14px;
		border-left: 1px solid var(--line);
		color: var(--faint);
		font-size: 14px;
	}

	.toc a:hover {
		color: var(--text);
		text-decoration: none;
	}

	.toc a.current {
		border-left-color: var(--text);
		color: var(--text);
	}

	.chapters {
		display: grid;
		gap: 160px;
	}

	.chapters article {
		scroll-margin-top: calc(var(--header) + 48px);
	}

	.chapters h2 {
		margin: 0;
		font-size: clamp(26px, 3vw, 36px);
		font-weight: 600;
		letter-spacing: -0.03em;
		line-height: 1.15;
	}

	.chapters article > p {
		max-width: 60ch;
		margin: 14px 0 0;
		color: var(--muted);
		font-size: 16px;
	}

	.chapters .fine {
		margin-top: 10px;
		color: var(--faint);
		font-size: 13px;
	}

	/* The diagrams' own variables, set to the page's palette. */
	.diagram {
		--bezel-bg: var(--bg);
		--bezel-fg: var(--text);
		--bezel-muted: var(--muted);
		--bezel-line: var(--muted);
		--bezel-surface: var(--panel);
		--bezel-surface-alt: #151515;
		--bezel-border: var(--line-strong);
		--bezel-accent: var(--text);
		--bezel-critical: #f87171;
		--bezel-series-1: #d4d4d4;
		--bezel-series-2: #8a8a8a;
		--bezel-font: -apple-system, BlinkMacSystemFont, 'Inter', system-ui, sans-serif;
		--bezel-font-mono: var(--mono);
		margin: 32px 0 0;
	}

	.diagram :global(svg) {
		display: block;
		max-width: 100%;
		height: auto;
	}

	.more {
		display: inline-block;
		margin-top: 20px;
		color: var(--muted);
		font-size: 14px;
	}

	.more:hover {
		color: var(--text);
	}

	.built {
		max-width: calc(1200px + 2 * var(--gutter));
		margin: 0 auto;
		padding: 0 var(--gutter) 96px;
		text-align: center;
	}

	.built img {
		display: block;
		width: 100%;
		max-width: 900px;
		height: auto;
		margin: 0 auto;
		border: 1px solid var(--line);
		border-radius: var(--radius-frame);
	}

	.built p {
		margin: 20px 0 0;
		color: var(--muted);
	}

	@media (max-width: 900px) {
		.hero {
			grid-template-columns: 1fr;
			gap: 48px;
			padding: 72px var(--gutter) 88px;
		}
	}

	@media (max-width: 780px) {
		.tour {
			grid-template-columns: 1fr;
		}

		.toc {
			display: none;
		}

		.chapters {
			gap: 96px;
		}

		.figures {
			margin-top: -32px;
		}

		.figures div {
			padding-left: 14px;
		}

		.figures,
		.stage,
		.tour,
		.built {
			padding-bottom: 64px;
		}
	}
</style>
