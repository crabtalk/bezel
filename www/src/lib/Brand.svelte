<script>
	// Official marks from `simple-icons`, not traced by hand: a brand logo drawn
	// from memory is wrong in a way readers notice and owners object to. The
	// paths inline at prerender, so this costs no request and no runtime.
	let { icon, size = 16 } = $props();
</script>

<svg
	role="img"
	viewBox="0 0 24 24"
	width={size}
	height={size}
	fill="currentColor"
	aria-hidden="true"
>
	<path d={icon.path} />
</svg>

<style>
	svg {
		display: block;
		flex: none;
	}
</style>
