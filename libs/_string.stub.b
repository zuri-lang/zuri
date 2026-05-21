/**
 * # Strings
 * 
 * Blade has a very rich support for strings and they can be expressed in several ways. In Blade, 
 * strings are denoted by enclosing characters in pairs of single quotes (`'...'`) or pairs of 
 * double quotes (`"..."`) and they are essentially the same.
 * 
 * ## Reference
 * 
 * - [Strings](#strings)
 * - [Reference](#reference)
 * - [More about Strings](#more-about-strings)
 * - [Unicode and UTF-8](#unicode-and-utf-8)
 * - [String Interpolation](#string-interpolation)
 * - [Characters](#characters)
 * - [String Operations](#string-operations)
 * - [String Methods](#string-methods)
 * - [Regular Expressions](#regular-expressions)
 * 
 * 
 * ## More about Strings
 * 
 * When strings are wrapped in `'` or `"`, you can escape that quotation within the string using 
 * the backslash (`\`).
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'string in single quote'
 * 'string in single quote'
 * %> "another version with double quotes"
 * 'another version with double quotes'
 * %> 'What\'s the escape character?'
 * "What's the escape character?"
 * %> "It's the \"\\\" character"
 * "It's the "\" character"
 * ```
 * 
 * > All Blade strings can span multiple lines whether created using single (`'`) or double (`"`) quotes.
 * >
 * > For example:
 * > ```blade
 * > # with single quotes
 * > 'Hello...
 * > World'
 * > 
 * > # same with double quotes
 * > "I am
 * > a living
 * > Legend!"
 * > ```
 * 
 * In the REPL mode, strings are enclosed in quotes based on the kind of data they contains. If a
 * string contains no quote, it is wrapped in single quotes (`'...'`). If it contains a single quote,
 * it is wrapped in double quotes (`"..."`). If it contains both, the latter is used. This is simply
 * for presentation purpose. This is to discourage readers from confusing them with language schematics.
 * 
 * The `print()` function produces a result that is more like what you'd expect. It does no extra 
 * processing of the output.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> print("It's the \"\\\" character")
 * It's the "\" character
 * ```
 * 
 * Blade strings support a lot of special characters called `escape sequence` for formatting and 
 * they also need to be escaped with `\` as follows:
 * 
 * | Sequence | Meaning |
 * |----------|---------|
 * | `\a` | Alert (Beep, Bell) |
 * | `\b` | Backspace |
 * | `\f` | Form-feed Page Break |
 * | `\n` | Newline |
 * | `\r` | Carriage Return |
 * | `\t` | Horizontal Tab |
 * | `\v` | Vertical Tab |
 * | `\\` | Backslash |
 * | `\'` | Single Quotation Mark |
 * | `\"` | Double Quotation Mark |
 * | `\0` | String terminator |
 * | `\$` | Escape for interpolated strings |
 * | `\xhh` | Hexadecimal number |
 * | `\uhhhh` | Unicode code point below 10000 hexadecimal |
 * | `\Uhhhhhhhhh` | Unicode code point where h is a hexadecimal digit |
 * 
 * > 1. _`h`_ stands for hexadecimal digit.
 * > 2. `\0` used anywhere in a string will cause the rest of the string to be ignored and useless.
 * > 3. `\$` _See the [`Interpolated Strings`](#string-interpolation) section below._
 * > 4. `\u` takes 4 hexadecimal digits _h_ after it.
 * > 5. `\U` takes 8 hexadecimal digits _h_ after it.
 * 
 * 
 * ## Unicode and UTF-8
 * 
 * As mentioned in the previous section, Blade strings fully supports unicode and are UTF-8 encoded by default.
 * Unicode code points can be represented using Unicode `\u` and `\U` escape sequences.
 * 
 * For example,
 * 
 * ```blade-repl
 * %> '\u00a9'
 * '©'
 * %> '10\u00B5s'
 * '10µs'
 * %> '\U0002B695 is a chinese character'
 * '𫚕 is a chinese character'
 * ```
 * 
 * Unicode characters can also be written directly in strings. This means, that in a Blade string, you can
 * actually use advanced texts like smilies, trademarks and many more directly in your source code.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'I am 😀'
 * 'I am 😀'
 * %> 'Black ♞ rule'
 * 'Black ♞ rule'
 * %> '道可道非常道，名可名非常名'
 * '道可道非常道，名可名非常名'
 * ```
 * 
 * To verify our UTF-8 support, how about we try to get the length of the chinese string `名可名非常名`. 
 * With UTF-8 support, the length of this string should be six (6) and much longer without UTF-8 support. 
 * To get the length of the string, we can call it's `length()` method.
 * 
 * For example,
 * 
 * ```blade-repl
 * %> '名可名非常名'.length()
 * 6
 * %> 'Hello, World'.length() # compared with English text
 * 12
 * ```
 * 
 * As you can see, Blade returns the correct length irrespective of the language of the source text.
 * 
 * 
 * ## String Interpolation
 * 
 * As we write more code, we seldom find ourselves needing to join two strings together or at other times,
 * join a string to a declared variable. Some other times, we want to have the result of an operation or 
 * expression within our string. All of these operations can soon become pretty verbose and tedious. Blade
 * allows _interpolation_ into string literals using the `$` character just as can be seen in Perl and Dart.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'Sum after addition = ${10 + 15}'
 * 'Sum after addition = 25'
 * ```
 * 
 * The above example shows the general construct for string interpolation. Start interpolation with the 
 * dollar (`$`) character, and wrap the interpolated expression within curly braces `{}` as shown above.
 * 
 * For another example, let's say we have two variables `x` and `y` declared as `Number` and `String` 
 * respectively and we want to have them _concatenated_ to our string at some location, we can have 
 * something like the following:
 * 
 * ```blade-repl
 * %> 'We have ${x} crates of ${y}'
 * 'We have 20 crates of eggs'
 * ```
 * 
 * To write the interpolation expression within a string without interpreting it, we need to escape the `$`
 * with a backslash (`\`) as follows:
 * 
 * ```blade-repl
 * %> 'Sample interpolation: \${x * y}'
 * 'Sample interpolation: ${x * y}'
 * ```
 * 
 * ## Characters
 * 
 * In Blade, _Characters_ are essentially strings with a length of one (1). No more, no less! However, there are 
 * times when we require Characters over Strings. For example, the builtin `ord()` function expects a character
 * and not a String. While this distinction looks thin, it is a very important distinction that must be put
 * to heart.
 * 
 * Characters are also UTF-8 compliant.
 * 
 * The sample code below shows an example of the clear distinctive use of characters and strings.
 * 
 * ```blade-repl
 * %> echo ord('A')
 * 65
 * %> echo ord('AB')
 * Unhandled Exception: ord() expects character as argument, string given
 * StackTrace:
 *   <repl>:1 -> @.script()
 * ```
 * 
 * A more complex example that skips a lot into the future of this tutorial is given as below for reference.
 * 
 * ```blade-repl
 * %> import types
 * %> types.char('a')
 * true
 * %> types.char('ab')
 * false
 * %> types.char('地')
 * true
 * %> types.char('地点')
 * false
 * ```
 * 
 * > Characters are always interchangeable for strings, but not the reverse.
 * 
 * ## String Operations
 * 
 * Blade strings support multiple operations categorized into one of the following four groups.
 * 
 * Two or more strings can be concatenated (glued together) via the `+` operator whether it's a literal or 
 * a variable, and a specific string can be repeated by multiplying it with a number via the `*` operator. 
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'str' + 'ing'
 * 'string'
 * %> 'abc' * 4 # repeating 'abc' four times
 * 'abcabcabcabc'
 * %> 'hat!' * 4 + 'rick' # and even in a more complex form
 * 'hat!hat!hat!hat!rick'
 * ```
 * 
 * The `+` operator is quite powerful with a string, allowing you to add a string to a number or a number 
 * to a string.
 * 
 * For example,
 * 
 * ```blade-repl
 * %> 5 + 'alive'
 * '5alive'
 * %> 'Base' + 64
 * 'Base64'
 * ```
 * 
 * Strings can also be checked for equality or inequality as needed. For example:
 * 
 * ```blade-repl
 * %> "abracadabra" == "xylophone"
 * false
 * %> "Hello, world." != "Goodbye, world."
 * true
 * %> "1 + 2 = 3" == "1 + 2 = ${1 + 2}"
 * true
 * ```
 * 
 * Strings indexes can be accessed. The first character of a Blade string have an index of `0`. The result of 
 * string indexes are characters.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'Hello'[0]
 * 'H'
 * %> 'Hello'[3]
 * 'l'
 * ```
 * 
 * Strings indexes can also be accessed with negative numbers. When using negative numbers to access string
 * indexes, note that the indexes will be returned in reverse. i.e. we start counting from the far right
 * where the first index will be `-1` (since -0 is the same as 0).
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'Hello'[-1]
 * 'o'
 * %> 'Hello'[-4]
 * 'e'
 * ```
 * 
 * Note that trying to access a non-existing index or an index out of the range of the length of the string
 * will result in an error.
 * 
 * For example, the following code throws an exception.
 * 
 * ```blade-repl
 * %> 'Hello'[6]
 * Unhandled Exception: string index 6 out of range
 * StackTrace:
 *   <repl>:1 -> @.script()
 * ```
 * 
 * In addition to indexing, slicing is also supported. While indexing is used to obtain individual characters, slicing allows you to obtain subparts of a string.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'Blade'[0,3] # characters starting from index 0 to index 3 - 1 (2)
 * 'Bla'
 * %> 'Blade'[2,5] # characters from index 2 to index 5 - 1 (4)
 * 'ade'
 * ```
 * 
 * The general syntax for slicing in Blade is `[lower limit, upper limit]`. Both lower limit and upper limit
 * can be omitted. When the lower limit is omitted, it defaults to `0` and when the upper limit is omitted,
 * it defaults to the length of the object e.g. the string length.
 * 
 * As with general indexing, the upper limit can also use negative numbers and follows the same rules as 
 * indexing with a negative number. A negative number in the lower limit will cause an empty object to be 
 * returned.
 * 
 * > 1. Slices are lower limit inclusive and upper limit exclusive. For example, slice `[0,3]` will return
 * > a substring starting from index `0` (inclusive) to index `2` and index itself will be excluded.
 * > 2. Index `in[,i] + in[i,]` is equal to the value of `in`. 
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'Blade'[0,3]  # starting from index 0 to 2
 * 'Bla'
 * %> 'Blade'[2,5]  # starting from index 2 to 4
 * 'ade'
 * %> 'Blade'[,]   # starting from index 0 to the end
 * 'Blade'
 * %> 'Blade'[,-3]   # starting from index 0 to string length - 3
 * 'Bl'
 * %> 'Blade'[3,]  # starting from index 3 to the end
 * 'de'
 * %> 'Blade'[-1,]  # negative index in lower limit returns an empty string
 * ''
 * %> 'Blade'[,4]   # starting from index 0 to 3
 * 'Blad'
 * %> 'Blade'[,3] + 'Blade'[3,]     # in[,i] + in[i,]
 * 'Blade'
 * ```
 * 
 * Blade strings are immutable. Hence, a string cannot be changed. Assigning to an indexed position in the string results in an error:
 * 
 * For example,
 * 
 * ```blade-repl
 * > 'Blade'[0] = 'J'
 * Unhandled Exception: strings do not support object assignment
 * StackTrace:
 *   <repl>:1 -> @.script()
 * ```
 * 
 * > You may notice how we are trying to assign to a string object directly instead of a variable and think
 * > that's why it isn't working. That's not why! In blade, if string wasn't immutable (e.g. Lists aren't 
 * > immutable), Blade will go ahead and do that assignment. The fact that you aren't storing that value 
 * > anywhere is up to you. But it's neither a syntax nor runtime error to do so.
 * 
 * If you need to to modify a string, you need to create a new one. Don't worry, Blade is smart enough to 
 * know when you don't need a string anymore and will gracefully delete the string for memory when necessary.
 * 
 * ## Regular Expressions
 * 
 * Regular expressions in Blade are simply special patterns expressed in a string following a few guidelines
 * that allow them to be distinguished by methods requiring them. We'll be using the term `regex` or `regexes` 
 * henceforth for the rest of this tutorial and most likely for the rest of the documentation.
 * 
 * Blade's _regex_ is built on-top the _PCRE2_ library, an excellent library that already powers regular 
 * expression in many programming languages and have been around for decades. It feels like a better choice
 * for now for Blade to depend on this library rather than invest years building one robust enough to match
 * the library's capabilities.
 * 
 * In simple words, Blade's _regex_ is PCRE compatible.
 * 
 * To create a valid _regex_ in Blade, **your regex pattern must be surrounded by identical non-word characters**.
 * 
 * For example, `/\d+/`. Note here how we surround our pattern `\d+` with forward slashes (`/`).
 * 
 * This tutorial will not attempt to teach regular expressions as there are many wonderful texts already
 * written on that topic as well as many online tools for learning them in greater depths than we can cover 
 * in this tutorial. Majority of them based on the same engine we are using. So here's 
 * [one of them](https://regexr.com/)for your reference.
 * 
 * Most languages support different modifiers for regular expressions, and Blade has some too. Modifiers
 * are placed after a valid regex to control how the pattern is executed by the language. For example, in
 * Blade, the pattern `/[a-z]/i` is a pattern modified with the `i` modifier telling the interpreter to
 * make sure the matching is done case insensitive.
 * 
 * The following table lists Blade modifiers.
 * 
 * | Modifier | Definition |
 * |----------|------------|
 * | `i` | Case insensitive matching |
 * | `m` | Multi-line. This mode cases `^` and `$` to match newlines. |
 * | `s` | Dot (`.`) matches all |
 * | `x` | Extended matching (Ignore whitespace and `#` comments) |
 * | `A` | Force pattern anchoring |
 * | `D` | Do not match newline at the end. In this mode, `$` will be the only valid line terminator. |
 * | `U` | Ungreedy match. |
 * | `u` | Treat pattern and subjects as UTF strings and use Unicode properties for \d, \w, etc. |
 * | `J` | Allow duplicate names for subpatterns |
 * 
 * 
 * 
 * Modifiers can be joined together to form a more powerful modification instruction. For example, you
 * can perform a multi-line and case-insensitive modification for our former sample as `/[a-z]/mi`.
 * 
 * For example:
 * 
 * ```blade-repl
 * %> 'The side bar includes a Cheatsheet'.matches('/([A-Z])\w+/')
 * {0: [The, Cheatsheet], 1: [T, C]}
 * ```
 * 
 * Or the same query with modifiers,
 * 
 * ```blade-repl
 * %> 'The side bar includes a Cheatsheet'.matches('/([A-Z])\w+/sim')
 * {0: [The, side, bar, includes, Cheatsheet], 1: [T, s, b, i, C]}
 * ```
 */

class string {

  /**
   * Returns the length of a string. Note that this method is UTF-8 compatible and will return the UTF-8
   * length for the string if the string contains UTF-8 characters whether written directly or via the
   * `\u` or `\U` escapes.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'This is a pretty long string'.length()
   * 28
   * %> 'उनका एक समय'.length()
   * 11
   * %> 'This text mixes English and 粵語'.length()
   * 30
   * ```
   * 
   * @returns {number}
   */
  length() {}


  /**
   * Returns a copy of the string with all the cased characters converted to uppercase. Note that the result
   * of this method may return `false` when tested with `is_upper()` of the _string_ contains Unicode
   * characters that are not case folded.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'blade'.upper()
   * 'BLADE'
   * ```
   * 
   * @returns {string}
   */
  upper() {}


  /**
   * Return a copy of the string with all the cased characters converted to lowercase.<br>
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'Blade Is Bae'.lower()
   * 'blade is bae'
   * ```
   * 
   * @returns {string}
   */
  lower() {}

  /**
   * Returns `true` if all the characters in the string are all alphabets and the string is not empty.,
   * otherwise returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'abracadabra'.is_alpha()
   * true
   * %> 'my tooth aches'.is_alpha()
   * false
   * %> ''.is_alpha()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_alpha() {}


  /**
   * Returns `true` if all the characters in the string are either alphabets or numbers and the string is
   * not empty, otherwise returns `false`. This method is the same as `string.is_alpha() or string.is_number()`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '3Idiots'.is_alnum()
   * true
   * %> 'Three Idiots'.is_alnum()
   * false
   * %> '3 Idiots'.is_alnum()
   * false
   * %> '3'.is_alnum()
   * true
   * %> 'idiots'.is_alnum()
   * true
   * %> ''.is_alnum()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_alnum() {}


  /**
   * Returns `true` if all the characters in the string are all digits and the string is not empty,
   * otherwise returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '123.5'.is_number()
   * false
   * %> '1970'.is_number()
   * true
   * %> '1980s'.is_number()
   * false
   * ```
   */
  is_number(n) {}


  /**
   * Returns `true` if at least one character in the string is cased, all cased characters are lower cased
   * and the string is not empty. Otherwise, it returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'all'.is_lower()
   * true
   * %> 'all...123'.is_lower()
   * true
   * %> 'All...123'.is_lower()
   * false
   * %> ''.is_lower()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_lower() {}


  /**
   * Returns `true` if at least one character in the string is cased, all cased characters are upper cased
   * and the string is not empty. Otherwise, it returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'ALL'.is_upper()
   * true
   * %> 'ALL...123'.is_upper()
   * true
   * %> 'All...123'.is_upper()
   * false
   * %> ''.is_upper()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_upper() {}


  /**
   * Returns `true` if there are only whitespace characters in the string and the string is not empty.
   * Otherwise, it returns empty.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '.     '.is_space()
   * false
   * %> '\r\n'.is_space()
   * true
   * %> '\t  '.is_space()
   * true
   * ```
   * 
   * @returns {boolean}
   */
  is_space() {}


  /**
   * Returns a copy of the string with the given character (_`chr`_) removed if it appears at the start or 
   * end of the string. If _`chr`_ is not given, it defaults to a space (`' '`). All matching leading and 
   * trailing characters are removed until a character that doesn't match is encountered. If no match is 
   * found, a copy of the original string is returned.
   * 
   * > The square brackets (`[]`) around the _`chr: char`_ in the method definition indicates that the 
   * > parameter is optional and does not mean you have to type the square brackets.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '  example  '.trim()
   * 'example'
   * %> '  example  '.trim('e')
   * '  example  '
   * %> 'example'.trim('e')
   * 'xampl'
   * ```
   * 
   * @param {char?} chr - The character to trim (Default = ' ').
   * @returns {string}
   */
  trim(chr) {}


  /**
   * Similar to the `trim()` method, except that this method only removes characters at the beginning of
   * the string.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '  example  '.ltrim()
   * 'example  '
   * %> 'example'.ltrim('e')
   * 'xample'
   * ```
   * 
   * @param {char?} chr - The character to trim (Default = ' ').
   * @returns {string}
   */
  ltrim(chr) {}


  /**
   * Similar to the `trim()` method, except that this method only removes characters at the end of 
   * the string.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '  example  '.rtrim()
   * '  example'
   * %> 'example'.rtrim('e')
   * 'exampl'
   * ```
   * 
   * @param {char?} chr - The character to trim (Default = ' ').
   * @returns {string}
   */
  rtrim(chr) {}


  /**
   * Returns a string which is a concatenation of the items in the iterable using the _string_ as the
   * separator. If the iterable contains just one item or the _string_ is empty, the original element 
   * is returned. If the _iterable_ contains non-string items, the items are converted to their string 
   * representation before joining.
   * 
   * `Bytes` are the only non supported iterables.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> ','.join(['ok', 1, true])
   * 'ok,1,true'
   * %> '--'.join('name')
   * 'n--a--m--e'
   * %> ','.join('a')
   * 'a'
   * ```
   * 
   * @param {string} string - The string to join the items in the iterable.
   * @returns {string}
   */
  join(string) {}


  /**
   * Returns a list of words or characters in a string after separating the content of the string at every
   * point where the _delimiter_ is found. 
   * 
   * If the _delimiter_ is an empty string, the resultant list will contain the individual characters of 
   * the string in the order in which they appear in the original string. Consecutive delimiters are not 
   * grouped together and are deemed to delimit empty strings. Splitting an empty string with a specified 
   * separator returns an empty list.
   * 
   * This method has full UTF-8 support.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'name'.split('')
   * [n, a, m, e]
   * %> '1<>2<>3'.split('<>')
   * [1, , 2, , 3]
   * %> '1,2,3'.split(',')
   * [1, 2, 3]
   * %> ''.split(',')
   * []
   * %> '地点'.split('')
   * [地, 点]
   * %> 'who is in the garden'.split('/\s/')
   * [who, is, in, the, garden]
   * ```
   * 
   * @param {string} delimiter - The delimiter to use the split the string.
   * @returns {list}
   */
  split(delimiter) {}


  /**
   * Returns the index position of the first occurrence of the string _`str`_ in the string _`string`_. If
   * the _str_ cannot be found anywhere in _string_, it returns -1. If the `start_index` parameter is given, 
   * it will start scanning from the given index.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'hello, world'.index_of(' ')
   * 6
   * %> 'hello, world'.index_of('e')
   * 1
   * %> 'hello, world'.index_of('q')
   * -1
   * %> 'hello, world'.index_of('o')
   * 4
   * %> 'hello, world'.index_of('o', 5)  # next index of `o` starting from index 5.
   * 8
   * ```
   * 
   * @param {string} str - The string to search for.
   * @param {number?} start_index - The index to start the search from.
   * @returns {number}
   */
  index_of(str, start_index) {}


  /**
   * Returns `true` if the string begins with the string or character specified in _str_, otherwise 
   * it returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'hello, world'.starts_with('hello')
   * true
   * %> 'hello, world'.starts_with('hellios')
   * false
   * ```
   * 
   * @param {string} str - The string to search for.
   * @returns {boolean}
   */
  starts_with(str) {}


  /**
   * Returns `true` if the string ends with the string or character specified in _str_, otherwise 
   * it returns `false`.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'gumtree'.ends_with('tree')
   * true
   * %> 'gumtree'.ends_with('mree')
   * false
   * ```
   * 
   * @param {string} str - The string to search for.
   * @returns {boolean}
   */
  ends_with(str) {}


  /**
   * Returns the number of non-overlapping occurrences of the substring _str_ in the string. 
   * 
   * _For those coming from Python who may consider this method similar to Python's own, this 
   * method differs in that it does not allow specifying a start and end region for the operation. 
   * Blade considers this unnecessary as the same can be accomplished by slicing the string._
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'Hallelujah'.count('l')
   * 3
   * %> 'ding dong'.count('ng')
   * 2
   * %> 'ding dong'[2,7].count('ng') # setting region to search for counts - 'ng do'
   * 1
   * ```
   * 
   * @param {string} str - The string to search for.
   * @returns {number}
   */
  count(str) {}


  /**
   * Returns the first numeric value contained in the string if any exists or `0` if the string
   * contains no numeric value. Floating numbers that have the same value as their integer counterparts
   * will return the integer value.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '123.0 hell'.to_number()
   * 123
   * %> '427 and 12'.to_number()
   * 427
   * %> '96.3 of 31'.to_number()
   * 96.3
   * %> 'error'.to_number()
   * 0
   * ```
   * 
   * @returns {number}
   */
  to_number() {}


  /**
   * Returns a list whose elements consists of every character contained in the string in order of
   * appearance. Characters that repeat in the string will have different entries in the same index
   * as they appear in the string.<br>
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'Blade'.to_list()
   * [B, l, a, d, e]
   * %> 'Plantation'.to_list()
   * [P, l, a, n, t, a, t, i, o, n]
   * ```
   * 
   * @returns {list}
   */
  to_list() {}


  /**
   * Returns the content of the string as a stream of `bytes`. 
   * 
   * > The Blade REPL _may_ truncate long bytes data when printing to console/terminal.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'Blade'.to_bytes()
   * (42 6c 61 64 65)
   * %> 'Plantation'.to_bytes()
   * (50 6c 61 6e 74 61 74 69 6f 6e)
   * ```
   * 
   * @returns {bytes}
   */
  to_bytes() {}


  /**
   * Returns the string left justified in a string of length _width_. Padding is done using the specified 
   * character _fill_ if given of a space (`' '`) if a _fill_ is not specified. The original string is 
   * returned if width is less than _`string.length()`_.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'cat'.lpad(5)
   * '  cat'
   * %> 'cat'.lpad(5, '-')
   * '--cat'
   * %> 'cat'.lpad(2, '-')
   * 'cat'
   * ```
   * 
   * @param {number} width - The length of the string after padding.
   * @param {string?} fill - The character to use for padding.
   * @returns {string}
   */
  lpad(width, fill) {}


  /**
   * Returns the string right justified in a string of length _width_. Padding is done using the specified 
   * character _fill_ if given of a space (`' '`) if a _fill_ is not specified. The original string is 
   * returned if width is less than _`string.length()`_.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'Hmm'.rpad(6)
   * 'Hmm   '
   * %> 'Hmm'.rpad(6, '.')
   * 'Hmm...'
   * %> 'Hmm'.rpad(3, '.')
   * 'Hmm'
   * ```
   * 
   * @param {number} width - The length of the string after padding.
   * @param {string?} fill - The character to use for padding.
   * @returns {string}
   */
  rpad(width, fill) {}


  /**
   * If the string _str_ is a regular string, this method returns `true` if the _string_ contains a
   * substring _str_. Otherwise, it returns `false`.
   * 
   * If the string _str_ contains a valid [regular expression](#regular-expressions) (we'll get to
   * that shortly below), it returns `false` if a match for the regex _str_ cannot be found in the
   * string. Otherwise, it returns a [dictionary](./dictionaries) containing all first matching
   * substring.
   * 
   * If the _offset_ argument is specified, it becomes the offset in the _string_ at which to start matching.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'gorilla'.match('go')      # regular string match
   * true
   * %> 'gorilla'.match('gox')     # regular string non-match
   * false
   * %> 'gorilla'.match('/gox?/')  # regular expression match
   * {0: go}
   * %> 'gorilla'.match('/gox\d/') # regular expression non-match
   * false
   * ```
   * 
   * @param {string} str - The string to match.
   * @returns {boolean|dictionary}
   */
  match(str) {}


  /**
   * Returns a dictionary containing every match of the given regular expression _reg_ in the source string.
   * If no match is found, an empty [dictionary](./dictionaries) is returned.
   * 
   * If the _offset_ argument is specified, it becomes the offset in the _string_ at which to start matching.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> '123 dollars'.matches('/[a-z]+|\d+/')
   * {0: [123, dollars]}
   * %> 'who is in the garden'.matches('/\w+/')
   * {0: [who, is, in, the, garden]}
   * ```
   * 
   * @param {string} reg - The regular expression to match.
   * @returns {dictionary}
   */
  matches(reg) {}


  /**
   * Returns a copy of the string with all occurrences or matches of _str_ replaced by the _replacement_
   * string.
   * 
   * In the _replacement_ string, if _str_ is a regular expression, then capture groups can be referenced
   * using the syntax `$index`. Taking as an example, capture group `0` contains the entire match and 
   * can be used in the _replacement_ string as `$0`.
   * 
   * > To escape the `$` sign in the _replacement_ string, use the double backslashes (`\\`).
   * 
   * For example:
   * 
   * ```blade-repl
   * %> 'lady friend'.replace('d', 'z')  # non-regex
   * 'lazy frienz'
   * %> 'John is 26 years old'.replace('/(\d+)/', '1$1') # regex example
   * 'John is 126 years old'
   * %> 'John is 26 years old'.replace('/(\d+)/', '1\\$2')
   * 'John is 1$2 years old'
   * ```
   * 
   * @note When the third parameter _`use_regex`_ is set to false, _str_ will never be treated as a regular
   *    expression even if it contains a valid regular expression.
   * @param {string} str - The string to match.
   * @param {string} replacement - The replacement string.
   * @param {boolean} use_regex - Whether to use the regular expression or the string string as the match 
   *    string (default = true).
   * @returns {string}
   */
  replace(str, replacement, use_regex) {}


  /**
   * Returns a copy of the string with all occurrences or matches of *regex* replaced with the result
   * of the function *callback* which is invoked only if and after a match has occurred.
   * 
   * The callback function is defined as follows:
   * 
   * ```blade
   * def replacer(match, p1, p2, /* …, */ pN, offset, string) {
   *   return replacement
   * }
   * ```
   * 
   * The arguments to the function are as follows:
   * 
   * - `match`: The matched substring. (Corresponds to `$0`.)
   * 
   * - `p1, p2, …, pN`: The nth string found by a capture group (including named capturing groups) 
   *   corresponds to `$1`, `$2`, etc. For example, if the pattern is `/(\a+)(\b+)/`, then `p1` is 
   *   the match for `\a+`, and `p2` is the match for `\b+`. If the group is part of a disjunction 
   *   (e.g. `"abc".replace_with('/(a)|(b)/', replacer)`), the unmatched alternative will be `nil`.
   * 
   * - `offset`: The offset of the matched substring within the whole string being examined. 
   *   For example, if the whole string was `'abcd'`, and the matched substring was `'bc'`, then 
   *   this argument will be `1`.
   * 
   * - `string`: The whole string being examined.
   * 
   * The exact number of arguments depends on how many capture groups are contained in the regex.
   * 
   * For example:
   * 
   * ```blade-repl
   * %> echo 'name'.replace_with('/m/', @(match, offset) {
   * ..   return match + '-'
   * .. })
   * 'nam-e'
   * ```
   * 
   * Below is another example that uses a capture group:
   * 
   * ```blade-repl
   * %> var text = 'all is well'
   * %> 
   * %> echo text.replace_with('/([a-z]+)/', @(match, val) {
   * ..   if val == 'is' return 'is not'
   * ..   return 'will be'
   * .. })
   * 'will be is not will be'
   * ```
   * 
   * @param {string} regex - The regular expression to match.
   * @param {function} callback - The callback function to invoke for each match.
   * @returns {string}
   */
  replace_with(regex, callback) {}


  /**
   * Sends the string into the ASCII mode.
   * 
   * In the ASCII mode, the string operates as an ASCII character set instead of a unicode string. This is 
   * especially useful for strings that were converted from byte streams into ASCII mode allows you to access 
   * the full range of the string irrespective of the NULL terminators in the string (i.e. beyond the length 
   * of the string as far as the length of the original byte stream where it originated from).
   * 
   * @returns {string}
   */
  ascii() {}


  /**
   * Convert a string into a standardized lowercase format while respecting the correct and valid Unicode 
   * defined lowercase equivalent to the original string.
   * 
   * @returns {string}
   */
  case_fold() {}


  /**
   * Compares the string with another string.
   * 
   * @param {string} other - The other string to compare with.
   * @throws {Exception} if the other string is not a string.
   * @returns {number}
   *    - A negative number if the string is less than the other string.
   *    - Zero if the strings are equal.
   *    - A positive number if the string is greater than the other string.
   */
  compare(other) {}

  @iter(n) {}
  @itern(n) {}
}
