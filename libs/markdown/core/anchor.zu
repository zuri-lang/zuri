import url

def slugify(s) {
  return url.encode(to_string(s).trim().case_fold().replace('/[^a-zA-Z0-9_\- ]/', '').replace(' ', '-'))
}

var tokens_filter = ['text', 'code_inline']

def get_tokens_text(tokens) {
  return ''.join(
    tokens.
      filter(@(t) {
        return tokens_filter.contains(t.type)
      }).map(@(t) {
        return t.content
      })
  )
}
  
def unique_slug(slug, slugs) {
  var uniq = slug
  var i = 1

  while slugs.contains(uniq) {
    uniq = '${slug}-${i}'
    i++
  }

  slugs[uniq] = true
  return uniq
}

var is_level_selected_number = @(selection) { return @(level) { return level >= selection } }

def anchor(state) {
  var slugs = {}
  var tokens = state.tokens

  iter var idx = 0; idx < tokens.length(); idx++ {
    var token = tokens[idx]

    if token.type != 'heading_open' {
      continue
    }

    if to_number(token.tag[1,]) < 1 {
      continue
    }

    # Aggregate the next token children text.
    var title = get_tokens_text(tokens[idx + 1].children)

    var slug = token.attr_get('id')

    if slug == nil {
      slug = unique_slug(slugify(title), slugs)
    } else {
      slug = unique_slug(slug, slugs)
    }

    token.attr_set('id', slug)

    # A permalink renderer could modify the `tokens` array so
    # make sure to get the up-to-date index on each iteration.
    idx = tokens.index_of(token)
  }
}

