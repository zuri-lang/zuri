#!-- part of the html module

/**
 * Tags which contain arbitrary non-parsed content
 * For example: `<script>` JavaScript should not be parsed
 * 
 * @readonly
 * @type list
 */
var childless_tags = ['style', 'script', 'template']

/**
 * Tags which auto-close because they cannot be nested
 * For example: `<p>Outer<p>Inner is <p>Outer</p><p>Inner</p>`
 * 
 * @readonly
 * @type list
 */
var closing_tags = [
  'html', 'head', 'body', 'dt', 'dd', 'li', 'option',
  'thead', 'th', 'tbody', 'tr', 'td', 'tfoot', 'colgroup'
]

/**
 * Closing tags which have ancestor tags which may exist within 
 * them which prevent the closing tag from auto-closing.
 * For example: in `<li><ul><li></ul></li>`, the top-level `<li>` 
 * should not auto-close.
 * 
 * @readonly
 * @type list
 */
var tag_ancestors = {
  li: ['ul', 'ol', 'menu'],
  dt: ['dl'],
  dd: ['dl'],
  tbody: ['table'],
  thead: ['table'],
  tfoot: ['table'],
  tr: ['table'],
  td: ['table']
}

/**
 * Tags which do not need the closing tag
 * For example: `<img>` does not need `</img>`
 * 
 * @readonly
 * @type list
 */
var void_tags = [
  '!doctype', 'area', 'base', 'br', 'col', 'command',
  'embed', 'hr', 'img', 'input', 'keygen', 'link',
  'meta', 'param', 'source', 'track', 'wbr'
]
