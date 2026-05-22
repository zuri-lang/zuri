# Process '\n'

import ..common.utils { is_space }

def newline(state, silent) {
  var pmax, max, ws, pos = state.pos

  if state.src[pos] != '\n' return false

  pmax = state.pending.length() - 1
  max = state.pos_max

  # '  \n' -> hardbreak
  # Lookup in pending chars is bad practice! Don't copy to other rules!
  # Pending string is stored in concat mode, indexed lookups will cause
  # convertion to flat mode.
  if !silent {
    if pmax >= 0 and state.pending[pmax] == ' ' {
      if pmax >= 1 and state.pending[pmax - 1] == ' ' {
        # Find whitespaces tail of pending chars.
        ws = pmax - 1
        while ws >= 1 and state.pending[ws - 1] == ' ' ws--

        state.pending = state.pending[,ws]
        state.push('hardbreak', 'br', 0)
      } else {
        state.pending = state.pending[,-1]
        state.push('softbreak', 'br', 0)
      }

    } else {
      state.push('softbreak', 'br', 0)
    }
  }

  pos++

  # skip heading spaces for next line
  while pos < max and is_space(state.src[pos]) pos++

  state.pos = pos
  return true
}

