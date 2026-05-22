#!-- part of the html module

def feed_position(position, str, len) {
  var start = position.index
  var end = position.index = start + len
  iter var i = start; i < end; i++ {
    var char = str[i]
    if char == '\n' {
      position.line++
      position.column = 1
    } else {
      position.column++
    }
  }
}

def jump_position(position, str, end) {
  var len = end - position.index
  return feed_position(position, str, len)
}

def lexer(str, options) {
  var state = {
    str,
    options,
    position: {
      index: 0,
      line: 1,
      column: 1,
    },
    tokens: []
  }
  lex(state)
  return state.tokens
}

def lex(state) {
  var len = state.str.length()
  while state.position.index < len {
    var start = state.position.index
    lex_text(state)
    if state.position.index == start {
      var is_comment = state.str.index_of('!--', start + 1) > -1
      if is_comment {
        lex_comment(state)
      } else {
        var name = lex_tag(state)
        if state.get('childless_tags', []).contains(name.lower()) {
          lex_skip_tag(name, state)
        }
      }
    }
  }
}

var alphanumeric = '/[A-_za-z0-9]/'

def find_text_end(str, index) {
  while true {
    var text_end = str.index_of('<', index)
    if text_end == -1 {
      return text_end
    }
    var char = str[text_end + 1]
    if char == '/' or char == '!' or char.match(alphanumeric) {
      return text_end
    }
    index = text_end + 1
  }
}

def lex_text(state) {
  var type = 'text'
  var text_end = find_text_end(state.str, state.position.index)
  if text_end == state.position.index return
  if text_end == -1 {
    text_end = state.str.length()
  }

  var start = state.position.clone()
  var content = state.str[state.position.index, text_end]
  jump_position(state.position, state.str, text_end)
  var end = state.position.clone()
  state.tokens.append({
    type, 
    content, 
    position: {
      start, 
      end,
    }
  })
}

def lex_comment(state) {
  var start = state.position.clone()
  feed_position(state.position, state.str, 4) # "<!--".length()

  var content_end = state.str.index_of('-->', state.position.index)
  var comment_end = content_end + 3 # "-->".length()
  if content_end == -1 {
    content_end = comment_end = state.str.length()
  }

  var content = state.str[state.position.index, content_end]
  jump_position(state.position, state.str, comment_end)
  state.tokens.append({
    type: 'comment',
    content,
    position: {
      start,
      end: state.position.clone()
    }
  })
}

def lex_tag(state) {
  {
    var second_char = state.str[state.position.index + 1]
    var close = second_char == '/'
    var start = state.position.clone()
    feed_position(state.position, state.str, close ? 2 : 1)
    state.tokens.append({
      type: 'tag-start', 
      close, 
      position: {
        start,
      }
    })
  }

  var name = lex_tag_name(state)
  lex_tag_attributes(state)

  {
    var first_char = state.str[state.position.index]
    var close = first_char == '/'
    feed_position(state.position, state.str, close ? 2 : 1)
    var end = state.position.clone()
    state.tokens.append({
      type: 'tag-end', 
      close, 
      position: {
        end,
      }
    })
  }

  return name
}

# _see https:#developer.mozilla.org/en-US/docs/_web/_java_script/_guide/_regular__expressions#special-white-space
var whitespace = '/\s/'

def is_whitespace_char(char) {
  return char.match(whitespace)
}

def lex_tag_name(state) {
  var len = state.str.length()
  var start = state.position.index
  while start < len {
    var char = state.str[start]
    var is_tag_char = !(is_whitespace_char(char) or char == '/' or char == '>')
    if is_tag_char break
    start++
  }

  var end = start + 1
  while end < len {
    var char = state.str[end]
    var is_tag_char = !(is_whitespace_char(char) or char == '/' or char == '>')
    if !is_tag_char break
    end++
  }

  jump_position(state.position, state.str, end)
  var name = state.str[start, end]
  state.tokens.append({
    type: 'tag',
    content: name
  })
  return name
}

def lex_tag_attributes(state) {
  var cursor = state.position.index
  var quote = nil # nil, single-, or double-quote
  var word_begin = cursor # index of word start
  var words = [] # "key", "key=value", "key='value'", etc
  var len = state.str.length()
  while cursor < len {
    var char = state.str[cursor]
    if quote {
      var is_quote_end = char == quote
      if is_quote_end {
        quote = nil
      }
      cursor++
      continue
    }

    var is_tag_end = char == '/' or char == '>'
    if is_tag_end {
      if cursor != word_begin {
        words.append(state.str[word_begin, cursor])
      }
      break
    }

    var is_word_end = is_whitespace_char(char)
    if is_word_end {
      if cursor != word_begin {
        words.append(state.str[word_begin, cursor])
      }
      word_begin = cursor + 1
      cursor++
      continue
    }

    var is_quote_start = char == '\'' or char == '"'
    if is_quote_start {
      quote = char
      cursor++
      continue
    }

    cursor++
  }
  jump_position(state.position, state.str, cursor)

  var w_len = words.length()
  var type = 'attribute'
  iter var i = 0; i < w_len; i++ {
    var word = words[i]
    var is_not_pair = word.index_of('=') == -1
    if is_not_pair and words.length() > i + 1 {
      var second_word = words[i + 1]
      if second_word and second_word.index_of('=') > -1 {
        if second_word.length() > 1 {
          var new_word = word + second_word
          state.tokens.append({type, content: new_word})
          i += 1
          continue
        }
        var third_word = words[i + 2]
        i += 1
        if third_word {
          var new_word = word + '=' + third_word
          state.tokens.append({type, content: new_word})
          i += 1
          continue
        }
      }
    }
    if word.ends_with('=') {
      var second_word = words[i + 1]
      if second_word and second_word.index_of('=') > -1 {
        var new_word = word + second_word
        state.tokens.append({type, content: new_word})
        i += 1
        continue
      }

      var new_word = word[0, -1]
      state.tokens.append({type, content: new_word})
      continue
    }

    state.tokens.append({type, content: word})
  }
}

def lex_skip_tag(name, state) {
  var safe_tag_name = name.lower()
  var len = state.str.length()
  var index = state.position.index
  while index < len {
    var next_tag = state.str.index_of('</', index)
    if next_tag == -1 {
      lex_text(state)
      break
    }

    var tag_start_position = state.position.clone()
    jump_position(tag_start_position, state.str, next_tag)
    var tag_state = {
      str: state.str, 
      position: tag_start_position, 
      tokens: [],
    }
    var name = lex_tag(tag_state)
    if safe_tag_name != name.lower() {
      index = tag_state.position.index
      continue
    }

    if next_tag != state.position.index {
      var text_start = state.position.clone()
      jump_position(state.position, state.str, next_tag)
      state.tokens.append({
        type: 'text',
        content: state.str[text_start.index, next_tag],
        position: {
          start: text_start,
          end: state.position.clone(),
        }
      })
    }

    state.tokens.extend(tag_state.tokens)
    # state.tokens.append(tag_state.tokens)
    jump_position(state.position, state.str, tag_state.position.index)
    break
  }
}
