var default_classes = {
  string: 's',
  interpolation: 'i',
  constant: 'x',
  method: 'm',
  function: 'f',
  keyword: 'k',
  comment: 'c',
  operator: 'o',
  number: 'n',
  prompt: 'p',
  result: 'r',
  error: 'e',
}

var zuri_keywords = '|'.join([
  'as', 'assert', 'break', 'catch', 'class', 'continue',
  'def', 'default', 'do', 'echo', 'else', 'for', 'if',
  'import', 'in', 'iter', 'raise', 'return', 'static',
  'using', 'var', 'when', 'while',
])

var constant_keywords = '|'.join([
  'nil', 'parent', 'self', 'true', 'false', '__args__', '__file__', '__root__'
])

var _quote_re = '/((?<![a-z])(\'(?:[^\'\\\\]|\\\\.)*\')|("(?:[^"\\\\]|\\\\.)*"))/m'

def highlight_zuri(text, classes) {
  text = text.
    replace('<', '&lt;').replace('>', '&gt;').
    # operators
    replace('/([+\-=%!<>@~\^]|(?<!\.)\.\.(?!\.)|(?<!\*)\/(?!\*)|(?<!\/)\*(?!\/))/', '<_o>$1</_o>').
    replace('/\\b(and|or)\\b/', '<_o>$1</_o>').
    # quotes
    replace(_quote_re, '<_q>$1</_q>').
    # constant keywords
    replace('/\\b(${constant_keywords})\\b/', '<_c>$1</_c>').

    # numbers
    replace('/\\b(([0-9][0-9]*\.[0-9]+([eE][0-9]+)?[fd]?)|(0x[0-9a-fA-F]+)|(0c[0-7][0-7]*)|(0b[01][01]*)|([0-9]+))\\b/', '<_n>$1</_n>').

    # functions
    # property/method call and access
    replace('/(?<=\.)[ ]*([a-zA-Z_][a-zA-Z0-9_]*)[ ]*(?=[(])/', '<_m>$1</_m>').
    # definition and call
    replace('/(?<!\.)([a-zA-Z_][a-zA-Z0-9_]*)[ ]*(?=[(])/', '<_f>$1</_f>').

    # keywords
    replace('/\\b(${zuri_keywords})\\b/', '<_k>$1</_k>').
    # comments
    replace('/(#(?=[^"\']*(?:"[^"]*"[^"]*|\'[^\']*\'[^\']*)*$)[^\n]*)/', '<_w>$1</_w>'). # line comment
    replace('/(\/\*(?:(?!\/\*|\*\/).|(?R))*\*\/)/ms', '<_w1>$1</_w1>') # block comment

  # clean up comments
  var comments = text.matches('/<_w>((.|\\n)*?)<\/_w>/')
  if comments {
    for comment in comments[1] {
      text = text.replace(comment, comment.replace('/<\/?_([^>]+)>/', ''), false)
    }
  }
  comments = text.matches('/<_w1>((.|\\n)*?)<\/_w1>/')
  if comments {
    for comment in comments[1] {
      text = text.replace(comment, comment.replace('/<\/?_([^>]+)>/', ''), false)
    }
  }

  # clean up quotes
  var quotes = text.matches('/<_q>((.|\\n)*?)<\/_q>/m')
  if quotes {
    for quote in quotes[1] {
      text = text.replace(
        quote,
        quote.replace('/<\/?_([^>]+)>/', '').
          # interpolation
          replace('/(\\$\{[^}]+\})/m', '<_i>$1</_i>'),
        false
      )
    }
  }

  # expand styles.
  return text.replace('/<_q>((.|\\n)*?)<\/_q>/m', '<span class="${classes.string}">$1</span>').
              replace('/<_i>(.*?)<\/_i>/', '<span class="${classes.interpolation}">$1</span>').
              replace('/<_c>(.*?)<\/_c>/', '<span class="${classes.constant}">$1</span>').
              replace('/<_m>(.*?)<\/_m>/', '<span class="${classes.method}">$1</span>').
              replace('/<_f>(.*?)<\/_f>/', '<span class="${classes.function}">$1</span>').
              replace('/<_k>(.*?)<\/_k>/', '<span class="${classes.keyword}">$1</span>').
              replace('/<_w1?>((.|\\n)*?)<\/_w1?>/', '<span class="${classes.comment}">$1</span>').
              replace('/<_o>(.*?)<\/_o>/', '<span class="${classes.operator}">$1</span>').
              replace('/<_n>(.*?)<\/_n>/', '<span class="${classes.number}">$1</span>')
              replace('/<_[^>]+>/', '') # replace all rouge temporary tags.
}

def highlight_html5(text, lang, classes) {
  var tags = text.matches('/<((?!(\s|!(?=[-]{2})))([^>]+))>/')
  if tags {
    iter var i = 0; i < tags[0].length(); i++ {
      var content = tags[1][i].replace('/([a-zA-Z_\-0-9]+)(?=[=])/', '<^a>$1</^a>').
                        replace('/((?<=((?<!\=)\=))${_quote_re[1,-2]})/', '<^v>$1</^v>').
                        replace('/((?<=((?<!\=)\=))[0-9]+\.?[0-9]*)/', '<^n>$1</^n>')
      text = text.replace(tags[0][i], '<span class="${classes.keyword}">&lt;${content}&gt;</span>', false)
    }
  }

  var result = text.replace('/<\^a>(.*?)<\/\^a>/', '<span class="${classes.function}">$1</span>').
              replace('/<\^v>(.*?)<\/\^v>/', '<span class="${classes.string}">$1</span>').
              replace('/<\^n>(.*?)<\/\^n>/', '<span class="${classes.number}">$1</span>')

  if lang == 'wire' {
    result = result.replace('/(?<!%)(\{\{.+?\}\})/', '<span class="${classes.interpolation}">$1</span>')
  }

  # cleanup comments
  var comments = text.matches('/<(!--.*?--)>/sm')
  if comments {
    iter var i = 0; i < comments[0].length(); i++ {
      result = result.replace(comments[0][i],
        '<span class="${classes.comment}">&lt;' +
        comments[1][i].replace('<', '&lt;').replace('>', '&gt;') +
        '&gt;</span>'
      )
    }
  }

  return result
}

def highlight_json(text, classes) {
  return text.replace('/("(?:[^"\\\\]|\\.)*")/', '<span class="${classes.operator}">$1</span>')
}

def highlight_zuri_repl(text, classes) {
  return '\n'.join(text.split('\n').map(@(line) {
    if line.starts_with('%> ') or line.starts_with('.. ') {
      return '<span class="${classes.prompt}">${line[,3]}</span>' + highlight_zuri(line[3,], classes)
    } else {
      line = line.replace('<', '&lt;').replace('>', '&gt;')
      var lower = line.lower()

      if lower.starts_with('unhandled') or
        lower.starts_with('syntaxerror at') or
        lower.starts_with('illegal state:') or
        lower.match('/^\s{2,}stacktrace/') or
        lower.match('/^\s{2,}&lt;repl&gt;:\d+\s-&gt;\s/') {
        return '<span class="${classes.error}">${line}</span>'
      }

      return '<span class="${classes.result}">${line}</span>'
    }
  }))
}

def highlight(classes) {

  # configuration
  if !classes {
    classes = default_classes
  } else {
    var classes_passed = classes
    classes = default_classes.clone()
    classes.extend(classes_passed)
  }

  return @(text, lang) {
    using lang {
      when 'zuri'
        return highlight_zuri(text, classes)
      when 'zuri-repl'
        return highlight_zuri_repl(text, classes)
      when 'html', 'html5', 'wire'
        return highlight_html5(text, lang, classes)
      when 'json', 'json5'
        return highlight_json(text, classes)
      default
        return text.replace('<', '&lt;').replace('>', '&gt;')
    }
  }
}

