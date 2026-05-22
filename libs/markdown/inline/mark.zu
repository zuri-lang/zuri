# Insert each marker as a separate text token, and add it to delimiter list
#
def tokenize(state, silent) {
  var i, scanned, token, len, ch,
      start = state.pos,
      marker = state.src[start]

  if silent return false

  if marker != '=' return false

  scanned = state.scan_delims(state.pos, true)
  len = scanned.length
  ch = marker

  if len < 2 return false

  if len % 2 > 0 {
    token         = state.push('text', '', 0)
    token.content = ch
    len--
  }

  iter i = 0; i < len; i += 2 {
    token         = state.push('text', '', 0)
    token.content = ch + ch

    if !scanned.can_open and !scanned.can_close continue

    state.delimiters.append({
      marker: marker,
      length: 0,     # disable "rule of 3" length checks meant for emphasis
      jump:   i / 2, # 1 delimiter = 2 characters
      token:  state.tokens.length() - 1,
      end:    -1,
      open:   scanned.can_open,
      close:  scanned.can_close
    })
  }

  state.pos += scanned.length

  return true
}


# Walk through delimiter list and replace text tokens with tags
#
def _post_process(state, delimiters) {
  var i, j,
      start_delim,
      end_delim,
      token,
      lone_markers = [],
      max = delimiters.length()

  iter i = 0; i < max; i++ {
    start_delim = delimiters[i]

    if start_delim.marker != '=' {
      continue
    }

    if start_delim.end == -1 {
      continue
    }

    end_delim = delimiters[start_delim.end]

    token         = state.tokens[start_delim.token]
    token.type    = 'mark_open'
    token.tag     = 'mark'
    token.nesting = 1
    token.markup  = '=='
    token.content = ''

    token         = state.tokens[end_delim.token]
    token.type    = 'mark_close'
    token.tag     = 'mark'
    token.nesting = -1
    token.markup  = '=='
    token.content = ''

    if (state.tokens[end_delim.token - 1].type == 'text' and
        state.tokens[end_delim.token - 1].content == '=') {

      lone_markers.append(end_delim.token - 1)
    }
  }

  # If a marker sequence has an odd number of characters, it's split
  # like this: `~~~~~` -> `~` + `~~` + `~~`, leaving one marker at the
  # start of the sequence.
  #
  # So, we have to move all those markers after subsequent s_close tags.
  #
  while lone_markers.length() > 0 {
    i = lone_markers.pop()
    j = i + 1

    while j < state.tokens.length and state.tokens[j].type == 'mark_close' {
      j++
    }

    j--

    if i != j {
      token = state.tokens[j]
      state.tokens[j] = state.tokens[i]
      state.tokens[i] = token
    }
  }
}

def post_process(state) {
  var curr,
      tokens_meta = state.tokens_meta,
      max = (state.tokens_meta or []).length()

  _post_process(state, state.delimiters)

  iter curr = 0; curr < max; curr++ {
    if tokens_meta[curr] and tokens_meta[curr].delimiters {
      _post_process(state, tokens_meta[curr].delimiters)
    }
  }
}