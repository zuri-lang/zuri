# Block quotes

import ..common.utils { is_space }

def blockquote(state, start_line, end_line, silent) {
  var adjust_tab,
      ch,
      i,
      initial,
      l,
      last_line_empty,
      lines,
      nextLine,
      offset,
      old_bMarks,
      old_bSCount,
      old_indent,
      old_parent_type,
      old_sCount,
      old_tShift,
      space_after_marker,
      terminate,
      terminator_rules,
      token,
      is_outdented,
      old_line_max = state.line_max,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line]

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  # check the block quote marker
  if state.src[pos] != '>' return false

  # we know that it's going to be a valid blockquote,
  # so no point trying to find the end of it in silent mode
  if silent return true

  old_bMarks  = []
  old_bSCount = []
  old_sCount  = []
  old_tShift  = []

  terminator_rules = state.md.block.ruler.get_rules('blockquote')

  old_parent_type = state.parent_type
  state.parent_type = 'blockquote'

  # Search the end of the block
  #
  # Block ends with either:
  #  1. an empty line outside:
  #     ```
  #     > test
  #
  #     ```
  #  2. an empty line inside:
  #     ```
  #     >
  #     test
  #     ```
  #  3. another tag:
  #     ```
  #     > test
  #      - - -
  #     ```
  iter nextLine = start_line; nextLine < end_line; nextLine++ {
    # check if it's outdented, i.e. it's inside list item and indented
    # less than said list item:
    #
    # ```
    # 1. anything
    #    > current blockquote
    # 2. checking this line
    # ```
    is_outdented = state.s_count[nextLine] < state.blk_indent

    pos = state.b_marks[nextLine] + state.t_shift[nextLine]
    max = state.e_marks[nextLine]

    if pos >= max {
      # Case 1: line is not inside the blockquote, and this line is empty.
      break
    }

    if state.src[pos++ - 1] == '>' and !is_outdented {
      # This line is inside the blockquote.

      # set offset past spaces and ">"
      initial = state.s_count[nextLine] + 1

      # skip one optional space after '>'
      if state.src[pos] == ' ' {
        # ' >   test '
        #     ^ -- position start of line here:
        pos++
        initial++
        adjust_tab = false
        space_after_marker = true
      } else if state.src[pos] == '\t' {
        space_after_marker = true

        if (state.bs_count[nextLine] + initial) % 4 == 3 {
          # '  >\t  test '
          #       ^ -- position start of line here (tab has width==1)
          pos++
          initial++
          adjust_tab = false
        } else {
          # ' >\t  test '
          #    ^ -- position start of line here + shift bs_count slightly
          #         to make extra space appear
          adjust_tab = true
        }
      } else {
        space_after_marker = false
      }

      offset = initial
      old_bMarks.append(state.b_marks[nextLine])
      state.b_marks[nextLine] = pos

      while pos < max {
        ch = state.src[pos]

        if is_space(ch) {
          if ch == '\t' {
            offset += 4 - (offset + state.bs_count[nextLine] + (adjust_tab ? 1 : 0)) % 4
          } else {
            offset++
          }
        } else {
          break
        }

        pos++
      }

      last_line_empty = pos >= max

      old_bSCount.append(state.bs_count[nextLine])
      state.bs_count[nextLine] = state.s_count[nextLine] + 1 + (space_after_marker ? 1 : 0)

      old_sCount.append(state.s_count[nextLine])
      state.s_count[nextLine] = offset - initial

      old_tShift.append(state.t_shift[nextLine])
      state.t_shift[nextLine] = pos - state.b_marks[nextLine]
      continue
    }

    # Case 2: line is not inside the blockquote, and the last line was empty.
    if last_line_empty break

    # Case 3: another tag found.
    terminate = false
    i = 0
    iter l = terminator_rules.length(); i < l; i++ {
      if terminator_rules[i](state, nextLine, end_line, true) {
        terminate = true
        break
      }
    }

    if terminate {
      # Quirk to enforce "hard termination mode" for paragraphs;
      # normally if you call `tokenize(state, start_line, nextLine)`,
      # paragraphs will look below nextLine for paragraph continuation,
      # but if blockquote is terminated by another tag, they shouldn't
      state.line_max = nextLine

      if state.blk_indent != 0 {
        # state.blk_indent was non-zero, we now set it to zero,
        # so we need to re-calculate all offsets to appear as
        # if indent wasn't changed
        old_bMarks.append(state.b_marks[nextLine])
        old_bSCount.append(state.bs_count[nextLine])
        old_tShift.append(state.t_shift[nextLine])
        old_sCount.append(state.s_count[nextLine])
        state.s_count[nextLine] -= state.blk_indent
      }

      break
    }

    old_bMarks.append(state.b_marks[nextLine])
    old_bSCount.append(state.bs_count[nextLine])
    old_tShift.append(state.t_shift[nextLine])
    old_sCount.append(state.s_count[nextLine])

    # A negative indentation means that this is a paragraph continuation
    #
    state.s_count[nextLine] = -1
  }

  old_indent = state.blk_indent
  state.blk_indent = 0

  token        = state.push('blockquote_open', 'blockquote', 1)
  token.markup = '>'
  token.map    = lines = [ start_line, 0 ]

  state.md.block.tokenize(state, start_line, nextLine)

  token        = state.push('blockquote_close', 'blockquote', -1)
  token.markup = '>'

  state.line_max = old_line_max
  state.parent_type = old_parent_type
  lines[1] = state.line

  # Restore original t_shift; this might not be necessary since the parser
  # has already been here, but just to make sure we can do that.
  iter i = 0; i < old_tShift.length(); i++ {
    state.b_marks[i + start_line] = old_bMarks[i]
    state.t_shift[i + start_line] = old_tShift[i]
    state.s_count[i + start_line] = old_sCount[i]
    state.bs_count[i + start_line] = old_bSCount[i]
  }
  state.blk_indent = old_indent

  return true
}

