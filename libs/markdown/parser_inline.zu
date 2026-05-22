import .ruler { Ruler }
import .inline

# Parser rules

var _rules = [
  [ 'text',            inline.text ],
  [ 'linkify',         inline.linkify ],
  [ 'newline',         inline.newline ],
  [ 'escape',          inline.escape ],
  [ 'backticks',       inline.backticks ],
  [ 'strikethrough',   inline.strikethrough.tokenize ],
  [ 'insert',          inline.insert.tokenize ],
  [ 'mark',            inline.mark.tokenize ],
  [ 'emphasis',        inline.emphasis.tokenize ],
  [ 'subscript',       inline.subscript ],
  [ 'superscript',     inline.superscript ],
  [ 'link',            inline.link ],
  [ 'image',           inline.image ],
  [ 'autolink',        inline.autolink ],
  [ 'html_inline',     inline.html_inline ],
  [ 'entity',          inline.entity ]
]

# `rule2` ruleset was created specifically for emphasis/strikethrough
# post-processing and may be changed in the future.
#
# Don't use this for anything except pairs (plugins working with `balance_pairs`).
var _rules2 = [
  [ 'balance_pairs',   inline.balance_pairs ],
  [ 'strikethrough',   inline.strikethrough.post_process ],
  [ 'insert',          inline.insert.post_process ],
  [ 'mark',            inline.mark.post_process ],
  [ 'emphasis',        inline.emphasis.post_process ],
  # rules for pairs separate '**' into its own text tokens, which may be left unused,
  # rule below merges unused segments back with the rest of the text
  [ 'fragments_join',  inline.fragments_join ]
]


/**
 * Tokenizes paragraph content.
 * 
 * @internal
 */
class InlineParser {

  /**
   * [[Ruler]] instance. Keep configuration of core rules.
   * 
   * @type Ruler
   */
  var ruler = Ruler()

  /**
   * [[Ruler]] instance. Second ruler used for post-processing
   * (e.g. in emphasis-like rules).
   * 
   * @type Ruler
   */
  var ruler2 = Ruler()

  /**
   * @constructor
   */
  InlineParser() {
    iter var i = 0; i < _rules.length(); i++ {
      self.ruler.push(_rules[i][0], _rules[i][1])
    }
  
    iter var i = 0; i < _rules2.length(); i++ {
      self.ruler2.push(_rules2[i][0], _rules2[i][1])
    }
  }

  /**
   * Skip single token by running all rules in validation mode
   */
  skip_token(state) {
    var ok, i, pos = state.pos,
        rules = self.ruler.get_rules(''),
        len = rules.length(),
        max_nesting = state.md.options.max_nesting,
        cache = state.cache

    if cache.get(pos) {
      state.pos = cache[pos]
      return
    }
  
    if state.level < max_nesting {
      iter i = 0; i < len; i++ {
        # Increment state.level and decrement it later to limit recursion.
        # It's harmless to do here, because no tokens are created. But ideally,
        # we'd need a separate private state variable for this purpose.
        state.level++
        ok = rules[i](state, true)
        state.level--
  
        if ok {
          # if pos >= state.pos raise Exception("inline rule didn't increment state position")
          break
        }
      }
    } else {
      # Too much nesting, just skip until the end of the paragraph.
      #
      # NOTE: this will cause links to behave incorrectly in the following case,
      #       when an amount of `[` is exactly equal to `max_nesting + 1`:
      #
      #       [[[[[[[[[[[[[[[[[[[[[foo]()
      #
      # TODO: remove this workaround when CM standard will allow nested links
      #       (we can replace it by preventing links from being parsed in
      #       validation mode)
      state.pos = state.pos_max
    }
   
    if !ok state.pos++
    cache[pos] = state.pos
  }

  /**
   * Generate tokens for input range
   */
  tokenize(state) {
    var ok, i,
        rules = self.ruler.get_rules(''),
        len = rules.length(),
        end = state.pos_max,
        max_nesting = state.md.options.max_nesting
  
    while state.pos < end {
      # Try all possible rules.
      # On success, rule should:
      #
      # - update `state.pos`
      # - update `state.tokens`
      # - return true
  
      # var prev_pos = state.pos
      if state.level < max_nesting {
        iter i = 0; i < len; i++ {
          ok = rules[i](state, false)
          if ok {
            # if prev_pos >= state.pos 
            #   raise Exception("inline rule didn't increment state.pos")
            break
          }
        }
      }
      
      if ok {
        if state.pos >= end break
        continue
      }
  
      state.pending += state.src[state.pos++ - 1]
    }
  
    if state.pending {
      state.push_pending()
    }
  }

  /**
   * Process input string and push inline tokens into `out_tokens`.
   * 
   * @internal
   */
  parse(str, md, env, out_tokens) {
    var i, rules, len
    var state = self.State(str, md, env, out_tokens)
  
    self.tokenize(state)
  
    rules = self.ruler2.get_rules('')
    len = rules.length()
  
    iter i = 0; i < len; i++ {
      rules[i](state)
    }
  }

  var State = inline.inline_state.InlineState
}

