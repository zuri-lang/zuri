# Inline parser state

import ..token as _tkn
import ..common.utils { is_white_space, is_punct_char, is_md_ascii_punct }

class InlineState {
  
  /**
   * Stores { start: end } pairs. Useful for backtrack
   * optimization of pairs parse (emphasis, strikes).
   * 
   * @type dict
   */
  var cache

  /**
   * List of emphasis-like delimiters for current tag.
   * 
   * @type list
   */
  var delimiters

  # backtick length => last seen position
  var backticks
  var backticks_scanned = false

  /**
   * Counter used to disable inline linkify execution
   * inside <a> and markdown links.
   * 
   * @type number
   */
  var link_level = 0

  var level = 0
  var pending_level = 0
  var pos = 0

  /**
   * @constructor
   */
  InlineState(src, md, env, out_tokens) {
    self.src = src
    self.env = env
    self.md = md
    self.tokens = out_tokens
    self.tokens_meta = [nil] * out_tokens.length()
    self.pos_max = self.src.length()
    self.pending = ''

    # Stack of delimiter lists for upper level tags
    self._prev_delimiters = []

    # needs to be overwritten for every new instance.
    self.cache = {}
    self.backticks = {}
    self.delimiters = []
  }

  /**
   * Flush pending text.
   * 
   * @returns Token
   */
  push_pending() {
    var token = _tkn.Token('text', '', 0)
    token.content = self.pending
    token.level = self.pending_level
    self.tokens.append(token)
    self.pending = ''
    return token
  }

  /**
   * Push new token to "stream".
   * If pending text exists - flush it as text token
   */
  push(type, tag, nesting) {
    if self.pending {
      self.push_pending()
    }
  
    var token = _tkn.Token(type, tag, nesting)
    var token_meta = nil
  
    if nesting < 0 {
      # closing tag
      self.level--
      self.delimiters = self._prev_delimiters.pop()
    }
  
    token.level = self.level
  
    if nesting > 0 {
      # opening tag
      self.level++
      self._prev_delimiters.append(self.delimiters)
      self.delimiters = []
      token_meta = { delimiters: self.delimiters }
    }
  
    self.pending_level = self.level
    self.tokens.append(token)
    self.tokens_meta.append(token_meta)
    return token
  }

  /**
   * Scan a sequence of emphasis-like markers, and determine whether
   * it can start an emphasis sequence or end an emphasis sequence.
   * 
   *   - `start` - position to scan from (it should point at a valid marker)
   *   - `can_split_word` - determine if these markers can be found inside a word
   */
  scan_delims(start, can_split_word) {
    var pos = start, last_char, nextChar, count, can_open, can_close,
        is_last_white_space, is_last_punct_char,
        is_next_white_space, is_next_punct_char,
        left_flanking = true,
        right_flanking = true,
        max = self.pos_max,
        marker = self.src[start]
  
    # treat beginning of the line as a whitespace
    last_char = start > 0 ? self.src[start - 1] : ' '
  
    while pos < max and self.src[pos] == marker pos++
  
    count = pos - start

    # treat end of the line as a whitespace
    nextChar = pos < max ? self.src[pos] : ' '
  
    is_last_punct_char = is_md_ascii_punct(last_char) or is_punct_char(last_char)
    is_next_punct_char = is_md_ascii_punct(nextChar) or is_punct_char(nextChar)
 
    is_last_white_space = is_white_space(last_char)
    is_next_white_space = is_white_space(nextChar)
  
    if is_next_white_space {
      left_flanking = false
    } else if is_next_punct_char {
      if !(is_last_white_space or is_last_punct_char) {
        left_flanking = false
      }
    }
  
    if is_last_white_space {
      right_flanking = false
    } else if is_last_punct_char {
      if !(is_next_white_space or is_next_punct_char) {
        right_flanking = false
      }
    }
  
    if !can_split_word {
      can_open  = left_flanking  and (!right_flanking or is_last_punct_char)
      can_close = right_flanking and (!left_flanking  or is_next_punct_char)
    } else {
      can_open  = left_flanking
      can_close = right_flanking
    }
  
    return {
      can_open:  can_open,
      can_close: can_close,
      length:    count
    }
  }

  var Token = _tkn.Token
}

