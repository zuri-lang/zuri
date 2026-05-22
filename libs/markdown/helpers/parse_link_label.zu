# Parse link label
#
# this function assumes that first character ("[") already matches;
# returns the end of the label

def parse_link_label(state, start, disable_nested) {
  var level, found = false, marker, prev_pos,
      label_end = -1,
      max = state.pos_max,
      old_pos = state.pos

  state.pos = start + 1
  level = 1

  while state.pos < max {
    marker = state.src[state.pos]
    if marker == ']' {
      level--
      if level == 0 {
        found = true
        break
      }
    }

    prev_pos = state.pos
    state.md.inline.skip_token(state)
    if marker == '[' {
      if prev_pos == state.pos - 1 {
        # increase level if we find text `[`, which is not a part of any token
        level++
      } else if disable_nested {
        state.pos = old_pos
        return -1
      }
    }
  }

  if found {
    label_end = state.pos
  }

  # restore old state
  state.pos = old_pos

  return label_end
}

