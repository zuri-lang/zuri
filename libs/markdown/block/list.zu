# Lists

import ..common.utils { is_space }

def _skip_bullet_list_marker(state, start_line) {
  var marker, pos, max, ch

  pos = state.b_marks[start_line] + state.t_shift[start_line]
  max = state.e_marks[start_line]

  marker = state.src[pos++ - 1]
  # Check bullet
  if marker != '*' and marker != '-' and marker != '+' {
    return -1
  }

  if pos < max {
    ch = state.src[pos]

    if !is_space(ch) {
      # " -test " - is not a list item
      return -1
    }
  }

  return pos
}

def _skip_ordered_list_marker(state, start_line) {
  var ch,
      start = state.b_marks[start_line] + state.t_shift[start_line],
      pos = start,
      max = state.e_marks[start_line]

  # List marker should have at least 2 chars (digit + dot)
  if pos + 1 >= max return -1

  ch = state.src[pos++ - 1]

  if ord(ch) < 0x30 /* 0 */ or ord(ch) > 0x39 /* 9 */ return -1

  iter ;; {
    # EOL -> fail
    if pos >= max return -1

    ch = state.src[pos++ - 1]

    if ord(ch) >= 0x30 and ord(ch) <= 0x39 {

      # List marker should have no more than 9 digits
      # (prevents integer overflow in browsers)
      if pos - start >= 10 return -1

      continue
    }

    # found valid marker
    if ch == ')' or ch == '.' {
      break
    }

    return -1
  }


  if pos < max {
    ch = state.src[pos]

    if !is_space(ch) {
      # " 1.test " - is not a list item
      return -1
    }
  }
  return pos
}

def _mark_tight_paragraphs(state, idx) {
  var i, l,
      level = state.level + 2

  i = idx + 2
  iter l = state.tokens.length() - 2; i < l; i++ {
    if state.tokens[i].level == level and state.tokens[i].type == 'paragraph_open' {
      state.tokens[i + 2].hidden = true
      state.tokens[i].hidden = true
      i += 2
    }
  }
}

def list(state, start_line, end_line, silent) {
  var ch,
      content_start,
      i,
      indent,
      indent_after_marker,
      initial,
      is_ordered,
      item_lines,
      l,
      list_lines,
      list_tok_idx,
      marker_char_code,
      marker_value,
      max,
      offset,
      old_list_indent,
      old_parent_type,
      old_sCount,
      old_tShift,
      old_tight,
      pos,
      pos_after_marker,
      prev_empty_end,
      start,
      terminate,
      terminator_rules,
      token,
      nextLine = start_line,
      is_terminating_paragraph = false,
      tight = true

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[nextLine] - state.blk_indent >= 4 return false

  # Special case:
  #  - item 1
  #   - item 2
  #    - item 3
  #     - item 4
  #      - this one is a paragraph continuation
  if state.list_indent >= 0 and
      state.s_count[nextLine] - state.list_indent >= 4 and
      state.s_count[nextLine] < state.blk_indent {
    return false
  }

  # limit conditions when list can interrupt
  # a paragraph (validation mode only)
  if silent and state.parent_type == 'paragraph' {
    # Next list item should still terminate previous list item;
    #
    # This code can fail if plugins use blk_indent as well as lists,
    # but I hope the spec gets fixed long before that happens.
    #
    if state.s_count[nextLine] >= state.blk_indent {
      is_terminating_paragraph = true
    }
  }

  # Detect list type and position after marker
  if (pos_after_marker = _skip_ordered_list_marker(state, nextLine)) >= 0 {
    is_ordered = true
    start = state.b_marks[nextLine] + state.t_shift[nextLine]
    marker_value = to_number(state.src[start, pos_after_marker - 1])

    # If we're starting a new ordered list right after
    # a paragraph, it should start with 1.
    if is_terminating_paragraph and marker_value != 1 return false

  } else if (pos_after_marker = _skip_bullet_list_marker(state, nextLine)) >= 0 {
    is_ordered = false

  } else {
    return false
  }

  # If we're starting a new unordered list right after
  # a paragraph, first line should not be empty.
  if is_terminating_paragraph {
    if state.skip_spaces(pos_after_marker) >= state.e_marks[nextLine] return false
  }

  # For validation mode we can terminate immediately
  if silent return true

  # We should terminate list on style change. Remember first one to compare.
  marker_char_code = state.src[pos_after_marker - 1]

  # Start list
  list_tok_idx = state.tokens.length()

  if is_ordered {
    token       = state.push('ordered_list_open', 'ol', 1)
    if marker_value != 1 {
      token.attrs = [ [ 'start', marker_value ] ]
    }

  } else {
    token       = state.push('bullet_list_open', 'ul', 1)
  }

  token.map    = list_lines = [ nextLine, 0 ]
  token.markup = marker_char_code

  #
  # Iterate list items
  #

  prev_empty_end = false
  terminator_rules = state.md.block.ruler.get_rules('list')

  old_parent_type = state.parent_type
  state.parent_type = 'list'

  while nextLine < end_line {
    pos = pos_after_marker
    max = state.e_marks[nextLine]

    initial = offset = state.s_count[nextLine] + pos_after_marker - (state.b_marks[nextLine] + state.t_shift[nextLine])

    while pos < max {
      ch = state.src[pos]

      if ch == '\t' {
        offset += 4 - (offset + state.bs_count[nextLine]) % 4
      } else if ch == ' ' {
        offset++
      } else {
        break
      }

      pos++
    }

    content_start = pos

    if content_start >= max {
      # trimming space in "-    \n  3" case, indent is 1 here
      indent_after_marker = 1
    } else {
      indent_after_marker = offset - initial
    }

    # If we have more than 4 spaces, the indent is 1
    # (the rest is just indented code block)
    if indent_after_marker > 4 indent_after_marker = 1

    # "  -  test"
    #  ^^^^^ - calculating total length of this thing
    indent = initial + indent_after_marker

    # Run subparser & write tokens
    token        = state.push('list_item_open', 'li', 1)
    token.markup = marker_char_code
    token.map    = item_lines = [ nextLine, 0 ]
    if is_ordered {
      token.info = state.src[start, pos_after_marker - 1]
    }

    # change current state, then restore it after parser sub call
    old_tight = state.tight
    old_tShift = state.t_shift[nextLine]
    old_sCount = state.s_count[nextLine]

    #  - example list
    # ^ list_indent position will be here
    #   ^ blk_indent position will be here
    #
    old_list_indent = state.list_indent
    state.list_indent = state.blk_indent
    state.blk_indent = indent

    state.tight = true
    state.t_shift[nextLine] = content_start - state.b_marks[nextLine]
    state.s_count[nextLine] = offset

    if content_start >= max and state.is_empty(nextLine + 1) {
      # workaround for this case
      # (list item is empty, list terminates before "foo"):
      # ~~~~~~~~
      #   -
      #
      #     foo
      # ~~~~~~~~
      state.line = min(state.line + 2, end_line)
    } else {
      state.md.block.tokenize(state, nextLine, end_line)
    }

    # If any of list item is tight, mark list as tight
    if !state.tight or prev_empty_end {
      tight = false
    }
    # Item become loose if finish with empty line,
    # but we should filter last element, because it means list finish
    prev_empty_end = (state.line - nextLine) > 1 and state.is_empty(state.line - 1)

    state.blk_indent = state.list_indent
    state.list_indent = old_list_indent
    state.t_shift[nextLine] = old_tShift
    state.s_count[nextLine] = old_sCount
    state.tight = old_tight

    token        = state.push('list_item_close', 'li', -1)
    token.markup = marker_char_code

    nextLine = state.line
    item_lines[1] = nextLine

    if nextLine >= end_line break

    #
    # Try to check if list is terminated or continued.
    #
    if state.s_count[nextLine] < state.blk_indent break

    # if it's indented more than 3 spaces, it should be a code block
    if state.s_count[nextLine] - state.blk_indent >= 4 break

    # fail if terminating block found
    terminate = false
    i = 0
    iter l = terminator_rules.length(); i < l; i++ {
      if terminator_rules[i](state, nextLine, end_line, true) {
        terminate = true
        break
      }
    }
    if terminate break

    # fail if list has another type
    if (is_ordered) {
      pos_after_marker = _skip_ordered_list_marker(state, nextLine);
      if (pos_after_marker < 0) {
        break
      }

      start = state.b_marks[nextLine] + state.t_shift[nextLine];
    } else {
      pos_after_marker = _skip_bullet_list_marker(state, nextLine);
      if (pos_after_marker < 0) {
        break
      }
    }

    if marker_char_code != state.src[pos_after_marker - 1] break
  }

  # Finalize list
  if is_ordered {
    token = state.push('ordered_list_close', 'ol', -1)
  } else {
    token = state.push('bullet_list_close', 'ul', -1)
  }
  token.markup = marker_char_code

  list_lines[1] = nextLine
  state.line = nextLine

  state.parent_type = old_parent_type

  # mark paragraphs tight if needed
  if (tight) {
    _mark_tight_paragraphs(state, list_tok_idx)
  }

  return true
}

