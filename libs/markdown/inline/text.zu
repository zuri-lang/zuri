# Skip text characters for text token, place those to pending buffer
# and increment current pos


# Rule to skip pure text
# '{}$%@~+=:' reserved for extensions

# !, ", #, $, %, &, ', (, ), *, +, ,, -, ., /, :, ;, <, =, >, ?, @, [, \, ], ^, _, `, {, |, }, or ~

# !!!! Don't confuse with "Markdown ASCII Punctuation" chars
# http://spec.commonmark.org/0.15/#ascii-punctuation-character
/* def _is_terminator_char(ch) {
  if is_string(ch) ch = ord(ch)
  using ch {
    when  0x0A, # \n
          0x21, # !
          0x23, # #
          0x24, # $
          0x25, # %
          0x26, # &
          0x2A, # *
          0x2B, # +
          0x2D, # -
          0x3A, # :
          0x3C, # <
          0x3D, # =
          0x3E, # >
          0x40, # @
          0x5B, # [
          0x5C, # \
          0x5D, # ]
          0x5E, # ^
          0x5F, # _
          0x60, # `
          0x7B, # {
          0x7D, # }
          0x7E # ~
      return true
    default return false
  }
} */

var _terminator_chars = [
  "\n",
  "!",
  "#",
  "$",
  "%",
  "&",
  "*",
  "+",
  "-",
  ":",
  "<",
  "=",
  ">",
  "@",
  "[",
  "\\",
  "]",
  "^",
  "_",
  "`",
  "{",
  "}",
  "~"
]

def text(state, silent) {
  var pos = state.pos
  while pos < state.pos_max and !_terminator_chars.contains(state.src[pos]) {
    pos++
  }
  if pos == state.pos return false

  if !silent state.pending += state.src[state.pos, pos]

  state.pos = pos

  return true
}

# Alternative implementation, for memory.
#
# It costs 10% of performance, but allows extend terminators list, if place it
# to `InlineParser` property. Probably, will switch to it sometime, such
# flexibility required.


/* var TERMINATOR_RE = '/[\n!#$%&*+\-:<=>@[\\\\\]^_`{}~]/'

def text(state, silent) {
  var pos = state.pos,
      idx_base = state.src[pos,].match(TERMINATOR_RE),
      idx = -1
  
  if idx_base {
    idx = state.src[pos,].index_of(idx_base[0])
  }

  # first char is terminator -> empty text
  if idx == 0 return false

  # no terminator -> text till end of string
  if idx < 0 {
    if !silent state.pending += state.src[pos,]
    state.pos = state.src.length()
    return true
  }
  
  if !silent state.pending += state.src[pos, pos + idx]

  state.pos += idx

  return true
} */

