# Join raw text tokens with the rest of the text
#
# This is set as a separate rule to provide an opportunity for plugins
# to run text replacements after text join, but before escape join.
#
# For example, `\:)` shouldn't be replaced with an emoji.

def text_join(state) {
  var j = 0, l, tokens, curr, max, last,
      block_tokens = state.tokens

  iter l = block_tokens.length(); j < l; j++ {
    if block_tokens[j].type != 'inline' continue

    tokens = block_tokens[j].children
    max = tokens.length()

    iter curr = 0; curr < max; curr++ {
      if tokens[curr].type == 'text_special' {
        tokens[curr].type = 'text'
      }
    }

    iter curr = last = 0; curr < max; curr++ {
      if tokens[curr].type == 'text' and
          curr + 1 < max and
          tokens[curr + 1].type == 'text' {

        # collapse two adjacent text nodes
        tokens[curr + 1].content = tokens[curr].content + tokens[curr + 1].content
      } else {
        if curr != last tokens[last] = tokens[curr]

        last++
      }
    }

    if curr != last {
      while curr > last {
        tokens.pop()
        curr--
      }
      # tokens.length = last
    }
  }
}

