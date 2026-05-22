# Parse backticks

def backticks(state, silent) {
  var start, max, marker, token, match_start, match_end, opener_length, closer_length,
      pos = state.pos,
      ch = state.src[pos]

  if ch != '`' return false

  start = pos
  pos++
  max = state.pos_max

  # scan marker length
  while pos < max and state.src[pos] == '`' pos++

  marker = state.src[start, pos]
  opener_length = marker.length()

  if state.backticks_scanned and state.backticks.get(opener_length, 0) <= start {
    if !silent state.pending += marker
    state.pos += opener_length
    return true
  }

  match_end = pos

  # Nothing found in the cache, scan until the end of the line (or until marker is found)
  while (match_start = state.src.index_of('`', match_end)) != -1 {
    match_end = match_start + 1

    # scan marker length
    while match_end < max and state.src[match_end] == '`' match_end++

    closer_length = match_end - match_start

    if closer_length == opener_length {
      # Found matching closer length.
      if !silent {
        token     = state.push('code_inline', 'code', 0)
        token.markup  = marker
        token.content = state.src[pos, match_start].
          replace('/\n/', ' ').
          replace('/^ (.+) $/', '$1')
      }
      state.pos = match_end
      return true
    }

    # Some different length found, put it in cache as upper limit of where closer can be found
    state.backticks[closer_length] = match_start
  }

  # Scanned through the end, didn't find anything
  state.backticks_scanned = true

  if !silent state.pending += marker
  state.pos += opener_length
  return true
}

