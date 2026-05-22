import ..common.utils { is_space }

# Search `[:~][\n ]`, returns next pos after marker on success
# or -1 on fail.
def skip_marker(state, line) {
  var pos, marker,
      start = state.b_marks[line] + state.t_shift[line],
      max = state.e_marks[line]

  if start >= max return -1

  # Check bullet
  marker = state.src[start++ - 1]
  if marker != '~' and marker != ':' return -1

  pos = state.skip_spaces(start)

  # require space after ":"
  if start == pos return -1

  # no empty definitions, e.g. "  : "
  if pos >= max return -1

  return start
}

def mark_tight_paragraphs(state, idx) {
  var i = idx + 2, l,
      level = state.level + 2

  iter l = state.tokens.length() - 2; i < l; i++ {
    if state.tokens[i].level == level and state.tokens[i].type == 'paragraph_open' {
      state.tokens[i + 2].hidden = true
      state.tokens[i].hidden = true
      i += 2
    }
  }
}

def deflist(state, start_line, end_line, silent) {
  var ch,
      content_start,
      dd_line,
      dt_line,
      item_lines,
      list_lines,
      list_tok_idx,
      max,
      next_line,
      offset,
      old_dDIndent,
      old_indent,
      old_parent_type,
      old_sCount,
      old_tShift,
      old_tight,
      pos,
      prev_empty_end,
      tight,
      token

  if silent {
    # quirk: validation mode validates a dd block only, not a whole deflist
    if state.dd_indent < 0 return false
    return skip_marker(state, start_line) >= 0
  }

  next_line = start_line + 1
  if next_line >= end_line return false

  if state.is_empty(next_line) {
    next_line++
    if next_line >= end_line return false
  }

  if state.s_count[next_line] < state.blk_indent return false
  content_start = skip_marker(state, next_line)
  if content_start < 0 return false

  # Start list
  list_tok_idx = state.tokens.length()
  tight = true

  token     = state.push('dl_open', 'dl', 1)
  token.map = list_lines = [ start_line, 0 ]

  #
  # Iterate list items
  #

  dt_line = start_line
  dd_line = next_line

  # One definition list can contain multiple DTs,
  # and one DT can be followed by multiple DDs.
  #
  # Thus, there is two loops here, and label is
  # needed to break out of the second one
  #
  var outer_break = false
  iter ;; {
    prev_empty_end = false

    token          = state.push('dt_open', 'dt', 1)
    token.map      = [ dt_line, dt_line ]

    token          = state.push('inline', '', 0)
    token.map      = [ dt_line, dt_line ]
    token.content  = state.get_lines(dt_line, dt_line + 1, state.blk_indent, false).trim()
    token.children = []

    token          = state.push('dt_close', 'dt', -1)

    iter ;; {
      token     = state.push('dd_open', 'dd', 1)
      token.map = item_lines = [ next_line, 0 ]

      pos = content_start
      max = state.e_marks[dd_line]
      offset = state.s_count[dd_line] + content_start - (state.b_marks[dd_line] + state.t_shift[dd_line])

      while pos < max {
        ch = state.src[pos]

        if is_space(ch) {
          if ch == '\t' {
            offset += 4 - offset % 4
          } else {
            offset++
          }
        } else {
          break
        }

        pos++
      }

      content_start = pos

      old_tight = state.tight
      old_dDIndent = state.dd_indent
      old_indent = state.blk_indent
      old_tShift = state.t_shift[dd_line]
      old_sCount = state.s_count[dd_line]
      old_parent_type = state.parent_type
      state.blk_indent = state.dd_indent = state.s_count[dd_line] + 2
      state.t_shift[dd_line] = content_start - state.b_marks[dd_line]
      state.s_count[dd_line] = offset
      state.tight = true
      state.parent_type = 'deflist'

      state.md.block.tokenize(state, dd_line, end_line)

      # If any of list item is tight, mark list as tight
      if !state.tight or prev_empty_end {
        tight = false
      }
      # Item become loose if finish with empty line,
      # but we should filter last element, because it means list finish
      prev_empty_end = (state.line - dd_line) > 1 and state.is_empty(state.line - 1)

      state.t_shift[dd_line] = old_tShift
      state.s_count[dd_line] = old_sCount
      state.tight = old_tight
      state.parent_type = old_parent_type
      state.blk_indent = old_indent
      state.dd_indent = old_dDIndent

      token = state.push('dd_close', 'dd', -1)

      item_lines[1] = next_line = state.line

      if next_line >= end_line { 
        outer_break = true
        break
      }

      if state.s_count[next_line] < state.blk_indent {
        outer_break = true
        break
      }
      content_start = skip_marker(state, next_line)
      if content_start < 0 break

      dd_line = next_line

      # go to the next loop iteration:
      # insert DD tag and repeat checking
    }

    if outer_break {
      break
    }

    if next_line >= end_line break
    dt_line = next_line

    if state.is_empty(dt_line) break
    if state.s_count[dt_line] < state.blk_indent break

    dd_line = dt_line + 1
    if dd_line >= end_line break
    if state.is_empty(dd_line) dd_line++
    if dd_line >= end_line break

    if state.s_count[dd_line] < state.blk_indent break
    content_start = skip_marker(state, dd_line)
    if content_start < 0 break

    # go to the next loop iteration:
    # insert DT and DD tags and repeat checking
  }

  # Finalize list
  token = state.push('dl_close', 'dl', -1)

  list_lines[1] = next_line

  state.line = next_line

  # mark paragraphs tight if needed
  if tight {
    mark_tight_paragraphs(state, list_tok_idx)
  }

  return true
}

