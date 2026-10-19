// An action for a box holding a gallery iframe and its `.cover`. With a mouse,
// pressing the cover lifts it and focuses the iframe, and a press anywhere else
// on the page puts it back. On touch, pressing it calls `expand`.
export function engage(node, expand) {
	const touch = matchMedia('(hover: none)');
	const cover = node.querySelector('.cover');

	const press = () => {
		if (touch.matches) return expand();
		node.classList.add('engaged');
		node.querySelector('iframe')?.focus();
	};
	// A press inside the iframe goes to its own document, so any press this
	// document sees outside the box is a press away from it.
	const away = (event) => {
		if (!node.contains(event.target)) node.classList.remove('engaged');
	};

	cover.addEventListener('click', press);
	document.addEventListener('pointerdown', away);
	return {
		update(next) {
			expand = next;
		},
		destroy() {
			cover.removeEventListener('click', press);
			document.removeEventListener('pointerdown', away);
		}
	};
}
