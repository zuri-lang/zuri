# Convert straight quotation marks to typographic ones

import ..common.utils { is_white_space, is_punct_char, is_md_ascii_punct }

var QUOTE_RE = '/[\'"]/'
var APOSTROPHE = '\u2019' /* ’ */

def _replace_at(str, index, ch) {
  return str[,index] + ch + str[index + 1,]
}

def _process_inlines(tokens, state) {
  var i, token, text, t, pos, max, this_level, item, last_char, nextChar,
      is_last_punct_char, is_nextPunct_char, is_last_white_space, is_nextWhite_space,
      can_open, can_close, j, is_single, stack, open_quote, close_quote

  stack = []

  iter i = 0; i < tokens.length(); i++ {
    token = tokens[i]

    this_level = tokens[i].level

    iter j = stack.length() - 1; j >= 0; j-- {
      if stack[j].level <= this_level break
    }
    while stack.length() > j + 1 stack.pop()
    # stack.length = j + 1

    if token.type != 'text' continue

    text = token.content
    pos = 0
    max = text.length()

    while pos < max {
      t = text[pos,].match(QUOTE_RE)
      if !t break

      var t_index = text.index_of(t[0], pos)
      t = t[0]

      can_open = can_close = true
      pos = t_index + 1
      is_single = t[0] == "'"

      # Find previous character,
      # default to space if it's the beginning of the line
      last_char = ' '

      if t_index - 1 >= 0 {
        last_char = text[t_index - 1]
      } else {
        iter j = i - 1; j >= 0; j-- {
          if tokens[j].type == 'softbreak' or tokens[j].type == 'hardbreak' break # last_char defaults to 0x20
          if !tokens[j].content continue # should skip all tokens except 'text', 'html_inline' or 'code_inline'

          last_char = tokens[j].content[tokens[j].content.length() - 1]
          break
        }
      }

      # Find next character,
      # default to space if it's the end of the line
      nextChar = ' '

      if pos < max {
        nextChar = text[pos]
      } else {
        iter j = i + 1; j < tokens.length(); j++ {
          if tokens[j].type == 'softbreak' or tokens[j].type == 'hardbreak' break # nextChar defaults to 0x20
          if !tokens[j].content continue # should skip all tokens except 'text', 'html_inline' or 'code_inline'

          nextChar = tokens[j].content[0]
          break
        }
      }

      is_last_punct_char = is_md_ascii_punct(last_char) or is_punct_char(last_char)
      is_nextPunct_char = is_md_ascii_punct(nextChar) or is_punct_char(nextChar)

      is_last_white_space = is_white_space(last_char)
      is_nextWhite_space = is_white_space(nextChar)

      if is_nextWhite_space {
        can_open = false
      } else if is_nextPunct_char {
        if !(is_last_white_space or is_last_punct_char) {
          can_open = false
        }
      }

      if is_last_white_space {
        can_close = false
      } else if is_last_punct_char {
        if !(is_nextWhite_space or is_nextPunct_char) {
          can_close = false
        }
      }

      if nextChar == '"' and t[0] == '"' {
        if ord(last_char) >= 0x30 /* 0 */ and ord(last_char) <= 0x39 /* 9 */ {
          # special case: 1"" - count first quote as an inch
          can_close = can_open = false
        }
      }

      if can_open and can_close {
        # Replace quotes in the middle of punctuation sequence, but not
        # in the middle of the words, i.e.:
        #
        # 1. foo " bar " baz - not replaced
        # 2. foo-"-bar-"-baz - replaced
        # 3. foo"bar"baz     - not replaced
        can_open = is_last_punct_char
        can_close = is_nextPunct_char
      }

      if !can_open and !can_close {
        # middle of word
        if is_single {
          token.content = _replace_at(token.content, t_index, APOSTROPHE)
        }
        continue
      }

      if can_close {
        # this could be a closing quote, rewind the stack to get a match
        var continue_outer = false
        iter j = stack.length() - 1; j >= 0; j-- {
          item = stack[j]
          if stack[j].level < this_level break
          if item.single == is_single and stack[j].level == this_level {
            item = stack[j]

            if is_single {
              open_quote = state.md.options.quotes[2]
              close_quote = state.md.options.quotes[3]
            } else {
              open_quote = state.md.options.quotes[0]
              close_quote = state.md.options.quotes[1]
            }

            # replace token.content *before* tokens[item.token].content,
            # because, if they are pointing at the same token, replace_at
            # could mess up indices when quote length != 1
            token.content = _replace_at(token.content, t_index, close_quote)
            tokens[item.token].content = _replace_at(
              tokens[item.token].content, item.pos, open_quote)

            pos += close_quote.length() - 1
            if item.token == i pos += open_quote.length() - 1

            text = token.content
            max = text.length()

            while stack.length() > j stack.pop()
            # stack.length = j

            continue_outer = true
            break
          }
        }

        if continue_outer continue
      }

      if can_open {
        stack.append({
          token: i,
          pos: t_index,
          single: is_single,
          level: this_level,
        })
      } else if can_close and is_single {
        token.content = _replace_at(token.content, t_index, APOSTROPHE)
      }
    }
  }
}

def smartquotes(state) {
  if !state.md.options.typographer return

  iter var blk_idx = state.tokens.length() - 1; blk_idx >= 0; blk_idx-- {

    if state.tokens[blk_idx].type != 'inline' or
        !state.tokens[blk_idx].content.match(QUOTE_RE) {
      continue
    }

    _process_inlines(state.tokens[blk_idx].children, state)
  }
}

