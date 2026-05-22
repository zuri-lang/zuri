# Process escaped chars and hard breaks

import ..common.utils { is_space }

# list mapping of escaped ASCII characters.
var ESCAPED = [0] * 256
'\\!"#$%&\'()*+,./:;<=>?@[]^_`{|}~-'.each(@(ch) { ESCAPED[ord(ch)] = 1 })

def escape(state, silent) {
  var ch1, ch2, orig_str, escaped_str, token, pos = state.pos, max = state.pos_max

  if state.src[pos] != '\\' return false
  pos++

  # '\' at the end of the inline block
  if pos >= max return false

  ch1 = ord(state.src[pos])

  if ch1 == 0x0A {
    if !silent {
      state.push('hardbreak', 'br', 0)
    }

    pos++
    # skip leading whitespaces from next line
    while pos < max {
      ch1 = ord(state.src[pos])
      if !is_space(ch1) break
      pos++
    }

    state.pos = pos
    return true
  }

  escaped_str = state.src[pos]

  if ch1 >= 0xD800 and ch1 <= 0xDBFF and pos + 1 < max {
    ch2 = ord(state.src[pos + 1])

    if ch2 >= 0xDC00 and ch2 <= 0xDFFF {
      escaped_str += state.src[pos + 1]
      pos++
    }
  }

  orig_str = '\\' + escaped_str

  if !silent {
    token = state.push('text_special', '', 0)

    if ch1 < 256 and ESCAPED[ch1] != 0 {
      token.content = escaped_str
    } else {
      token.content = orig_str
    }

    token.markup = orig_str
    token.info   = 'escape'
  }

  state.pos = pos + 1
  return true
}

