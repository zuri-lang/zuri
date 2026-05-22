# Process *this* and _that_

# Insert each marker as a separate text token, and add it to delimiter list
def tokenize(state, silent) {
  var i, scanned, token,
      start = state.pos,
      marker = state.src[start]

  if silent return false

  if marker != '_' and marker != '*' return false

  scanned = state.scan_delims(state.pos, marker == '*')

  iter i = 0; i < scanned.length; i++ {
    token         = state.push('text', '', 0)
    token.content = marker

    state.delimiters.append({
      #Char code of the starting marker (number).
      marker: marker,

      #Total length of these series of delimiters.
      length: scanned.length,

      #A position of the token this delimiter corresponds to.
      token:  state.tokens.length() - 1,

      #If this delimiter is matched as a valid opener, `end` will be
      #equal to its position, otherwise it's `-1`.
      end:    -1,

      #Boolean flags that determine if this delimiter could open or close
      #an emphasis.
      open:   scanned.can_open,
      close:  scanned.can_close
    })
  }

  state.pos += scanned.length

  return true
}

def _post_process(state, delimiters) {
  var i,
      start_delim,
      end_delim,
      token,
      ch,
      is_strong,
      max = delimiters.length()

  iter i = max - 1; i >= 0; i-- {
    start_delim = delimiters[i]

    if start_delim.marker != '_' and start_delim.marker != '*' {
      continue
    }
    
    #Process only opening markers
    if start_delim.end == -1 {
      continue
    }
    
    end_delim = delimiters[start_delim.end]

    # If the previous delimiter has the same marker and is adjacent to this one,
    # merge those into one strong delimiter.
    #
    # `<em><em>whatever</em></em>` -> `<strong>whatever</strong>`
    is_strong = i > 0 and
               delimiters[i - 1].end == start_delim.end + 1 and
               #check that first two markers match and adjacent
               delimiters[i - 1].marker == start_delim.marker and
               delimiters[i - 1].token == start_delim.token - 1 and
               #check that last two markers are adjacent (we can safely assume they match)
               delimiters[start_delim.end + 1].token == end_delim.token + 1

    ch = start_delim.marker

    token         = state.tokens[start_delim.token]
    token.type    = is_strong ? 'strong_open' : 'em_open'
    token.tag     = is_strong ? 'strong' : 'em'
    token.nesting = 1
    token.markup  = is_strong ? ch + ch : ch
    token.content = ''

    token         = state.tokens[end_delim.token]
    token.type    = is_strong ? 'strong_close' : 'em_close'
    token.tag     = is_strong ? 'strong' : 'em'
    token.nesting = -1
    token.markup  = is_strong ? ch + ch : ch
    token.content = ''

    if is_strong {
      state.tokens[delimiters[i - 1].token].content = ''
      state.tokens[delimiters[start_delim.end + 1].token].content = ''
      i--
    }
  }
}

def post_process(state) {
  var curr,
      tokens_meta = state.tokens_meta,
      max = state.tokens_meta.length()

  _post_process(state, state.delimiters)

  iter curr = 0; curr < max; curr++ {
    if tokens_meta[curr] and tokens_meta[curr].delimiters {
      _post_process(state, tokens_meta[curr].delimiters)
    }
  }
}
