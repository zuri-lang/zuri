# Process auto links '<protocol:...>'

var EMAIL_RE    = '/^([a-zA-Z0-9.!#$%&\'*+\/=?^_`{|}~-]+@[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(?:\.[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*)$/'
var AUTOLINK_RE = '/^([a-zA-Z][a-zA-Z0-9+.\-]{1,31}):([^<>\\x00-\\x20]*)$/'


def autolink(state, silent) {
  var url, full_url, token, ch, start, max,
      pos = state.pos

  if state.src[pos] != '<' return false

  start = state.pos
  max = state.pos_max

  iter ;; {
    if pos++ >= max return false

    ch = state.src[pos]

    if ch == '<' return false
    if ch == '>' break
  }

  url = state.src[start + 1, pos]

  if url.match(AUTOLINK_RE) {
    full_url = state.md.normalize_link(url)
    if !state.md.validate_link(full_url) return false

    if !silent {
      token         = state.push('link_open', 'a', 1)
      token.attrs   = [ [ 'href', full_url ] ]
      token.markup  = 'autolink'
      token.info    = 'auto'

      token         = state.push('text', '', 0)
      token.content = state.md.normalize_link_text(url)

      token         = state.push('link_close', 'a', -1)
      token.markup  = 'autolink'
      token.info    = 'auto'
    }

    state.pos += url.length() + 2
    return true
  }

  if url.match(EMAIL_RE) {
    full_url = state.md.normalize_link('mailto:' + url)
    if !state.md.validate_link(full_url) return false

    if !silent {
      token         = state.push('link_open', 'a', 1)
      token.attrs   = [ [ 'href', full_url ] ]
      token.markup  = 'autolink'
      token.info    = 'auto'

      token         = state.push('text', '', 0)
      token.content = state.md.normalize_link_text(url)

      token         = state.push('link_close', 'a', -1)
      token.markup  = 'autolink'
      token.info    = 'auto'
    }

    state.pos += url.length() + 2
    return true
  }

  return false
}

