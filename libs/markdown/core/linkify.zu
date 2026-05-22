# Replace link-like texts with link nodes.

import ..common.utils { array_replace_at }
import ..common.html_re { LINKS_RE }
import url as _url

def _is_link_open(str) {
  return str.match('/^<a[>\s]/i')
}
def _is_link_close(str) {
  return str.match('/^<\/a\s*>/i')
}

def linkify(state) {
  var i, j = 0, l, tokens, token, current_token, nodes, ln, text, pos, last_pos,
      level, html_link_level, url, full_url, url_text,
      block_tokens = state.tokens,
      links

  if !state.md.options.linkify return

  iter l = block_tokens.length(); j < l; j++ {
    if block_tokens[j].type != 'inline' or !block_tokens[j].content.match(LINKS_RE) {
      continue
    }

    tokens = block_tokens[j].children

    html_link_level = 0

    # We scan from the end, to keep position when new tags added.
    # Use reversed logic in links start/end match
    iter i = tokens.length() - 1; i >= 0; i-- {
      current_token = tokens[i]

      # Skip content of markdown links
      if current_token.type == 'link_close' {
        i--
        while tokens[i].level != current_token.level and tokens[i].type != 'link_open' {
          i--
        }
        continue
      }

      # Skip content of html tag links
      if current_token.type == 'html_inline' {
        if _is_link_open(current_token.content) and html_link_level > 0 {
          html_link_level--
        }
        if _is_link_close(current_token.content) {
          html_link_level++
        }
      }
      if html_link_level > 0 continue

      if current_token.type == 'text' and current_token.content.match(LINKS_RE) {

        text = current_token.content

        var links_matched = text.matches(LINKS_RE)
        if links_matched {
          links = []
          for link in links_matched[0] {
            links.append(_url.parse(link))
          }
        }

        # Now split string to nodes
        nodes = []
        level = current_token.level
        last_pos = 0

        # forbid escape sequence at the start of the string,
        # this avoids http\://example.com/ from being linkified as
        # http:<a href="//example.com/">//example.com/</a>
        if links.length() > 0 and links[0].index == 0 and i > 0 and 
          tokens[i - 1].type == 'text_special' {
          links = links[1,]
        }

        iter ln = 0; ln < links.length(); ln++ {
          url = links[ln].url
          full_url = state.md.normalize_link(url)
          if !state.md.validate_link(full_url) continue

          url_text = links[ln].to_string()

          # Linkifier might send raw hostnames like "example.com", where url
          # starts with domain name. So we prepend http:// in those cases,
          # and remove it afterwards.
          if !links[ln].schema {
            url_text = state.md.normalize_link_text('http://' + url_text).replace('/^http:\/\//', '')
          } else if links[ln].schema == 'mailto' and !url_text.match('/^mailto:/i') {
            url_text = state.md.normalize_link_text('mailto:' + url_text).replace('/^mailto:/', '')
          } else {
            url_text = state.md.normalize_link_text(url_text)
          }

          pos = links[ln].index

          if pos > last_pos {
            token         = state.Token('text', '', 0)
            token.content = text[last_pos, pos]
            token.level   = level
            nodes.append(token)
          }

          token         = state.Token('link_open', 'a', 1)
          token.attrs   = [ [ 'href', full_url ] ]
          token.level   = level++
          token.markup  = 'linkify'
          token.info    = 'auto'
          nodes.append(token)

          token         = state.Token('text', '', 0)
          token.content = url_text
          token.level   = level
          nodes.append(token)

          token         = state.Token('link_close', 'a', -1)
          token.level   = level--
          token.markup  = 'linkify'
          token.info    = 'auto'
          nodes.append(token)

          last_pos = links[ln].last_index;
        }
        if last_pos < text.length() {
          token         = state.Token('text', '', 0)
          token.content = text[last_pos,]
          token.level   = level
          nodes.append(token)
        }

        # replace current node
        block_tokens[j].children = tokens = array_replace_at(tokens, i, nodes)
      }
    }
  }
}

