# Parse link destination

import ..common.utils { unescape_all }

def parse_link_destination(str, pos, max) {
  var code, level,
      lines = 0,
      start = pos,
      result = {
        ok: false,
        pos: 0,
        lines: 0,
        str: '',
      }
      
  if str.length() <= pos return result

  if str[pos] == '<' {
    pos++
    while pos < max {
      code = str[pos]
      if code == '\n' return result
      if code == '<' return result
      if code == '>' {
        result.pos = pos + 1
        result.str = unescape_all(str[start + 1, pos])
        result.ok = true
        return result
      }
      if code == '\\' and pos + 1 < max {
        pos += 2
        continue
      }

      pos++
    }

    # no closing '>'
    return result
  }

  # this should be ... } else { ... branch

  level = 0
  while pos < max {
    code = str[pos]

    if code == ' ' break

    # ascii control characters
    if ord(code) < 0x20 or ord(code) == 0x7F break

    if code == '\\' and pos + 1 < max {
      if str[pos + 1] == ' ' break
      pos += 2
      continue
    }

    if code == '(' {
      level++
      if level > 32 return result
    }

    if code == ')' {
      if level == 0 break
      level--
    }

    pos++
  }

  if start == pos return result
  if level != 0 return result

  result.str = unescape_all(str[start, pos])
  result.pos = pos
  result.ok = true
  return result
}

 