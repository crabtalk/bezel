// Renames the built gallery's wasm-bindgen output to content-hashed names and
// rewrites the two references to them: the wasm URL in web.js and the web.js
// import in index.html. Runs on `build/`, after `vite build`; `static/` and the
// dev server keep the plain names.
//
// Brittle to wasm-bindgen's template and to index.html changing — that's why
// this fails loudly instead of silently no-op-ing.
import { createHash } from 'node:crypto';
import { readFileSync, renameSync, writeFileSync } from 'node:fs';

const dir = 'build/gallery';

function hashed(name) {
	const bytes = readFileSync(`${dir}/${name}`);
	const hash = createHash('sha256').update(bytes).digest('hex').slice(0, 16);
	const dot = name.lastIndexOf('.');
	const renamed = `${name.slice(0, dot)}.${hash}${name.slice(dot)}`;
	renameSync(`${dir}/${name}`, `${dir}/${renamed}`);
	return renamed;
}

function rewrite(file, target, replacement) {
	const path = `${dir}/${file}`;
	const source = readFileSync(path, 'utf8');
	if (!source.includes(target)) {
		throw new Error(`hash-gallery: expected ${target} in ${path} — update the script`);
	}
	writeFileSync(path, source.replace(target, replacement));
}

const wasm = hashed('web_bg.wasm');
rewrite('web.js', "new URL('web_bg.wasm', import.meta.url)", `new URL('${wasm}', import.meta.url)`);
const js = hashed('web.js');
rewrite('index.html', "from './web.js'", `from './${js}'`);
