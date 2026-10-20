<script>
	// Fixed behind the home page: nested rounded rims (static/rims.svg), a bezel
	// inside a bezel, invisible except inside a circle of light that follows the
	// pointer. Touch input has no pointer to follow, so nothing shows, and over an
	// element marked data-own-light (the live gallery) it goes out.
	import { base } from '$app/paths';
	import { follow } from '$lib/follow.js';

	let node = $state();

	$effect(() => {
		const light = follow(([x, y]) => {
			node.style.setProperty('--x', `${x}px`);
			node.style.setProperty('--y', `${y}px`);
		});
		const move = (event) => {
			if (event.pointerType === 'touch') return;
			if (event.target.closest?.('[data-own-light]')) leave();
			else light.aim([event.clientX, event.clientY]);
		};
		const leave = () => {
			light.clear();
			node.style.removeProperty('--x');
			node.style.removeProperty('--y');
		};
		addEventListener('pointermove', move);
		document.documentElement.addEventListener('pointerleave', leave);
		return () => {
			light.stop();
			removeEventListener('pointermove', move);
			document.documentElement.removeEventListener('pointerleave', leave);
		};
	});
</script>

<div class="rims" style:--map="url('{base}/rims.svg')" bind:this={node} aria-hidden="true"></div>

<style>
	.rims {
		--light: radial-gradient(circle 260px at var(--x, -999px) var(--y, -999px), #000 0%, rgb(0 0 0 / 0.45) 45%, transparent 100%);
		position: fixed;
		inset: 0;
		z-index: -1;
		pointer-events: none;
		background: rgb(233 231 232 / 0.22);
		-webkit-mask:
			var(--map) center / cover no-repeat,
			var(--light);
		-webkit-mask-composite: source-in;
		mask:
			var(--map) center / cover no-repeat,
			var(--light);
		mask-composite: intersect;
	}
</style>
