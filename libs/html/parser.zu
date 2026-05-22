#!-- part of the html module

def parser(tokens, options) {
  var root = {
    name: nil, 
    children: [],
  }
  var state = {
    tokens, 
    options, 
    cursor: 0,
    stack: [
      root
    ],
  }
  parse(state)
  return root.children
}

def has_terminal_parent(name, stack, terminals) {
  var tag_parents = terminals.get(name, nil)
  if tag_parents {
    var current_index = stack.length() - 1
    while current_index >= 0 {
      var parent_tag_name = stack[current_index].name
      if parent_tag_name == name {
        break
      }
      if tag_parents.contains(parent_tag_name) {
        return true
      }
      current_index--
    }
  }
  return false
}

def rewind_stack(stack, new_length, children_end_position, end_position) {
  stack[new_length].position.end = end_position
  var len = stack.length()
  iter var i = new_length + 1; i < len; i++ {
    stack[i].position.end = children_end_position
  }
  stack[new_length,]
}

def parse(state) {
  var nodes = state.stack[state.stack.length() - 1].children
  var len = state.tokens.length()
  while state.cursor < len {
    var token = state.tokens[state.cursor]
    if token.type != 'tag-start' {
      nodes.append(token)
      state.cursor++
      continue
    }

    var tag_token = state.tokens[state.cursor++]
    state.cursor++
    var name = tag_token.content.lower()
    if token.close {
      var index = state.stack.length()
      var should_rewind = false
      while index-- > -1 {
        if state.stack[index].name == name {
          should_rewind = true
          break
        }
      }
      while state.cursor < len {
        var end_token = state.tokens[state.cursor]
        if end_token.type != 'tag-end' break
        state.cursor++
      }
      if should_rewind {
        rewind_stack(state.stack, index, token.position.start, state.tokens[state.cursor - 1].position.end)
        break
      } else {
        continue
      }
    }

    var is_closing_tag = state.options.closing_tags.contains(name.lower())
    var should_rewind_to_auto_close = is_closing_tag
    if should_rewind_to_auto_close {
      var terminals = state.options.get('tag_ancestors')
      should_rewind_to_auto_close = !has_terminal_parent(name.lower(), state.stack, terminals)
    }

    if should_rewind_to_auto_close {
      # rewind the stack to just above the previous
      # closing tag of the same name
      var current_index = state.stack.length() - 1
      while current_index > 0 {
        if name == state.stack[current_index].name {
          rewind_stack(state.stack, current_index, token.position.start, token.position.start)
          var previous_index = current_index - 1
          nodes = state.stack[previous_index].children
          break
        }
        current_index = current_index - 1
      }
    }

    var attributes = []
    var attr_token
    while state.cursor < len {
      attr_token = state.tokens[state.cursor]
      if attr_token.type == 'tag-end' break
      attributes.append(attr_token.content)
      state.cursor++
    }

    state.cursor++
    var children = []
    var position = {
      start: token.position.start,
      end: attr_token.position.end
    }
    var element_node = {
      type: 'element',
      name: tag_token.content,
      attributes,
      children,
      position,
    }
    nodes.append(element_node)

    var has_children = !(attr_token.close or state.options.void_tags.contains(name.lower()))
    if has_children {
      var size = state.stack.append({
        name, 
        children, 
        position,
      })
      var inner_state = {
        tokens: state.tokens, 
        options: state.options, 
        cursor: state.cursor, 
        stack: state.stack,
      }
      parse(inner_state)
      state.cursor = inner_state.cursor
      var rewound_in_element = state.stack.length() == size
      if rewound_in_element {
        element_node.position.end = state.tokens[state.cursor - 1].position.end
      }
    }
  }
}
