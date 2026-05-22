import ..common.utils { UNESCAPE_RE, UNESCAPE_SPACE_RE }


def subscript(state, silent) {
  var found,
      content,
      token,
      max = state.pos_max,
      start = state.pos

  if state.src[start] != '~' return false
  if silent return false # don't run any pairs in validation mode
  if start + 2 >= max return false

  state.pos = start + 1

  while state.pos < max {
    if state.src[state.pos] == '~' {
      found = true
      break
    }

    state.md.inline.skip_token(state)
  }

  if !found or start + 1 == state.pos {
    state.pos = start
    return false
  }

  content = state.src[start + 1, state.pos]

  # don't allow unescaped spaces/newlines inside
  if content.match(UNESCAPE_SPACE_RE) {
    state.pos = start
    return false
  }

  # found!
  state.pos_max = state.pos
  state.pos = start + 1

  # Earlier we checked !silent, but this implementation does not need it
  token         = state.push('sub_open', 'sub', 1)
  token.markup  = '~'

  token         = state.push('text', '', 0)
  token.content = content.replace(UNESCAPE_RE, '$1')

  token         = state.push('sub_close', 'sub', -1)
  token.markup  = '~'

  state.pos = state.pos_max + 1
  state.pos_max = max
  return true
}

