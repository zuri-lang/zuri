#!-- part of the html module

def split_head(str, sep) {
  var idx = str.index_of(sep)
  if idx == -1 return [str]
  return [str[0, idx], str[idx + sep.length(),]]
}

def unquote(str) {
  var car = str[0]
  var end = str.length() - 1
  var is_quote_start = car == '"' or car == "'"
  if is_quote_start and car == str[end] {
    return str[1, end]
  }
  return str
}

def format(nodes, options) {
  return nodes.map(@( node ) {
    var type = node.type
    var output_node = type == 'element' ? {
        type,
        name: node.name,
        attributes: format_attributes(node.attributes),
        children: format(node.children, options),
      } : { 
        type, 
        content: node.content, 
      }
    if options.get('with_position', false) {
      output_node.position = node.position
    }
    return output_node
  })
}

def format_attributes(attributes) {
  return attributes.map(@( attribute ) {
    var parts = split_head(attribute.trim(), '=')
    var name = parts[0]
    var value
    if parts.length() > 1 {
      value = is_string(parts[1]) ? unquote(parts[1]) : nil
    }
    return {name, value}
  })
}
