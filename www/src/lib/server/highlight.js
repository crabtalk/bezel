import { Check, Copy } from 'lucide-static';
import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import ini from 'highlight.js/lib/languages/ini';
import json from 'highlight.js/lib/languages/json';
import rust from 'highlight.js/lib/languages/rust';

// Server-only, so highlight.js runs at build time and never ships to a reader.
// Only the languages the site uses — the core build plus four grammars, rather
// than all 190.
hljs.registerLanguage('bash', bash);
hljs.registerLanguage('json', json);
hljs.registerLanguage('rust', rust);
hljs.registerLanguage('toml', ini);

const ESCAPES = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' };
const escape = (code) => code.replace(/[&<>"]/g, (char) => ESCAPES[char]);

/** A code block as the site draws one: highlighted, with a copy button. */
export function codeBlock(text, lang) {
	const language = lang && hljs.getLanguage(lang) ? lang : null;
	const body = language ? hljs.highlight(text, { language }).value : escape(text);
	// The button carries both marks and swaps them with a class; what it copies
	// is the block's own textContent, so the source is never duplicated into an
	// attribute just to be read back.
	return (
		`<div class="code-block">` +
		`<button class="copy" type="button" aria-label="Copy code">${Copy}${Check}</button>` +
		`<pre><code class="hljs${language ? ` language-${language}` : ''}">${body}</code></pre>` +
		`</div>\n`
	);
}
