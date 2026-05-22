# Process links like https://example.org/

import ..common.html_re { LINKS_RE }

# RFC3986: scheme = ALPHA *( ALPHA / DIGIT / "+" / "-" / "." )
var SCHEME_RE = '/(?:^|[^a-z0-9.+-])([a-z][a-z0-9.+-]*)$/i'


def linkify(state, silent) {
  var pos, max, match, proto, link, url, full_url, token

  if !state.md.options.linkify return false
  if state.link_level > 0 return false

  pos = state.pos
  max = state.pos_max

  if pos + 3 > max return false
  if state.src[pos] != ':' return false
  if state.src[pos + 1] != '/' return false
  if state.src[pos + 2] != '/' return false

  match = state.pending.match(SCHEME_RE)
  if !match return false

  proto = match[1]

  link = state.src[pos - proto.length(),].match(LINKS_RE)
  if !link return false

  url = link[0]

  # disallow '*' at the end of the link (conflicts with emphasis)
  url = url.replace('/\*+$/', '')

  full_url = state.md.normalize_link(url)
  if !state.md.validate_link(full_url) return false

  if (!silent) {
    state.pending = state.pending[,-proto.length()]

    token         = state.push('link_open', 'a', 1)
    token.attrs   = [ [ 'href', full_url ] ]
    token.markup  = 'linkify'
    token.info    = 'auto'

    token         = state.push('text', '', 0)
    token.content = state.md.normalize_link_text(url)

    token         = state.push('link_close', 'a', -1)
    token.markup  = 'linkify'
    token.info    = 'auto'
  }

  state.pos += url.length() - proto.length()
  return true
}

