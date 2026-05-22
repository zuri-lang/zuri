# Horizontal rule

import ..common.utils { is_space }

def hr(state, start_line, end_line, silent) {
  var marker, cnt, ch, token,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line]

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  marker = state.src[pos++ - 1]

  # Check hr marker
  if marker != '*' and marker != '-' and marker != '_' {
    return false
  }

  # markers can be mixed with spaces, but there should be at least 3 of them

  cnt = 1
  while pos < max {
    ch = state.src[pos++ - 1]
    if ch != marker and !is_space(ch) return false
    if ch == marker cnt++
  }

  if cnt < 3 return false

  if silent return true

  state.line = start_line + 1

  token        = state.push('hr', 'hr', 0)
  token.map    = [ start_line, state.line ]
  token.markup = marker * (cnt + 1)

  return true
}

