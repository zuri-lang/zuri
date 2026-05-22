import ..common.utils { is_space }
import ..common.utils { normalize_reference }

def reference(state, start_line, _end_line, silent) {
  var ch,
      dest_end_pos,
      dest_end_line_no,
      end_line,
      href,
      i,
      l,
      label,
      label_end = -1,
      old_parent_type,
      res,
      start,
      str,
      terminate,
      terminator_rules,
      title,
      lines = 0,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line],
      nextLine = start_line + 1

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  if state.src[pos] != '[' return false

  # Simple check to quickly interrupt scan on [link](url) at the start of line.
  # Can be useful on practice: https://github.com/markdown-it/markdown-it/issues/54
  while pos++ < max {
    if state.src[pos] == ']' and state.src[pos - 1] != '\\' {
      if pos + 1 == max return false
      if state.src[pos + 1] != ':' return false
      break
    }
  }

  end_line = state.line_max

  # jump line-by-line until empty one or EOF
  terminator_rules = state.md.block.ruler.get_rules('reference')

  old_parent_type = state.parent_type
  state.parent_type = 'reference'

  iter ; nextLine < end_line and !state.is_empty(nextLine); nextLine++ {
    # this would be a code block normally, but after paragraph
    # it's considered a lazy continuation regardless of what's there
    if state.s_count[nextLine] - state.blk_indent > 3 continue

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

  str = state.get_lines(start_line, nextLine, state.blk_indent, false).trim()
  max = str.length()

  iter pos = 1; pos < max; pos++ {
    ch = str[pos]
    if ch == '[' {
      return false
    } else if ch == ']' {
      label_end = pos
      break
    } else if ch == '\n' {
      lines++
    } else if ch == '\\' {
      pos++
      if pos < max and str[pos] == '\n' {
        lines++
      }
    }
  }

  if label_end < 0 or str.length() <= label_end + 1 or str[label_end + 1] != ':' return false

  # [label]:   destination   'title'
  #         ^^^ skip optional whitespace here
  iter pos = label_end + 2; pos < max; pos++ {
    ch = str[pos]
    if ch == '\n' {
      lines++
    } else if is_space(ch) {
    } else {
      break
    }
  }

  # [label]:   destination   'title'
  #            ^^^^^^^^^^^ parse this
  res = state.md.helpers.parse_link_destination(str, pos, max)
  if !res.ok return false


  href = state.md.normalize_link(res.str)
  if !state.md.validate_link(href) return false

  pos = res.pos
  lines += res.lines

  # save cursor state, we could require to rollback later
  dest_end_pos = pos
  dest_end_line_no = lines

  # [label]:   destination   'title'
  #                       ^^^ skipping those spaces
  start = pos
  iter ; pos < max; pos++ {
    ch = str[pos]
    if ch == '\n' {
      lines++
    } else if is_space(ch) {
    } else {
      break
    }
  }

  # [label]:   destination   'title'
  #                          ^^^^^^^ parse this
  res = state.md.helpers.parse_link_title(str, pos, max)
  if pos < max and start != pos and res.ok {
    title = res.str
    pos = res.pos
    lines += res.lines
  } else {
    title = ''
    pos = dest_end_pos
    lines = dest_end_line_no
  }

  # skip trailing spaces until the rest of the line
  while pos < max {
    ch = str[pos]
    if !is_space(ch) break
    pos++
  }

  if pos < max and str[pos] != '\n' {
    if title {
      # garbage at the end of the line after title,
      # but it could still be a valid reference if we roll back
      title = ''
      pos = dest_end_pos
      lines = dest_end_line_no
      while pos < max {
        ch = str[pos]
        if !is_space(ch) break
        pos++
      }
    }
  }

  if pos < max and str[pos] != '\n' {
    # garbage at the end of the line
    return false
  }

  label = normalize_reference(str[1, label_end])
  if !label {
    # CommonMark 0.20 disallows empty labels
    return false
  }

  # Reference can not terminate anything. This check is for safety only.
  /*istanbul ignore if*/
  if silent return true

  if !state.env.contains('references') {
    state.env.references = {}
  }
  if !state.env.references.contains(label) {
    state.env.references[label] = { title: title, href: href }
  }

  state.parent_type = old_parent_type

  state.line = start_line + lines + 1
  return true
}

