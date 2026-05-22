def block(state) {
  var token

  if state.inline_mode {
    token          = state.Token('inline', '', 0)
    token.content  = state.src
    token.map      = [ 0, 1 ]
    token.children = []
    state.tokens.append(token)
  } else {
    state.md.block.parse(state.src, state.md, state.env, state.tokens)
  }
}

