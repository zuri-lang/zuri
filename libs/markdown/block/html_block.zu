# HTML block

import ..common.html_blocks { html_blocks }
import ..common.html_re { HTML_OPEN_CLOSE_TAG_RE }

# A list of opening and corresponding closing sequences for html tags,
# last argument defines whether it can terminate a paragraph or not
var HTML_SEQUENCES = [
  [ '/^<(script|pre|style|textarea)(?=(\s|>|$))/i', '/<\/(script|pre|style|textarea)>/i', true ],
  [ '/^<!--/',        '/-->/',   true ],
  [ '/^<\?/',         '/\?>/',   true ],
  [ '/^<![A-Z]/',     '/>/',     true ],
  [ '/^<!\[CDATA\[/', '/\]\]>/', true ],
  [ '/^</?(' + '|'.join(html_blocks) + ')(?=(\\s|/?>|$))/i', '/^$/', true ],
  [ '/${HTML_OPEN_CLOSE_TAG_RE}\s*$/',  '/^$/', false ],
]

def html_block(state, start_line, end_line, silent) {
  var i, next_line, token, line_text,
      pos = state.b_marks[start_line] + state.t_shift[start_line],
      max = state.e_marks[start_line]

  # if it's indented more than 3 spaces, it should be a code block
  if state.s_count[start_line] - state.blk_indent >= 4 return false

  if !state.md.options.html return false

  if state.src[pos] != '<' return false

  line_text = state.src[pos, max]

  iter i = 0; i < HTML_SEQUENCES.length(); i++ {
    if line_text.match(HTML_SEQUENCES[i][0]) break
  }

  if i == HTML_SEQUENCES.length() return false

  if silent {
    # true if this sequence can be a terminator, false otherwise
    return HTML_SEQUENCES[i][2]
  }

  next_line = start_line + 1

  # If we are here - we detected HTML block.
  # Let's roll down till block end.
  if !line_text.match(HTML_SEQUENCES[i][1]) {
    iter ; next_line < end_line; next_line++ {
      if state.s_count[next_line] < state.blk_indent break

      pos = state.b_marks[next_line] + state.t_shift[next_line]
      max = state.e_marks[next_line]
      line_text = state.src[pos, max]

      if line_text.match(HTML_SEQUENCES[i][1]) {
        if line_text.length() != 0 next_line++
        break
      }
    }
  }

  state.line = next_line

  token         = state.push('html_block', '', 0)
  token.map     = [ start_line, next_line ]
  token.content = state.get_lines(start_line, next_line, state.blk_indent, true)

  return true
}

