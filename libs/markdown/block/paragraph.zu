# Paragraph

def paragraph(state, start_line, end_line, silent) {
  var content, terminate, i, l, token, old_parent_type,
      nextLine = start_line + 1,
      terminator_rules = state.md.block.ruler.get_rules('paragraph')

  old_parent_type = state.parent_type
  state.parent_type = 'paragraph'

  # jump line-by-line until empty one or EOF
  iter ; nextLine < end_line and !state.is_empty(nextLine); nextLine++ {
    # this would be a code block normally, but after paragraph
    # it's considered a lazy continuation regardless of what's there
    if state.s_count[nextLine] - state.blk_indent > 3 continue

    # quirk for blockquotes, this line should already be checked by that rule
    if state.s_count[nextLine] < 0 continue

    # Some tags can terminate paragraph without empty line.
    terminate = false
    l = terminator_rules.length()
    iter i = 0; i < l; i++ {
      if terminator_rules[i](state, nextLine, end_line, true) {
        terminate = true
        break
      }
    }
    if terminate break
  }

  content = state.get_lines(start_line, nextLine, state.blk_indent, false).trim()

  state.line = nextLine

  token          = state.push('paragraph_open', 'p', 1)
  token.map      = [ start_line, state.line ]

  token          = state.push('inline', '', 0)
  token.content  = content
  token.map      = [ start_line, state.line ]
  token.children = []

  token          = state.push('paragraph_close', 'p', -1)

  state.parent_type = old_parent_type

  return true
}

