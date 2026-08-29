// wasm-bindgen's generated loader only falls back off `instantiateStreaming`
// when the Content-Type header is wrong. WebKit — every iOS browser, Apple
// requires it — can fail streaming compilation on a compressed response
// (Cloudflare Brotli-compresses the wasm) for other reasons, and that
// failure is re-thrown instead of falling back. Skip streaming entirely on
// iOS rather than rely on catching a failure mode that can also just hang.
//
// Brittle to wasm-bindgen's template changing between versions — that's why
// this fails loudly instead of silently no-op-ing.
import { readFileSync, writeFileSync } from 'node:fs';

const path = 'www/static/gallery/web.js';
const target = "if (typeof WebAssembly.instantiateStreaming === 'function') {";
const isIOS =
	"(/iP(hone|od|ad)/.test(navigator.userAgent) || (navigator.platform === 'MacIntel' && navigator.maxTouchPoints > 1))";
const replacement = `if (!${isIOS} && typeof WebAssembly.instantiateStreaming === 'function') {`;

const source = readFileSync(path, 'utf8');
if (!source.includes(target)) {
	throw new Error(
		`patch-ios-wasm-loader: expected pattern not found in ${path} — wasm-bindgen's template changed, update the patch`
	);
}
writeFileSync(path, source.replace(target, replacement));
