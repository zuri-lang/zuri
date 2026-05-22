import .ruler { Ruler }
import .block


var _rules = [
  # First 2 params - rule name & source. Secondary array - list of rules,
  # which can be terminated by this one.
  [ 'table',      block.table,      [ 'paragraph', 'reference' ] ],
  [ 'code',       block.code ],
  [ 'fence',      block.fence,      [ 'paragraph', 'reference', 'blockquote', 'list' ] ],
  [ 'blockquote', block.blockquote, [ 'paragraph', 'reference', 'blockquote', 'list' ] ],
  [ 'hr',         block.hr,         [ 'paragraph', 'reference', 'blockquote', 'list' ] ],
  [ 'list',       block.list,       [ 'paragraph', 'reference', 'blockquote' ] ],
  [ 'reference',  block.reference ],
  [ 'html_block', block.html_block, [ 'paragraph', 'reference', 'blockquote' ] ],
  [ 'heading',    block.heading,    [ 'paragraph', 'reference', 'blockquote' ] ],
  [ 'lheading',   block.lheading ],
  [ 'deflist',    block.deflist,    [ 'paragraph', 'reference', 'blockquote' ] ],
  [ 'paragraph',  block.paragraph ]
]


/**
 * Block-level tokenizer.
 * 
 * @internal
 */
class BlockParser {

  /**
   * [[Ruler]] instance. Keep configuration of block rules.
   * 
   * @type Ruler
   */
  var ruler = Ruler()

  /**
   * @constructor
   */
  BlockParser() {
    iter var i = 0; i < _rules.length(); i++ {
      self.ruler.push(_rules[i][0], _rules[i][1], { alt: (_rules[i].get(2) or [])[,] })
    }
  }

  /**
   * Generate tokens for input range
   */
  tokenize(state, start_line, end_line) {
    var ok, i, prev_line,
        rules = self.ruler.get_rules(''),
        len = rules.length(),
        line = start_line,
        has_empty_lines = false,
        max_nesting = state.md.options.max_nesting
  
    while line < end_line {
      state.line = line = state.skip_empty_lines(line)
      if line >= end_line break
  
      # Termination condition for nested calls.
      # Nested calls currently used for blockquotes & lists
      if state.s_count[line] < state.blk_indent  break
  
      # If nesting level exceeded - skip tail to the end. That's not ordinary
      # situation and we should not care about content.
      if state.level >= max_nesting {
        state.line = end_line
        break
      }
  
      # Try all possible rules.
      # On success, rule should:
      #
      # - update `state.line`
      # - update `state.tokens`
      # - return true
      prev_line = state.line
  
      iter i = 0; i < len; i++ {
        ok = rules[i](state, line, end_line, false)
        if ok {
          # if prev_line >= state.line {
          #   raise Exception("block rule didn't increment state line")
          # }
          break
        }
      }
  
      # this can only happen if user disables paragraph rule
      if !ok raise Exception('none of the block rules matched')
  
      # set state.tight if we had an empty line before current tag
      # i.e. latest empty line should not count
      state.tight = !has_empty_lines
  
      # paragraph might "eat" one newline after it in nested lists
      if state.is_empty(state.line - 1) {
        has_empty_lines = true
      }
  
      line = state.line
  
      if line < end_line and state.is_empty(line) {
        has_empty_lines = true
        line++
        state.line = line
      }
    }
  }

  /**
   * Process input string and push block tokens into `out_tokens`.
   * 
   * @internal
   */
  parse(src, md, env, out_tokens) {
    var state
  
    if !src return
  
    state = self.State(src, md, env, out_tokens)
  
    self.tokenize(state, state.line, state.line_max)
  }

  var State = block.block_state.BlockState
}

