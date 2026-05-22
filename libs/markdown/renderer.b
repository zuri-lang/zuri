import .common.utils { assign, unescape_all, escape_html }

var default_rules = {}

default_rules.code_inline = @(tokens, idx, options, env, slf) {
  var token = tokens[idx]

  return  '<code' + slf.render_attrs(token) + '>' +
          escape_html(token.content) +
          '</code>'
}

default_rules.code_block = @(tokens, idx, options, env, slf) {
  var token = tokens[idx]

  return  '<pre' + slf.render_attrs(token) + '><code>' +
          escape_html(token.content) +
          '</code></pre>\n'
}

default_rules.fence = @(tokens, idx, options, env, slf) {
  var token = tokens[idx],
      info = token.info ? unescape_all(token.info).trim() : '',
      lang_name = '',
      lang_attrs = '',
      highlighted, i, arr, tmp_attrs, tmp_token

  if info {
    arr = info.split('/(\s+)/')
    lang_name = arr[0]
    lang_attrs = ''.join(arr[2,])
  }

  if options.highlight {
    highlighted = options.highlight(token.content, lang_name) or escape_html(token.content)
  } else {
    highlighted = escape_html(token.content)
  }

  if highlighted.index_of('<pre') == 0 {
    return highlighted + '\n'
  }

  # If language exists, inject class gently, without modifying original token.
  # May be, one day we will add .deep_clone() for token and simplify this part, but
  # now we prefer to keep things local.
  if info {
    i        = token.attr_index('class')
    tmp_attrs = token.attrs ? token.attrs[,] : []

    if i < 0 {
      tmp_attrs.append([ 'class', options.lang_prefix + lang_name ])
    } else {
      tmp_attrs[i] = tmp_attrs[i][,]
      tmp_attrs[i][1] += ' ' + options.lang_prefix + lang_name
    }

    # Fake token just to render attributes
    tmp_token = {
      attrs: tmp_attrs
    }

    return  '<pre><code' + slf.render_attrs(tmp_token) + '>' +
            highlighted +
            '</code></pre>\n'
  }

  return  '<pre><code' + slf.render_attrs(token) + '>' +
          highlighted +
          '</code></pre>\n'
}

default_rules.image = @(tokens, idx, options, env, slf) {
  var token = tokens[idx]

  # "alt" attr MUST be set, even if empty. Because it's mandatory and
  # should be placed on proper position for tests.
  #
  # Replace content with actual value

  token.attrs[token.attr_index('alt')][1] =
    slf.render_inline_as_text(token.children, options, env)

  return slf.render_token(tokens, idx, options)
}

default_rules.hardbreak = @(tokens, idx, options , env, slf) {
  return options.xhtml_out ? '<br />\n' : '<br>\n'
}
default_rules.softbreak = @(tokens, idx, options , env, slf) {
  return options.breaks ? (options.xhtml_out ? '<br />\n' : '<br>\n') : '\n'
}

default_rules.text = @(tokens, idx , options, env, slf) {
  return escape_html(tokens[idx].content)
}


default_rules.html_block = @(tokens, idx , options, env, slf) {
  return tokens[idx].content
}
default_rules.html_inline = @(tokens, idx , options, env, slf) {
  return tokens[idx].content
}

default_rules.emoji = @(tokens, idx , options, env, slf) {
  return tokens[idx].content
}

/**
 * Generates HTML from parsed token stream. Each instance has independent
 * copy of rules. Those can be rewritten with ease. Also, you can add new
 * rules if you create plugin and adds new token types.
 */
class Renderer {

  /**
   * Contains render rules for tokens. Can be updated and extended.
   *
   * ##### Example
   *
   * ```zuri
   * import markdown as md
   *
   * md.renderer.rules.strong_open  = @{ return '<b>' }
   * md.renderer.rules.strong_close = @{ return '</b>' }
   *
   * var result = md.render_inline(...)
   * ```
   *
   * Each rule is called as independent static function with fixed signature:
   *
   * ```zuri
   * def my_token_render(tokens, idx, options, env, renderer) {
   *   # ...
   *   return rendered_hTML
   * }
   * ```
   * 
   * @type dict
   */
  var rules = assign({}, default_rules)

  /**
   * Render token attributes to string.
   * 
   * @param Token token
   * @returns string
   */
  render_attrs(token) {
    var i = 0, l, result
  
    if !token.attrs return ''
  
    result = ''
  
    iter l = token.attrs.length(); i < l; i++ {
      result += ' ' + escape_html(to_string(token.attrs[i][0])) + '="' + escape_html(to_string(token.attrs[i][1])) + '"'
    }
  
    return result
  }

  /**
   * Default token renderer. Can be overriden by custom function
   * in [[markdown.Renderer.rules]].
   * 
   * @param list tokens: list of tokens
   * @param number idx: token index to render
   * @param dict options: params of parser instance
   * @returns string
   */
  render_token(tokens, idx, options) {
    var nextToken,
        result = '',
        need_lf = false,
        token = tokens[idx]
  
    # Tight list paragraphs
    if token.hidden {
      return ''
    }
  
    # Insert a newline between hidden paragraph and subsequent opening
    # block-level tag.
    #
    # For example, here we should insert a newline before blockquote:
    #  - a
    #    >
    if token.block and token.nesting != -1 and idx and tokens[idx - 1].hidden {
      result += '\n'
    }
  
    # Add token name, e.g. `<img`
    result += (token.nesting == -1 ? '</' : '<') + token.tag
  
    # Encode attributes, e.g. `<img src="foo"`
    result += self.render_attrs(token)
  
    # Add a slash for self-closing tags, e.g. `<img src="foo" /`
    if token.nesting == 0 and options.xhtml_out {
      result += ' /'
    }
  
    # Check if we need to add a newline after this tag
    if token.block {
      need_lf = true
  
      if token.nesting == 1 {
        if idx + 1 < tokens.length() {
          nextToken = tokens[idx + 1]
  
          if nextToken.type == 'inline' or nextToken.hidden {
            # Block-level tag containing an inline tag.
            #
            need_lf = false
  
          } else if nextToken.nesting == -1 and nextToken.tag == token.tag {
            # Opening tag + closing tag of the same type. E.g. `<li></li>`.
            #
            need_lf = false
          }
        }
      }
    }
  
    result += need_lf ? '>\n' : '>'
  
    return result
  }

  /**
   * The same as [[markdown.Renderer.render]], but for single token of `inline` type.
   * 
   * @param list tokens: list on block tokens to render
   * @param dict options: params of parser instance
   * @param dict env: additional data from parsed input (references, for example)
   * @returns string
   */
  render_inline(tokens, options, env) {
    var type,
        result = '',
        rules = self.rules,
        i = 0
  
    iter var len = tokens.length(); i < len; i++ {
      type = tokens[i].type
  
      if rules.contains(type) {
        result += rules[type](tokens, i, options, env, self)
      } else {
        result += self.render_token(tokens, i, options)
      }
    }
  
    return result
  }

  /**
   * Special kludge for image `alt` attributes to conform CommonMark spec.
   * Don't try to use it! Spec requires to show `alt` content with stripped markup,
   * instead of simple escaping.
   * 
   * @param list tokens: list on block tokens to render
   * @param dict options: params of parser instance
   * @param dict env: additional data from parsed input (references, for example)
   * @returns string
   * @internal
   */
  render_inline_as_text(tokens, options, env) {
    var result = '', i = 0
  
    iter var len = tokens.length(); i < len; i++ {
      if tokens[i].type == 'text' {
        result += tokens[i].content
      } else if tokens[i].type == 'image' {
        result += self.render_inline_as_text(tokens[i].children, options, env)
      } else if tokens[i].type == 'softbreak' {
        result += '\n'
      }
    }
  
    return result
  }

  /**
   * Takes token stream and generates HTML. Probably, you will never need to call
   * this method directly.
   * 
   * @param list tokens: list on block tokens to render
   * @param dict options: params of parser instance
   * @param dict env: additional data from parsed input (references, for example)
   * @returns string
   **/
  render(tokens, options, env) {
    var i = 0, len, type,
        result = '',
        rules = self.rules
  
    iter len = tokens.length(); i < len; i++ {
      type = tokens[i].type
  
      if type == 'inline' {
        result += self.render_inline(tokens[i].children, options, env)
      } else if rules.contains(type) {
        result += rules[type](tokens, i, options, env, self)
      } else {
        result += self.render_token(tokens, i, options)
      }
    }
  
    return result
  }
}

