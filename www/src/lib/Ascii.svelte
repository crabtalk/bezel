<script>
	// A hexdump for the hero to stand on. Seeded rather than random: the page is
	// prerendered, so an unseeded roll would print one field into the HTML and a
	// different one on hydration, and the block would visibly change on load.
	//
	// FNV-1a → mulberry32, the same pair `../web` uses, so a seed means the same
	// thing on both sides. Purely decorative: behind content, no pointer events,
	// out of the accessibility tree.
	let { rows = 18, cols = 26, seed = 'bezel' } = $props();

	function seedOf(value) {
		let hash = 2166136261;
		for (let i = 0; i < value.length; i++) {
			hash ^= value.charCodeAt(i);
			hash = Math.imul(hash, 16777619);
		}
		return hash >>> 0;
	}

	function prng(start) {
		let a = start;
		return () => {
			a |= 0;
			a = (a + 0x6d2b79f5) | 0;
			let t = Math.imul(a ^ (a >>> 15), 1 | a);
			t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
			return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
		};
	}

	const field = (() => {
		const next = prng(seedOf(seed));
		return Array.from({ length: rows }, () =>
			Array.from({ length: cols }, () =>
				Math.floor(next() * 256)
					.toString(16)
					.padStart(2, '0')
			).join(' ')
		);
	})();
</script>

<pre class="ascii" aria-hidden="true">{field.join('\n')}</pre>

<style>
	.ascii {
		position: absolute;
		inset: 0;
		margin: 0;
		padding: 0;
		border: 0;
		background: none;
		overflow: hidden;
		pointer-events: none;
		user-select: none;
		font-family: var(--mono);
		font-size: 12px;
		line-height: 1.6;
		color: var(--line-strong);
		/* Fades into the page instead of ending on a hard rectangle. */
		-webkit-mask-image: radial-gradient(120% 90% at 70% 40%, #000 0%, transparent 72%);
		mask-image: radial-gradient(120% 90% at 70% 40%, #000 0%, transparent 72%);
	}
</style>
