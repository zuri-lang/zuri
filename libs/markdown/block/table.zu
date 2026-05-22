# GFM table, https://github.github.com/gfm/#tables-extension-

import ..common.utils { is_space }

def _get_line(state, line) {
  var pos = state.b_marks[line] + state.t_shift[line],
      max = state.e_marks[line]

  return state.src[pos, max]
}

def _escaped_split(str) {
  var result = [],
      pos = 0,
      max = str.length(),
      ch,
      is_escaped = false,
      last_pos = 0,
      current = ''

  ch  = str[pos]

  while pos < max {
    if ch == '|' {
      if !is_escaped {
        # pipe separating cells, '|'
        result.append(current + str[last_pos, pos])
        current = ''
        last_pos = pos + 1
      } else {
        # escaped pipe, '\|'
        current += str[last_pos, pos - 1]
        last_pos = pos
      }
    }
    
    is_escaped = ch == '\\'
    pos++

    # using a slice so we can avoid error due to unknown index
    ch = str[pos, pos + 1]
  }

  result.append(current + str[last_pos,])

  return result
}

def table(state, start_line, end_line, silent) {
  var ch, line_text, pos, i, l, nextLine, columns, column_count, token,
      aligns, t, table_lines, tbody_lines, old_parent_type, terminate,
      terminator_rules, first_ch, second_ch

  # should have at least two lines
  if start_line + 2 > end_line return false

  nextLine = start_line + 1

  if state.s_count[nextLine] < state.blk_indent return false

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[nextLine] - state.blk_indent >= 4 return false

  # first character of the second line should be '|', '-', ':',
  # and no other characters are allowed but spaces;
  # basically, this is the equivalent of /^[-:|][-:|\s]*$/ regexp

  pos = state.b_marks[nextLine] + state.t_shift[nextLine]
  if pos >= state.e_marks[nextLine] return false

  first_ch = state.src[pos++ - 1]
  if first_ch != '|' and first_ch != '-' and first_ch != ':' return false

  if pos >= state.e_marks[nextLine] return false

  second_ch = state.src[pos++ - 1]
  if second_ch != '|' and second_ch != '-' and second_ch != ':' and !is_space(second_ch) {
    return false
  }

  # if first character is '-', then second character must not be a space
  # (due to parsing ambiguity with list)
  if first_ch == '-' and is_space(second_ch) return false

  while pos < state.e_marks[nextLine] {
    ch = state.src[pos]

    if ch != '|' and ch != '-' and ch != ':' and !is_space(ch) return false

    pos++
  }

  line_text = _get_line(state, start_line + 1)

  columns = line_text.split('|')
  aligns = []
  iter i = 0; i < columns.length(); i++ {
    t = columns[i].trim()
    if !t {
      # allow empty columns before and after table, but not in between columns;
      # e.g. allow ` |---| `, disallow ` ---||--- `
      if i == 0 or i == columns.length() - 1 {
        continue
      } else {
        return false
      }
    }

    if !t.match('/^:?-+:?$/') return false
    if t[-1] == ':' {
      aligns.append(t[0] == ':' ? 'center' : 'right')
    } else if t[0] == ':' {
      aligns.append('left')
    } else {
      aligns.append('')
    }
  }

  line_text = _get_line(state, start_line).trim()
  if line_text.index_of('|') == -1 return false
  if state.s_count[start_line] - state.blk_indent >= 4 return false
  columns = _escaped_split(line_text)
  if columns and columns[0] == '' columns.shift()
  if columns and columns[-1] == '' columns.pop()

  # header row will define an amount of columns in the entire table,
  # and align row should be exactly the same (the rest of the rows can differ)
  column_count = columns.length()
  if column_count == 0 or column_count != aligns.length() return false

  if silent return true

  old_parent_type = state.parent_type
  state.parent_type = 'table'

  # use 'blockquote' lists for termination because it's
  # the most similar to tables
  terminator_rules = state.md.block.ruler.get_rules('blockquote')

  token     = state.push('table_open', 'table', 1)
  token.map = table_lines = [ start_line, 0 ]

  token     = state.push('thead_open', 'thead', 1)
  token.map = [ start_line, start_line + 1 ]

  token     = state.push('tr_open', 'tr', 1)
  token.map = [ start_line, start_line + 1 ]

  iter i = 0; i < columns.length(); i++ {
    token          = state.push('th_open', 'th', 1)
    if aligns[i] {
      token.attrs  = [ [ 'style', 'text-align:' + aligns[i] ] ]
    }

    token          = state.push('inline', '', 0)
    token.content  = columns[i].trim()
    token.children = []

    token          = state.push('th_close', 'th', -1)
  }

  token     = state.push('tr_close', 'tr', -1)
  token     = state.push('thead_close', 'thead', -1)

  iter nextLine = start_line + 2; nextLine < end_line; nextLine++ {
    if state.s_count[nextLine] < state.blk_indent break

    terminate = false
    i = 0
    iter l = terminator_rules.length(); i < l; i++ {
      if terminator_rules[i](state, nextLine, end_line, true) {
        terminate = true
        break
      }
    }

    if terminate break
    line_text = _get_line(state, nextLine).trim()
    if !line_text break
    if state.s_count[nextLine] - state.blk_indent >= 4 break
    columns = _escaped_split(line_text)
    if columns and columns[0] == '' columns.shift()
    if columns and columns[-1] == '' columns.pop()

    if nextLine == start_line + 2 {
      token     = state.push('tbody_open', 'tbody', 1)
      token.map = tbody_lines = [ start_line + 2, 0 ]
    }

    token     = state.push('tr_open', 'tr', 1)
    token.map = [ nextLine, nextLine + 1 ]

    iter i = 0; i < column_count; i++ {
      token          = state.push('td_open', 'td', 1)
      if aligns[i] {
        token.attrs  = [ [ 'style', 'text-align:' + aligns[i] ] ]
      }

      token          = state.push('inline', '', 0)
      token.content  = columns.get(i) ? columns.get(i).trim() : ''
      token.children = []

      token          = state.push('td_close', 'td', -1)
    }
    token = state.push('tr_close', 'tr', -1)
  }

  if tbody_lines {
    token = state.push('tbody_close', 'tbody', -1)
    tbody_lines[1] = nextLine
  }

  token = state.push('table_close', 'table', -1)
  table_lines[1] = nextLine

  state.parent_type = old_parent_type
  state.line = nextLine
  return true
}

