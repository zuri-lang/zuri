import .ruler { Ruler }
import .core


var _rules = [
  [ 'normalize',      core.normalize     ],
  [ 'block',          core.block         ],
  [ 'inline',         core.inline        ],
  [ 'linkify',        core.linkify       ],
  [ 'emoji',          core.emoji         ],
  [ 'replacements',   core.replacements  ],
  [ 'smartquotes',    core.smartquotes   ],
  # `text_join` finds `text_special` tokens (for escape sequences)
  # and joins them with the rest of the text
  [ 'text_join',      core.text_join     ],
  [ 'anchor',         core.anchor        ],
]

/**
 * Top-level rules executor. Glues block/inline parsers and does intermediate
 * transformations.
 * 
 * @internal
 */
class BlockCore {

  /**
   * [[Ruler]] instance. Keep configuration of core rules.
   * 
   * @type Ruler
   */
  var ruler = Ruler()

  /**
   * @internal
   */
  BlockCore() {
    iter var i = 0; i < _rules.length(); i++ {
      self.ruler.push(_rules[i][0], _rules[i][1])
    }
  }

  /**
   * Executes core chain rules.
   * 
   * @internal
   */
  process(state) {
    var i = 0, l, rules
  
    rules = self.ruler.get_rules('')
  
    iter l = rules.length(); i < l; i++ {
      rules[i](state)
    }
  }

  var State = core.core_state.CoreState
}

