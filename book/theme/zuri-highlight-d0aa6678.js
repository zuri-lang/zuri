/*
 * Syntax highlighting for Zuri, for both books.
 *
 * mdBook ships highlight.js and has already run it by the time this
 * file loads, so this registers the two languages and then highlights
 * the blocks that were skipped on that first pass. An unknown language
 * makes highlight.js fall back to no-highlight and leave the block's
 * text alone, so re-running it here is working on untouched source
 * rather than on something already marked up.
 *
 * Two languages, because a REPL session is not a program:
 *
 *   zuri       a Zuri source file
 *   zuri-repl  a transcript, where only the prompt lines are code and
 *              everything else is output the interpreter printed back
 */
(function () {
  'use strict';

  if (typeof hljs === 'undefined') {
    return;
  }

  hljs.registerLanguage('zuri', function (hljs) {
    // The keyword list is the lexer's own, in `src/compiler/lexer.rs`.
    var KEYWORDS = {
      $pattern: /[A-Za-z_]\w*/,
      keyword:
        'and as assert break catch class const continue def default do echo ' +
        'else for if import in iter or parent raise return self static using ' +
        'var when while',
      literal: 'true false nil',
      built_in:
        'bytes file id instance_of print rand sum time typeof ' +
        'delprop getprop hasprop setprop ' +
        'is_bigint is_bool is_bytes is_callable is_class is_dict is_file ' +
        'is_function is_instance is_int is_iterable is_list is_number ' +
        'is_object is_string'
    };

    var NUMBER = {
      className: 'number',
      variants: [
        { begin: /\b0[bB][01]+/ },
        { begin: /\b0[cC][0-7]+/ },
        { begin: /\b0[xX][0-9a-fA-F]+/ },
        // A bigint wears its `n` suffix, and has to be tried before the
        // plain decimal rule that would otherwise stop just short of it.
        { begin: /\b\d[\d_]*n/ },
        { begin: /\b\d[\d_]*(\.\d[\d_]*)?([eE][+-]?\d+)?/ }
      ],
      relevance: 0
    };

    var SUBST = {
      className: 'subst',
      begin: /\$\{/,
      end: /\}/,
      keywords: KEYWORDS,
      contains: [NUMBER]
    };

    // Both quote styles interpolate, take the same escapes, and may
    // span lines, so neither carries the usual `illegal: /\n/`.
    var STRING = {
      className: 'string',
      variants: [
        { begin: /'/, end: /'/, contains: [hljs.BACKSLASH_ESCAPE, SUBST] },
        { begin: /"/, end: /"/, contains: [hljs.BACKSLASH_ESCAPE, SUBST] }
      ]
    };

    // Interpolation nests through strings: `'${ inner("${x}") }'`.
    SUBST.contains.push(STRING);

    // Zuri block comments nest, which `self` is what expresses: a
    // comment containing `/*` swallows the matching `*/` rather than
    // ending on it. Doc blocks open `/**`, which this already matches.
    var COMMENT = {
      className: 'comment',
      begin: /\/\*/,
      end: /\*\//,
      contains: ['self', { className: 'doctag', begin: /@[a-z_]+\b/ }]
    };

    return {
      name: 'Zuri',
      aliases: ['zu'],
      // Both languages are asked for by name; neither should ever win
      // an auto-detection contest against a block of something else.
      disableAutodetect: true,
      keywords: KEYWORDS,
      contains: [
        COMMENT,
        hljs.HASH_COMMENT_MODE,
        STRING,
        NUMBER,
        // `@new` and the other decorated methods, `@(` opening an
        // anonymous function, and the `@.` of a module path or a
        // re-exporting import.
        { className: 'meta', begin: /@[A-Za-z_]\w*|@(?=[(.])/ },
        {
          beginKeywords: 'class',
          // Stops before `<` and `>` so only the class's own name is
          // titled, not the parent it extends or the class it extends
          // into.
          end: /[{<>]/,
          excludeEnd: true,
          contains: [hljs.UNDERSCORE_TITLE_MODE]
        },
        {
          beginKeywords: 'def',
          end: /[({]/,
          excludeEnd: true,
          contains: [hljs.UNDERSCORE_TITLE_MODE]
        }
      ]
    };
  });

  hljs.registerLanguage('zuri-repl', function () {
    return {
      name: 'Zuri REPL session',
      disableAutodetect: true,
      contains: [
        // `%> ` is the REPL's prompt and `.. ` its continuation, so a
        // line opening with either is code the reader typed. Everything
        // the rest of this definition matches is output; anything it
        // does not match is output too, and stays unstyled, which is
        // what a printed value should look like.
        {
          className: 'meta',
          begin: /^\s*(%>|\.\.)[ ]?/,
          starts: { end: /$/, subLanguage: 'zuri' }
        },
        {
          className: 'deletion',
          begin: /^(Unhandled|Uncaught)\b.*$/
        },
        {
          className: 'deletion',
          begin: /^(StackTrace|Stack trace).*$/
        },
        // The frames under a trace, and the source snippet an error
        // carries: muted, because they are context rather than the
        // message itself.
        {
          className: 'comment',
          begin: /^\s+(at\s|<repl>|-->\s|\d+\s*\||\|)/,
          end: /$/
        }
      ]
    };
  });

  // mdBook highlighted the page before this file loaded, skipping every
  // block whose language it did not know. Those are the ones to revisit.
  var blocks = document.querySelectorAll(
    'pre code.language-zuri, pre code.language-zuri-repl'
  );

  Array.prototype.forEach.call(blocks, function (block) {
    hljs.highlightBlock(block);
  });
})();
