def inline(state) {
  var tokens = state.tokens, tok, i = 0, l

  # Parse inlines
  iter l = tokens.length(); i < l; i++ {
    tok = tokens[i]
    if tok.type == 'inline' {
      state.md.inline.parse(tok.content, state.md, state.env, tok.children)
    }
  }
}

