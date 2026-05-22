#!-- part of the html module

def format_attributes(attributes) {
  return attributes.reduce(@( attrs, attribute ) {
    if (attribute.value == nil) {
      return '${attrs} ${attribute.name}'
    }
    # var quote_escape = attribute.value.index_of('\'') != -1
    # var quote = quote_escape ? '"' : '\''
    var quote = '"'
    return '${attrs} ${attribute.name}=${quote}${attribute.value}${quote}'
  }, '')
}

def html(tree, options) {
  var res = ''
  if tree {
    for node in tree {
      if node {
        if is_list(node) {
          res += html(node, options)
        } else  if (node.type == 'text') {
          res += node.content
        } else if (node.type == 'comment') {
          res += '<!--${node.content}-->'
        } else  {  
          var is_self_closing = options.void_tags.contains(node.name.lower())
          res += is_self_closing ? '<${node.name}${format_attributes(node.attributes)}>' : 
            '<${node.name}${format_attributes(node.attributes)}>${html(node.children, options)}</${node.name}>'
        }
      }
    }
  }
  return res
}
