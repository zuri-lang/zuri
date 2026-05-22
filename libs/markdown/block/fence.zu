# fences (``` lang, ~~~ lang)

def fence(state, start_line, end_line, silent) {
  var marker, len, params, nextLine, mem, token, markup,
      have_end_marker = false,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line]

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  if pos + 3 > max return false

  marker = state.src[pos]

  if marker != '~' and marker != '`' {
    return false
  }

  # scan marker length
  mem = pos
  pos = state.skip_chars(pos, marker)

  len = pos - mem

  if len < 3 return false

  markup = state.src[mem, pos]
  params = state.src[pos, max]

  if marker == '`' {
    if params.index_of(marker) >= 0 {
      return false
    }
  }

  # Since start is found, we can report success here in validation mode
  if silent return true

  # search end of block
  nextLine = start_line;

  iter ;; {
    nextLine++
    if nextLine >= end_line {
      # unclosed block should be autoclosed by end of document.
      # also block seems to be autoclosed by end of parent
      break
    }

    pos = mem = state.b_marks[nextLine] + state.t_shift[nextLine]
    max = state.e_marks[nextLine]

    if pos < max and state.s_count[nextLine] < state.blk_indent {
      # non-empty line with negative indent should stop the list:
      # - ```
      #  test
      break
    }

    if state.src[pos] != marker continue

    if state.s_count[nextLine] - state.blk_indent >= 4 {
      # closing fence should be indented less than 4 spaces
      continue
    }

    pos = state.skip_chars(pos, marker)

    # closing code fence must be at least as long as the opening one
    if pos - mem < len continue

    # make sure tail has spaces only
    pos = state.skip_spaces(pos)

    if pos < max continue

    have_end_marker = true
    # found!
    break
  }

  # If a fence has heading spaces, they should be removed from its inner block
  len = state.s_count[start_line]

  state.line = nextLine + (have_end_marker ? 1 : 0)

  token         = state.push('fence', 'code', 0)
  token.info    = params
  token.content = state.get_lines(start_line + 1, nextLine, len, true)
  token.markup  = markup
  token.map     = [ start_line, state.line ]

  return true
}

