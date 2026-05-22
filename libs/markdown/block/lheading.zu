# lheading (---, ===)

def lheading(state, start_line, end_line, silent) {
  var content, terminate, i, l, token, pos, max, level, marker,
      nextLine = start_line + 1, old_parent_type,
      terminator_rules = state.md.block.ruler.get_rules('paragraph')

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  old_parent_type = state.parent_type
  state.parent_type = 'paragraph' # use paragraph to match terminator_rules

  # jump line-by-line until empty one or EOF
  iter ; nextLine < end_line and !state.is_empty(nextLine); nextLine++ {
    # this would be a code block normally, but after paragraph
    # it's considered a lazy continuation regardless of what's there
    if state.s_count[nextLine] - state.blk_indent > 3 continue

    #
    # Check for underline in setext header
    #
    if state.s_count[nextLine] >= state.blk_indent {
      pos = state.b_marks[nextLine] + state.t_shift[nextLine]
      max = state.e_marks[nextLine]

      if pos < max {
        marker = state.src[pos]

        if marker == '-' or marker == '=' {
          pos = state.skip_chars(pos, marker)
          pos = state.skip_spaces(pos)

          if pos >= max {
            level = marker == '=' ? 1 : 2
            break
          }
        }
      }
    }

    # quirk for blockquotes, this line should already be checked by that rule
    if state.s_count[nextLine] < 0 continue

    # Some tags can terminate paragraph without empty line.
    terminate = false
    i = 0
    iter l = terminator_rules.length(); i < l; i++ {
      if terminator_rules[i](state, nextLine, end_line, true) {
        terminate = true
        break
      }
    }
    if terminate break
  }

  if !level {
    # Didn't find valid underline
    return false
  }

  content = state.get_lines(start_line, nextLine, state.blk_indent, false).trim()

  state.line = nextLine + 1

  token          = state.push('heading_open', 'h' + level, 1)
  token.markup   = marker
  token.map      = [ start_line, state.line ]

  token          = state.push('inline', '', 0)
  token.content  = content
  token.map      = [ start_line, state.line - 1 ]
  token.children = []

  token          = state.push('heading_close', 'h' + level, -1)
  token.markup   = marker

  state.parent_type = old_parent_type

  return true
}

