; A script body is TSX, which is the grammar JavaScript rides here.
(script_element
  (raw_text) @injection.content
  (#set! injection.language "tsx"))

(style_element
  (raw_text) @injection.content
  (#set! injection.language "css"))
