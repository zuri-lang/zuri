# Process HTML tags

import ..common.html_re { HTML_TAG_RE }

def _is_link_open(str) {
  return str.match('/^<a[>\s]/i')
}

def _is_link_close(str) {
  return str.match('/^<\/a\s*>/i')
}

def _is_letter(ch) {
  var lc = ch | 0x20 # to lower case
  return (lc >= 0x61) and (lc <= 0x7a) # a and z
}

def html_inline(state, silent) {
  var ch, match, max, token,
      pos = state.pos

  if !state.md.options.html return false

  # Check start
  max = state.pos_max
  if state.src[pos] != '<' or pos + 2 >= max {
    return false
  }

  # Quick fail on second char
  ch = state.src[pos + 1]
  if ch != '!' and ch != '?' and ch != '/' and !_is_letter(ord(ch)) {
    return false
  }

  match = state.src[pos,].match(HTML_TAG_RE)
  if !match return false

  if !silent {
    token         = state.push('html_inline', '', 0)
    token.content = match[0]

    if _is_link_open(token.content)  state.link_level++
    if _is_link_close(token.content) state.link_level--
  }
  state.pos += match[0].length()
  return true
}

