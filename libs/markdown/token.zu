# Token class

/**
 * class Token
 *
 * @serializable
 */
class Token {

  /**
   * Type of the token (string, e.g. "paragraph_open")
   * 
   * @type string
   */
  var type

  /**
   * html tag name, e.g. "p"
   * 
   * @type string
   */
  var tag

  /**
   * Html attributes. Format: `[ [ name1, value1 ], [ name2, value2 ] ]`.
   * 
   * @type list
   */
  var attrs

  /**
   * Source map info. Format: `[ line_begin, line_end ]`.
   * 
   * @type list
   */
  var map

  /**
   * Level change (number in {-1, 0, 1} set), where:
   *
   * -  `1` means the tag is opening
   * -  `0` means the tag is self-closing
   * - `-1` means the tag is closing
   * 
   * @type number
   */
  var nesting

  /**
   * nesting level, the same as `state.level`.
   * 
   * @type number
   */
  var level

  /**
   * A list of child nodes (inline and img tokens).
   * 
   * @type list
   */
  var children

  /**
   * In a case of self-closing tag (code, html, fence, etc.),
   * it has contents of this tag.
   * 
   * @type string
   */
  var content

  /**
   * '*' or '_' for emphasis, fence string for fence, etc.
   * 
   * @type string
   */
  var markup

  /**
   * Additional information:
   *
   * - Info string for "fence" tokens
   * - The value "auto" for autolink "link_open" and "link_close" tokens
   * - The string value of the item marker for ordered-list "list_item_open" tokens
   * 
   * @type string
   */
  var info

  /**
   * A place for plugins to store an arbitrary data.
   * 
   * @type dict
   */
  var meta

  /**
   * True for block-level tokens, false for inline tokens.
   * Used in renderer to calculate line breaks.
   * 
   * @type bool
   */
  var block

  /**
   * If it's true, ignore this element when rendering. Used for tight lists
   * to hide paragraphs.
   * 
   * @type bool
   */
  var hidden

  /**
   * @constructor
   */
  Token(type, tag, nesting) {
    self.type     = type
    self.tag      = tag
    self.attrs    = nil
    self.map      = nil
    self.nesting  = nesting
    self.level    = 0
    self.children = nil
    self.content  = ''
    self.markup   = ''
    self.info     = ''
    self.meta     = nil
    self.block    = false
    self.hidden   = false
  }

  /**
   * Search attribute index by name.
   * 
   * @type number
   */
  attr_index(name) {
    var attrs, i = 0, len
  
    if !self.attrs return -1
  
    attrs = self.attrs
  
    iter len = attrs.length(); i < len; i++ {
      if attrs[i][0] == name return i
    }
    return -1
  }

  /**
   * Add `[ name, value ]` attribute to list. Init attrs if necessary
   */
  attr_push(attr_data) {
    if self.attrs {
      self.attrs.append(attr_data)
    } else {
      self.attrs = [ attr_data ]
    }
  }

  /**
   * Set `name` attribute to `value`. Override old value if exists.
   */
  attr_set(name, value) {
    var idx = self.attr_index(name),
        attr_data = [ name, value ]
  
    if idx < 0 {
      self.attr_push(attr_data)
    } else {
      self.attrs[idx] = attr_data
    }
  }

  /**
   * Get the value of attribute `name`, or nil if it does not exist.
   */
  attr_get(name) {
    var idx = self.attr_index(name), value = nil
    if idx >= 0 {
      value = self.attrs[idx][1]
    }
    return value
  }

  /**
   * Join value to existing attribute via space. Or create new attribute if not
   * exists. Useful to operate with token classes.
   */
  attr_join(name, value) {
    var idx = self.attr_index(name)
  
    if idx < 0 {
      self.attr_push([ name, value ])
    } else {
      self.attrs[idx][1] = self.attrs[idx][1] + ' ' + value
    }
  }

  /**
   * Returns the Token properties as a dictionary.
   *
   * @returns dict
   */
  to_dict() {
    return {
      type: self.type,
      tag: self.tag,
      attrs: self.attrs,
      map: self.map,
      nesting: self.nesting,
      level: self.level,
      children: self.children,
      content: self.content,
      markup: self.markup,
      info: self.info,
      meta: self.meta,
      block: self.block,
      hidden: self.hidden,
    }
  }

  @to_json() {
    return self.to_dict()
  }
}

