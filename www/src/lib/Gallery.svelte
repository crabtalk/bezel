<script>
	// A dialog, not a link: navigating to the gallery left the reader in a static
	// page with no way back to the one they came from.
	let { title, src, open = $bindable(false) } = $props();

	let dialog = $state(null);

	// The iframe is a wasm instance of its own, so it is not created until asked
	// for — and it is torn down on close rather than left running behind the page.
	$effect(() => {
		if (open) dialog?.showModal();
		else dialog?.close();
	});
</script>

<dialog bind:this={dialog} onclose={() => (open = false)}>
	<div class="frame">
		<div class="bar">
			<span class="what">{title}</span>
			<button class="close" onclick={() => (open = false)} aria-label="Close">Esc</button>
		</div>
		{#if open}
			<iframe title="{title}, running" {src}></iframe>
		{/if}
	</div>
</dialog>

<style>
	dialog {
		width: min(1240px, 94vw);
		max-width: none;
		height: min(88vh, 900px);
		max-height: none;
		padding: 0;
		border: 1px solid var(--line-strong);
		border-radius: 12px;
		background: var(--bg);
		color: var(--text);
		overflow: hidden;
	}

	dialog::backdrop {
		background: rgb(0 0 0 / 0.72);
		backdrop-filter: blur(3px);
	}

	.frame {
		display: flex;
		flex-direction: column;
		height: 100%;
	}

	.bar {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: 0 14px;
		height: 40px;
		border-bottom: 1px solid var(--line);
		flex: none;
	}

	.what {
		color: var(--muted);
		font-size: 13px;
	}

	.close {
		font: inherit;
		font-size: 12px;
		cursor: pointer;
		color: var(--muted);
		background: none;
		border: 1px solid var(--line-strong);
		border-radius: 5px;
		padding: 3px 8px;
	}

	.close:hover {
		color: var(--text);
		border-color: var(--text);
	}

	iframe {
		flex: 1;
		display: block;
		width: 100%;
		border: 0;
		background: var(--bg);
	}
</style>
