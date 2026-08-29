<script>
	import { TriangleAlert } from 'lucide-static';

	let dismissed = $state(true);

	$effect(() => {
		dismissed = localStorage.getItem('mobile-notice-dismissed') === '1';
	});

	function dismiss() {
		dismissed = true;
		localStorage.setItem('mobile-notice-dismissed', '1');
	}
</script>

{#if !dismissed}
	<div class="notice" role="status">
		<!-- eslint-disable-next-line svelte/no-at-html-tags -->
		{@html TriangleAlert}
		<p>
			<strong>Known issue</strong> — on mobile, the embedded gallery is WebAssembly built on
			<code>gpui_web</code>, which isn't touch-interactive yet. Full mobile support ships in the
			next release.
		</p>
		<button onclick={dismiss} aria-label="Dismiss">&times;</button>
	</div>
{/if}

<style>
	.notice {
		display: none;
	}

	/* The one hue on the page: everywhere else is neutral by construction, but
	   a warning that carries no color reads as just another note. */
	@media (hover: none) {
		.notice {
			--warn: #eab308;
			display: flex;
			align-items: flex-start;
			gap: 10px;
			padding: 12px 16px;
			border-top: 3px solid var(--warn);
			border-bottom: 1px solid var(--line);
			background: color-mix(in srgb, var(--warn) 10%, var(--panel));
		}
	}

	.notice :global(svg) {
		flex: none;
		width: 17px;
		height: 17px;
		margin-top: 1px;
		color: var(--warn);
	}

	.notice p {
		flex: 1;
		min-width: 0;
		margin: 0;
		color: var(--muted);
		font-size: 13px;
		line-height: 1.45;
	}

	.notice p strong {
		color: var(--warn);
	}

	.notice p code {
		font-family: var(--mono);
		font-size: 12px;
		color: var(--text);
	}

	.notice button {
		flex: none;
		background: none;
		border: 0;
		padding: 0 4px;
		color: var(--muted);
		font-size: 18px;
		line-height: 1;
		cursor: pointer;
	}

	.notice button:hover {
		color: var(--text);
	}
</style>
