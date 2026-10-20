import { codeBlock } from '$lib/server/highlight.js';

const SAMPLE = `// in the view's constructor
let editor = cx.new(|cx| Editor::new("# Notes", cx));

// in render()
div().child(self.editor.clone())

// on save: the markdown, as the document stands
let markdown = self.editor.read(cx).source();`;

export function load() {
	return { sample: codeBlock(SAMPLE, 'rust') };
}
