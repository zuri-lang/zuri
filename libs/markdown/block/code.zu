# Code block (4 spaces padded)

def code(state, start_line, end_line, silent) {
  var nextLine, last, token

  if state.s_count[start_line] - state.blk_indent < 4 return false

  last = nextLine = start_line + 1

  while nextLine < end_line {
    if state.is_empty(nextLine) {
      nextLine++
      continue
    }

    if state.s_count[nextLine] - state.blk_indent >= 4 {
      nextLine++
      last = nextLine
      continue
    }
    break
  }

  state.line = last

  token         = state.push('code_block', 'code', 0)
  token.content = state.get_lines(start_line, last, 4 + state.blk_indent, false) + '\n'
  token.map     = [ start_line, state.line ]

  return true
}

