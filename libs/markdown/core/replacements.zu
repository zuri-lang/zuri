# Simple typographic replacements
#
# (c) (C) → ©
# (tm) (TM) → ™
# (r) (R) → ®
# +- → ±
# ... → … (also ?.... → ?.., !.... → !..)
# ???????? → ???, !!!!! → !!!, `,,` → `,`
# -- → &ndash;, --- → &mdash;

var RARE_RE = '/\+-|\.\.|\?\?\?\?|!!!!|,,|--/'

var SCOPED_ABBR_RE = '/\((c|tm|r)\)/i'
var SCOPED_ABBR = {
  c: '©',
  r: '®',
  tm: '™'
}

def _replace_scoped(inline_tokens) {
  var i, token, inside_autolink = 0

  iter i = inline_tokens.length() - 1; i >= 0; i-- {
    token = inline_tokens[i]

    if token.type == 'text' and !inside_autolink {
      var matches = token.content.matches(SCOPED_ABBR_RE)
      if matches {
        iter var i = 0; i < matches[0].length(); i++ {
          token.content = token.content.replace(matches[0][i], matches[1][i], false)
        }
      }
    }

    if token.type == 'link_open' and token.info == 'auto' {
      inside_autolink--
    }

    if token.type == 'link_close' and token.info == 'auto' {
      inside_autolink++
    }
  }
}

def _replace_rare(inline_tokens) {
  var i, token, inside_autolink = 0

  iter i = inline_tokens.length() - 1; i >= 0; i-- {
    token = inline_tokens[i]

    if token.type == 'text' and !inside_autolink {
      if token.content.match(RARE_RE) {
        token.content = token.content.
          replace('/\+-/', '±').
          # .., ..., ....... -> …
          # but ?..... & !..... -> ?.. & !..
          replace('/\.{2,}/', '…').replace('/([?!])…/', '$1..').
          replace('/([?!]){4,}/', '$1$1$1').replace('/,{2,}/', ',').
          # em-dash
          replace('/(^|[^-])---(?=[^-]|$)/m', '$1\u2014').
          # en-dash
          replace('/(^|\s)--(?=\s|$)/m', '$1\u2013').
          replace('/(^|[^-\s])--(?=[^-\s]|$)/m', '$1\u2013')
      }
    }

    if token.type == 'link_open' and token.info == 'auto' {
      inside_autolink--
    }

    if token.type == 'link_close' and token.info == 'auto' {
      inside_autolink++
    }
  }
}

def replacements(state) {
  var blk_idx

  if !state.md.options.typographer return

  iter blk_idx = state.tokens.length() - 1; blk_idx >= 0; blk_idx-- {

    if (state.tokens[blk_idx].type != 'inline') {
      continue
    }

    if state.tokens[blk_idx].content.match(SCOPED_ABBR_RE) {
      _replace_scoped(state.tokens[blk_idx].children)
    }

    if state.tokens[blk_idx].content.match(RARE_RE) {
      _replace_rare(state.tokens[blk_idx].children)
    }
  }
}
