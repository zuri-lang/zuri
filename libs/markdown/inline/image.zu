# Process ![image](<src> "title")

import ..common.utils { normalize_reference, is_space }

def image(state, silent) {
  var attrs,
      code,
      content,
      label,
      label_end,
      label_start,
      pos,
      ref,
      res,
      title,
      token,
      tokens,
      start,
      href = '',
      old_pos = state.pos,
      max = state.pos_max

  if state.src[state.pos] != '!' return false
  if state.pos + 1 >= state.src.length() or state.src[state.pos + 1] != '[' return false

  label_start = state.pos + 2
  label_end = state.md.helpers.parse_link_label(state, state.pos + 1, false)

  # parser failed to find ']', so it's not a valid link
  if label_end < 0 return false

  pos = label_end + 1
  if pos < max and state.src[pos] == '(' {
    #
    # Inline link
    #

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
    } else {
      title = ''
    }

    if pos >= max or state.src[pos] != ')' {
      state.pos = old_pos
      return false
    }
    pos++
  } else {
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

    # covers label == '' and label == undefined
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
  # We found the end of the link, and know for a fact it's a valid link;
  # so all that's left to do is to call tokenizer.
  #
  if !silent {
    content = state.src[label_start, label_end]

    state.md.inline.parse(
      content,
      state.md,
      state.env,
      tokens = []
    )

    token          = state.push('image', 'img', 0)
    token.attrs    = attrs = [ [ 'src', href ], [ 'alt', '' ] ]
    token.children = tokens
    token.content  = content

    if title {
      attrs.append([ 'title', title ])
    }
  }

  state.pos = pos
  state.pos_max = max
  return true
}
