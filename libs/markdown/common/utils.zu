# Merge objects
def assign(obj, ...) {
  if !is_dict(obj) 
    raise TypeError('dictionary expected in argument 1')
  for source in __args__ {
    if !source continue
    if !is_dict(source)
      raise TypeError('invalid dictionary in parameter list')

    obj.extend(source)
  }

  return obj
}

# Remove element from array and put another array at those position.
# Useful for some operations with tokens
def array_replace_at(src, pos, new_elements) {
  return src[,pos] + new_elements + src[pos + 1,]
}

def is_valid_entity_code(c) {
  if is_string(c) c = to_number(c)
  # broken sequence
  if c >= 0xD800 and c <= 0xDFFF return false
  # never used
  if c >= 0xFDD0 and c <= 0xFDEF return false
  if (c & 0xFFFF) == 0xFFFF or (c & 0xFFFF) == 0xFFFE return false
  # control codes
  if c >= 0x00 and c <= 0x08 return false
  if c == 0x0B return false
  if c >= 0x0E and c <= 0x1F return false
  if c >= 0x7F and c <= 0x9F return false
  # out of range
  if c > 0x10FFFF return false
  return true
}

var UNESCAPE_MD_RE  = '\\\\([\\\\!"#$%&\'()*+,.\\/:;<=>?@[\\]^_`{|}~-])'
var ENTITY_RE       = '&([a-z#][a-z0-9]{1,31});'
var UNESCAPE_ALL_RE = '/' + UNESCAPE_MD_RE + '|' + ENTITY_RE + '/si'
var UNESCAPE_RE = '/\\\\([ \\\\!"#$%&\'()*+,.\/:;<=>?@[\]^_`{|}~-])/'
var UNESCAPE_SPACE_RE = '/(^|[^\\\\])(\\\\\\\\)*\s/'

var DIGITAL_ENTITY_TEST_RE = '/^#((?:x[a-f0-9]{1,8}|[0-9]{1,8}))$/i'

import .entities { entities }

def replace_entity_pattern(match, name) {
  var code

  if entities.contains(name) {
    return entities[name]
  }

  if name[0] == '#' and name.match(DIGITAL_ENTITY_TEST_RE) {
    code = to_number(name[1].lower() == 'x' ? '0' + name[1,] : name[1,])

    if is_valid_entity_code(code) {
      return chr(code)
    }
  }

  return match
}

def unescape_md(str) {
  if !str.index_of('\\') return str
  return str.replace('/' + UNESCAPE_MD_RE + '/s', '$1')
}

/* def str_replace_fn(str, pattern, fn) {
  var result = '', match, next_index = 0

  while match = str.match(pattern, next_index) {
    var index = str[next_index,].index_of(match[0])
    result += str[next_index, next_index + index]
    next_index += index + match[0].length()

    result += fn(match.to_list()[1])
  }
  if next_index < str.length() {
    result += str[next_index,]
  }

  return result
} */

def unescape_all(str) {
  if !str.index_of('\\') and !str.index_of('&') return str

  return str.replace_with(UNESCAPE_ALL_RE, @(match, escaped, entity) {
    if escaped return escaped
    return replace_entity_pattern(match, entity)
  })
}

var HTML_ESCAPE_TEST_RE = '/[&<>"]/'
var HTML_ESCAPE_REPLACE_RE = '/[&<>"]/'
var HTML_REPLACEMENTS = {
  '&': '&amp;',
  '<': '&lt;',
  '>': '&gt;',
  '"': '&quot;',
}

def replace_unsafe_char(ch) {
  return HTML_REPLACEMENTS[ch]
}

def escape_html(str) {
  if str.match(HTML_ESCAPE_TEST_RE) {
    return str.replace_with(HTML_ESCAPE_REPLACE_RE, replace_unsafe_char)
  }
  return str
}

var REGEXP_ESCAPE_RE = '/[.?*+^$[\]\\\\(){}|-]/'

def escape_re(str) {
  return str.replace(REGEXP_ESCAPE_RE, '\\$&')
}

def is_space(code) {
  using code {
    when 0x09, 0x20, '\t', ' ' return true
  }

  return false
}

# Zs (unicode class) || [\t\f\v\r\n]
def is_white_space(code) {
  if is_string(code) code = ord(code)
  if code >= 0x2000 and code <= 0x200A return true
  using code {
    when  0x09, # \t
          0x0A, # \n
          0x0B, # \v
          0x0C, # \f
          0x0D, # \r
          0x20,
          0xA0,
          0x1680,
          0x202F,
          0x205F,
          0x3000 return true
  }
  return false
}

# TODO: Make this regex work!
/* /[!-#%-\*,-\/:;\?@\[-\]_\{\}\xA1\xA7\xAB\xB6\xB7\xBB\xBF\u037E\u0387\u055A-\u055F\u0589\u058A\u05BE\u05C0\u05C3\u05C6\u05F3\u05F4\u0609\u060A\u060C\u060D\u061B\u061E\u061F\u066A-\u066D\u06D4\u0700-\u070D\u07F7-\u07F9\u0830-\u083E\u085E\u0964\u0965\u0970\u09FD\u0A76\u0AF0\u0C84\u0DF4\u0E4F\u0E5A\u0E5B\u0F04-\u0F12\u0F14\u0F3A-\u0F3D\u0F85\u0FD0-\u0FD4\u0FD9\u0FDA\u104A-\u104F\u10FB\u1360-\u1368\u1400\u166D\u166E\u169B\u169C\u16EB-\u16ED\u1735\u1736\u17D4-\u17D6\u17D8-\u17DA\u1800-\u180A\u1944\u1945\u1A1E\u1A1F\u1AA0-\u1AA6\u1AA8-\u1AAD\u1B5A-\u1B60\u1BFC-\u1BFF\u1C3B-\u1C3F\u1C7E\u1C7F\u1CC0-\u1CC7\u1CD3\u2010-\u2027\u2030-\u2043\u2045-\u2051\u2053-\u205E\u207D\u207E\u208D\u208E\u2308-\u230B\u2329\u232A\u2768-\u2775\u27C5\u27C6\u27E6-\u27EF\u2983-\u2998\u29D8-\u29DB\u29FC\u29FD\u2CF9-\u2CFC\u2CFE\u2CFF\u2D70\u2E00-\u2E2E\u2E30-\u2E4E\u3001-\u3003\u3008-\u3011\u3014-\u301F\u3030\u303D\u30A0\u30FB\uA4FE\uA4FF\uA60D-\uA60F\uA673\uA67E\uA6F2-\uA6F7\uA874-\uA877\uA8CE\uA8CF\uA8F8-\uA8FA\uA8FC\uA92E\uA92F\uA95F\uA9C1-\uA9CD\uA9DE\uA9DF\uAA5C-\uAA5F\uAADE\uAADF\uAAF0\uAAF1\uABEB\uFD3E\uFD3F\uFE10-\uFE19\uFE30-\uFE52\uFE54-\uFE61\uFE63\uFE68\uFE6A\uFE6B\uFF01-\uFF03\uFF05-\uFF0A\uFF0C-\uFF0F\uFF1A\uFF1B\uFF1F\uFF20\uFF3B-\uFF3D\uFF3F\uFF5B\uFF5D\uFF5F-\uFF65]|\uD800[\uDD00-\uDD02\uDF9F\uDFD0]|\uD801\uDD6F|\uD802[\uDC57\uDD1F\uDD3F\uDE50-\uDE58\uDE7F\uDEF0-\uDEF6\uDF39-\uDF3F\uDF99-\uDF9C]|\uD803[\uDF55-\uDF59]|\uD804[\uDC47-\uDC4D\uDCBB\uDCBC\uDCBE-\uDCC1\uDD40-\uDD43\uDD74\uDD75\uDDC5-\uDDC8\uDDCD\uDDDB\uDDDD-\uDDDF\uDE38-\uDE3D\uDEA9]|\uD805[\uDC4B-\uDC4F\uDC5B\uDC5D\uDCC6\uDDC1-\uDDD7\uDE41-\uDE43\uDE60-\uDE6C\uDF3C-\uDF3E]|\uD806[\uDC3B\uDE3F-\uDE46\uDE9A-\uDE9C\uDE9E-\uDEA2]|\uD807[\uDC41-\uDC45\uDC70\uDC71\uDEF7\uDEF8]|\uD809[\uDC70-\uDC74]|\uD81A[\uDE6E\uDE6F\uDEF5\uDF37-\uDF3B\uDF44]|\uD81B[\uDE97-\uDE9A]|\uD82F\uDC9F|\uD836[\uDE87-\uDE8B]|\uD83A[\uDD5E\uDD5F]/ */

# Currently without astral characters support.
var UNICODE_PUNCT_RE = '/[!-#%-\*,-\/:;\?@\[-\]_\{\}]|\p{P}|\p{Pc}\p{Pd}|\p{Pe}|\p{Pf}|\p{Pi}|\p{Po}|\p{Ps}/u'

# Currently without astral characters support.
def is_punct_char(ch) {
  return ch.match(UNICODE_PUNCT_RE)
}

var _md_ascii_punct = [
  '!', 0x21,
  '"', 0x22,
  '#', 0x23,
  '$', 0x24,
  '%', 0x25,
  '&', 0x26,
  '\'', 0x27,
  '(', 0x28,
  ')', 0x29,
  '*', 0x2a,
  '+', 0x2b,
  ',', 0x2c,
  '-', 0x2d,
  '.', 0x2e,
  '/', 0x2f,
  ':', 0x3a,
  ';', 0x3b,
  '<', 0x3c,
  '=', 0x3d,
  '>', 0x3e,
  '?', 0x3f,
  '@', 0x40,
  '[', 0x5b,
  '\\', 0x5c,
  ']', 0x5d,
  '^', 0x5e,
  '_', 0x5f,
  '`', 0x60,
  '{', 0x7b,
  '|', 0x7c,
  '}', 0x7d,
  '~', 0x7e,
]

def is_md_ascii_punct(ch) {
  return _md_ascii_punct.contains(ch)
}

def normalize_reference(str) {
  # Trim and collapse whitespace
  #
  return str.trim().replace('/\s+/', ' ').case_fold(true).upper()
}


var NAMED_RE   = '/^&([a-z][a-z0-9]{1,31});/i'

def replace_entities(str) {
  if str.index_of('&') < 0 return str
  return str.replace_with(NAMED_RE, replace_entity_pattern)
}
