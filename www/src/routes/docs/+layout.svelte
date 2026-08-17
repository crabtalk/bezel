<script>
	import { base } from '$app/paths';
	import { page } from '$app/state';
	import { documented } from '$lib/catalog.js';

	let { children } = $props();

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
		<div class="reading">{@render children()}</div>
	</main>
</div>

<style>
	/* Full bleed on purpose: the rail belongs to the window, not to the column
	   it sits beside. Only the prose gets a reading width, and it centres in
	   whatever space is left rather than dragging the rail inward with it. */
	.shell {
		display: flex;
		align-items: flex-start;
		min-height: calc(100vh - 56px);
	}

	aside {
		position: sticky;
		top: 56px;
		flex: 0 0 272px;
		height: calc(100vh - 56px);
		overflow-y: auto;
		scrollbar-width: thin;
		scrollbar-color: transparent transparent;
		border-right: 1px solid var(--line);
		padding: 24px 20px 48px;
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
		border-radius: 6px;
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

	.reading {
		max-width: 860px;
		margin: 0 auto;
		padding: 32px 40px 96px;
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
