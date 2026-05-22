# Process [link](<to> "stuff")

import ..common.utils { normalize_reference, is_space }

def link(state, silent) {
  var attrs,
      code,
      label,
      label_end,
      label_start,
      pos,
      res,
      ref,
      token,
      href = '',
      title = '',
      old_pos = state.pos,
      max = state.pos_max,
      start = state.pos,
      parse_reference = true

  if state.src[state.pos] != '[' return false

  label_start = state.pos + 1
  label_end = state.md.helpers.parse_link_label(state, state.pos, true)

  # parser failed to find ']', so it's not a valid link
  if label_end < 0 return false

  pos = label_end + 1
  if pos < max and state.src[pos] == '(' {
    #
    # Inline link
    #

    # might have found a valid shortcut link, disable reference parsing
    parse_reference = false

    # [link](  <href>  "title"  )
    #        ^^ skipping these spaces
    pos++
    iter ; pos < max; pos++ {
      code = state.src[pos]
      if !is_space(code) and code != '\n' break
    }
    if pos >= max return false

    # [link](  <href>  "title"  )
    #          ^^^^^^ parsing link destination
    start = pos
    res = state.md.helpers.parse_link_destination(state.src, pos, state.pos_max)
    if res.ok {
      href = state.md.normalize_link(res.str)
      if state.md.validate_link(href) {
        pos = res.pos
      } else {
        href = ''
      }

      # [link](  <href>  "title"  )
      #                ^^ skipping these spaces
      start = pos
      iter ; pos < max; pos++ {
        code = state.src[pos]
        if !is_space(code) and code != '\n' break
      }

      # [link](  <href>  "title"  )
      #                  ^^^^^^^ parsing link title
      res = state.md.helpers.parse_link_title(state.src, pos, state.pos_max)
      if pos < max and start != pos and res.ok {
        title = res.str
        pos = res.pos

        # [link](  <href>  "title"  )
        #                         ^^ skipping these spaces
        iter ; pos < max; pos++ {
          code = state.src[pos]
          if !is_space(code) and code != '\n' break
        }
      }
    }

    if pos >= max or state.src[pos] != ')' {
      # parsing a valid shortcut link failed, fallback to reference
      parse_reference = true
    }
    pos++
  }

  if parse_reference {
    #
    # Link reference
    #
    if !state.env.contains('references') return false

    if pos < max and state.src[pos] == '[' {
      start = pos + 1
      pos = state.md.helpers.parse_link_label(state, pos)
      if pos >= 0 {
        label = state.src[start, pos++ - 1]
      } else {
        pos = label_end + 1
      }
    } else {
      pos = label_end + 1
    }

    # covers label == '' and label == nil
    # (collapsed reference link and shortcut reference link respectively)
    if !label label = state.src[label_start, label_end]
    
    ref = state.env.references.get(normalize_reference(label), nil)
    if !ref {
      state.pos = old_pos
      return false
    }
    href = ref.href
    title = ref.title
  }

  #
  # We found the end of the link, and know for a fact it's a valid link
  # so all that's left to do is to call tokenizer.
  #
  if !silent {
    state.pos = label_start
    state.pos_max = label_end

    token        = state.push('link_open', 'a', 1)
    token.attrs  = attrs = [ [ 'href', href ] ]
    if title {
      attrs.append([ 'title', title ])
    }

    state.link_level++
    state.md.inline.tokenize(state)
    state.link_level--

    token        = state.push('link_close', 'a', -1)
  }

  state.pos = pos
  state.pos_max = max
  return true
}

