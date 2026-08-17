<script>
	import { base } from '$app/paths';
	import { page } from '$app/state';
	import { catalog } from '$lib/catalog.js';

	let { children } = $props();
</script>

<div class="shell">
	<aside>
		{#each catalog as tab (tab.title)}
			<section>
				<h2>{tab.title}</h2>
				{#each tab.groups as group (group.title)}
					<h3>{group.title}</h3>
					<ul>
						{#each group.sections as item (item.key)}
							<li>
								<a
									href="{base}/docs/{item.key}/"
									class:current={page.params.slug === item.key}
								>
									{item.title}
								</a>
							</li>
						{/each}
					</ul>
				{/each}
			</section>
		{/each}
	</aside>
	<main>{@render children()}</main>
</div>

<style>
	.shell {
		display: grid;
		grid-template-columns: 250px minmax(0, 1fr);
		gap: 48px;
		max-width: 1180px;
		margin: 0 auto;
		padding: 0 24px;
	}

	aside {
		position: sticky;
		top: 56px;
		align-self: start;
		max-height: calc(100vh - 56px);
		overflow-y: auto;
		padding: 28px 0 48px;
	}

	aside h2 {
		font-size: 12px;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		color: var(--muted);
		margin: 24px 0 8px;
	}

	aside h3 {
		font-size: 13px;
		font-weight: 500;
		color: var(--text);
		margin: 16px 0 6px;
	}

	ul {
		list-style: none;
		margin: 0;
		padding: 0;
		border-left: 1px solid var(--line);
	}

	li a {
		display: block;
		padding: 3px 0 3px 12px;
		margin-left: -1px;
		border-left: 1px solid transparent;
		color: var(--muted);
		font-size: 14px;
	}

	li a:hover {
		color: var(--text);
		text-decoration: none;
	}

	li a.current {
		color: var(--accent);
		border-left-color: var(--accent);
	}

	main {
		padding: 28px 0 96px;
		min-width: 0;
	}

	@media (max-width: 820px) {
		.shell {
			grid-template-columns: 1fr;
			gap: 0;
		}

		aside {
			position: static;
			max-height: none;
		}
	}
</style>
