// A point that eases toward its target over requestAnimationFrame, calling `draw`
// with the eased position each frame; with reduced motion it jumps there.
export function follow(draw, seconds = 0.18) {
	const still = matchMedia('(prefers-reduced-motion: reduce)');
	let at = null;
	let to = null;
	let frame = 0;
	let last = 0;

	const tick = (now) => {
		const dt = last ? Math.min((now - last) / 1000, 1 / 30) : 1 / 60;
		last = now;
		const k = 1 - Math.exp(-dt / seconds);
		at = [at[0] + (to[0] - at[0]) * k, at[1] + (to[1] - at[1]) * k];
		draw(at);
		frame = Math.hypot(to[0] - at[0], to[1] - at[1]) > 0.05 ? requestAnimationFrame(tick) : 0;
	};

	return {
		/** Ease toward a point, or place it at once when there is no position yet. */
		aim(point) {
			to = point;
			if (!at || still.matches) {
				at = [...point];
				draw(at);
				return;
			}
			if (!frame) {
				last = 0;
				frame = requestAnimationFrame(tick);
			}
		},
		/** Forget the position, as when the pointer leaves. */
		clear() {
			cancelAnimationFrame(frame);
			frame = 0;
			at = null;
			to = null;
		},
		stop() {
			cancelAnimationFrame(frame);
		}
	};
}
