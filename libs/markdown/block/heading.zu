# heading (#, ##, ...)

import ..common.utils { is_space }

def heading(state, start_line, end_line, silent) {
  var ch, level, tmp, token,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line]

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  ch  = state.src[pos]

  if ch != '#' or pos >= max return false

  # count heading level
  level = 1
  ch = state.src[pos++]
  while ch == '#' and pos < max and level <= 6 {
    level++
    ch = state.src[pos++]
  }

  if level > 6 or (pos < max and !is_space(ch)) return false

  if silent return true

  # Let's cut tails like '    ###  ' from the end of string

  max = state.skip_spaces_back(max, pos)
  tmp = state.skip_chars_back(max, '#', pos) # #
  if tmp > pos and is_space(state.src[tmp - 1]) {
    max = tmp
  }

  state.line = start_line + 1

  token        = state.push('heading_open', 'h' + level, 1)
  token.markup = '########'[,level]
  token.map    = [ start_line, state.line ]

  token          = state.push('inline', '', 0)
  token.content  = state.src[pos, max].trim()
  token.map      = [ start_line, state.line ]
  token.children = []

  token        = state.push('heading_close', 'h' + level, -1)
  token.markup = '########'[,level]

  return true
}

