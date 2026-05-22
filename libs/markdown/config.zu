var commonmark = {
  options: {
    html:         true,         # Enable HTML tags in source
    xhtml_out:     true,         # Use '/' to close single tags (<br />)
    breaks:       false,        # Convert '\n' in paragraphs into <br>
    lang_prefix:   'language-',  # CSS language prefix for fenced blocks
    linkify:      false,        # auto convert URL-like texts to links

    # Enable some language-neutral replacements + quotes beautification
    typographer:  false,

    # Double + single quotes replacement pairs, when typographer enabled,
    # and smart quotes on. Could be either a String or an Array.
    #
    # For example, you can use '«»„“' for Russian, '„“‚‘' for German,
    # and ['«\xA0', '\xA0»', '‹\xA0', '\xA0›'] for French (including nbsp).
    quotes: '\u201c\u201d\u2018\u2019', /* “”‘’ */

    # Highlighter function. Should return escaped HTML,
    # or '' if the source string is not changed and should be escaped externally.
    # If result starts with <pre... internal wrapper is skipped.
    #
    # function (/*str, lang*/) { return ''; }
    #
    highlight: nil,

    max_nesting:   20            # Internal protection, recursion limit
  },

  components: {

    core: {
      rules: [
        'normalize',
        'block',
        'inline',
        'text_join'
      ]
    },

    block: {
      rules: [
        'blockquote',
        'code',
        'fence',
        'heading',
        'hr',
        'html_block',
        'lheading',
        'list',
        'reference',
        'paragraph'
      ]
    },

    inline: {
      rules: [
        'autolink',
        'backticks',
        'emphasis',
        'entity',
        'escape',
        'html_inline',
        'image',
        'link',
        'newline',
        'text'
      ],
      rules2: [
        'balance_pairs',
        'emphasis',
        'fragments_join'
      ]
    }
  }
}

var standard = {
  options: {
    html:         false,        # Enable HTML tags in source
    xhtml_out:     false,        # Use '/' to close single tags (<br />)
    breaks:       false,        # Convert '\n' in paragraphs into <br>
    lang_prefix:   'language-',  # CSS language prefix for fenced blocks
    linkify:      false,        # auto convert URL-like texts to links

    # Enable some language-neutral replacements + quotes beautification
    typographer:  false,

    # Double + single quotes replacement pairs, when typographer enabled,
    # and smart quotes on. Could be either a String or an Array.
    #
    # For example, you can use '«»„“' for Russian, '„“‚‘' for German,
    # and ['«\xA0', '\xA0»', '‹\xA0', '\xA0›'] for French (including nbsp).
    quotes: '\u201c\u201d\u2018\u2019', /* “”‘’ */

    # Highlighter function. Should return escaped HTML,
    # or '' if the source string is not changed and should be escaped externally.
    # If result starts with <pre... internal wrapper is skipped.
    #
    # def (/*str, lang*/) { return '' }
    #
    highlight: nil,

    max_nesting:   100            # Internal protection, recursion limit
  },

  components: {

    core: {},
    block: {},
    inline: {}
  }
}

var zero = {
  options: {
    html:         false,        # Enable HTML tags in source
    xhtml_out:     false,        # Use '/' to close single tags (<br />)
    breaks:       false,        # Convert '\n' in paragraphs into <br>
    lang_prefix:   'language-',  # CSS language prefix for fenced blocks
    linkify:      false,        # auto convert URL-like texts to links

    # Enable some language-neutral replacements + quotes beautification
    typographer:  false,

    # Double + single quotes replacement pairs, when typographer enabled,
    # and smart quotes on. Could be either a String or an Array.
    #
    # For example, you can use '«»„“' for Russian, '„“‚‘' for German,
    # and ['«\xA0', '\xA0»', '‹\xA0', '\xA0›'] for French (including nbsp).
    quotes: '\u201c\u201d\u2018\u2019', /* “”‘’ */

    # Highlighter function. Should return escaped HTML,
    # or '' if the source string is not changed and should be escaped externally.
    # If result starts with <pre... internal wrapper is skipped.
    #
    # function (/*str, lang*/) { return ''; }
    #
    highlight: nil,

    max_nesting:   20            # Internal protection, recursion limit
  },

  components: {

    core: {
      rules: [
        'normalize',
        'block',
        'inline',
        'text_join'
      ]
    },

    block: {
      rules: [
        'paragraph'
      ]
    },

    inline: {
      rules: [
        'text'
      ],
      rules2: [
        'balance_pairs',
        'fragments_join'
      ]
    }
  }
}
