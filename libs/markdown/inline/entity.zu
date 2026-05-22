# Process html entity - &#123;, &#xF;, &quot;, ...

import ..common.entities { entities }
import ..common.utils { is_valid_entity_code, NAMED_RE }

var DIGITAL_RE = '/^&#((?:x[a-f0-9]{1,6}|[0-9]{1,7}));/i'

def entity(state, silent) {
  var ch, code, match, token, pos = state.pos, max = state.pos_max

  if state.src[pos] != '&' return false

  if pos + 1 >= max return false

  ch = state.src[pos + 1]

  if ch == '#' {
    match = state.src[pos,].match(DIGITAL_RE)

    if match {
      if !silent {
        code = to_number(match[1][0].lower() == 'x' ? '0' + match[1].lower() : match[1])

        token         = state.push('text_special', '', 0)
        token.content = is_valid_entity_code(code) ? chr(code) : chr(0xFFFD)
        token.markup  = match[0]
        token.info    = 'entity'
      }
      state.pos += match[0].length()
      return true
    }
  } else {
    match = state.src[pos,].match(NAMED_RE)
    if match {
      if entities.contains(match[1]) {
        if !silent {
          token         = state.push('text_special', '', 0)
          token.content = entities[match[1]]
          token.markup  = match[0]
          token.info    = 'entity'
        }
        state.pos += match[0].length()
        return true
      }
    }
  }

  return false
}

