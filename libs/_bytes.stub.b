/**
 * # Working with Binary data
 * 
 * Working with binary data in Blade is a simple as working with _lists_ and this can be 
 * done using `bytes`. A `bytes` (meaning Byte stream) is an iterable in-memory contiguous 
 * list of numbers. Blade allows easily creating _bytes_ from strings and vice-versa and 
 * they are the primary medium of reading and writing binary data into and from the OS.
 * 
 * ## Creating a Byte stream
 * 
 * The built-in `bytes()` function can be used to create a byte stream. For example,
 * 
 * ```blade-repl
 * %> bytes(5)
 * (0 0 0 0 0)
 * ```
 * 
 * The above code creates a byte stream containing five bytes all initialized to zero (0).
 * You can also create byte stream from a list of numbers as well. For example,
 * 
 * ```blade-repl
 * %> bytes([72, 69, 76, 76, 79])
 * (48 45 4c 4c 4f)
 * ```
 * 
 * The above code creates a byte stream sequence corresponding to `HELLO` in memory.
 * 
 * Byte streams are like lists and they can be extended, concatenated and more. They allow
 * us operate on binary data. 
 * 
 * One very important use-case for bytes are to create a file containing binary data, 
 * the `file.write()` API expects to write byte streams to file when the file is opened in 
 * the binary mode and the most basic way of providing it with such data is by using `bytes`.
 * 
 * For example,
 * 
 * ```blade-repl
 * %> file('dummy.txt', 'wb').write(bytes([72, 69, 76, 76, 79]))
 * true
 * %> file('dummy.txt').read()
 * 'HELLO'
 * ```
 * 
 * In the above sample, we wrote the bytes we created earlier into a file and read its contents.
 * 
 * > When a byte stream is created using a list of numbers, the maximum number 
 * > that will be interpreted as is is 255. All numbers exceeding that amount 
 * > will be equal to `(number % 255) - 1`.
 * > 
 * > E.g.
 * > ```blade-repl
 * > %> bytes([256])
 * > (0)
 * > %> bytes([386])
 * > (82)
 * > %> 0x82
 * > 130
 * > %> (386 % 255) - 1
 * > 130
 * > ```
 * 
 * ## Byte stream indexing
 * 
 * Like _Lists_, _Strings_ and _Dictionaries_, Byte streams can also be indexed in Blade. We can 
 * retrieve an index in a byte stream using the same operators as others (`[]`) and we can use this 
 * to access or modify the contents of a byte stream.
 * 
 * For example,
 * 
 * ```blade-repl
 * %> var g = bytes([31, 47, 83, 105, 72])
 * %> g
 * (1f 2f 53 69 48)
 * %> g[3]
 * 105
 * %> g[1,3]
 * (2f 53)
 * %> g[2] = 96
 * %> g
 * (1f 2f 60 69 48)
 * ```
 * 
 * @note Byte streams are printed as hexadecimal lists.
 */


class bytes {

  /**
   * Creates a new byte stream. 
   * 
   * @note If a number is given, creates an array of size number
   * @note If a list is given, converts the bytes list into an array of bytes.
   * 
   * @param {number|list} n The number of bytes or the list of bytes to create.
   * @returns {bytes}
   * @constructor
   * 
   * For example,
   *  
   * ```blade-repl
   * %> bytes(5)
   * (0 0 0 0 0)
   * %> bytes([65, 66, 67, 68, 69])
   * (41  42 43 44 45)
   * ```
   */
  bytes(n) {}

   /**
   * Returns the number of bytes in the byte stream.
   * 
   * ```blade-repl
   * %> bytes([25, 57]).length()
   * 2
   * ```
   * 
   * @returns {number}
   */
  length(n) {}


  /**
   * Adds an item to the top of a byte stream.
   *  
   * For example,
   *  
   * ```blade-repl
   * %> var a = bytes([0x40, 0x75])
   * %> a.append(0x16)
   * %> echo a
   * (40 75 16)
   * ```
   * 
   * @param {number} n The byte to add.
   * @returns {bytes}
   */
  append(n) {}


  /**
   * Returns a deep clone of the byte stream.
   *  
   * For example,
   *  
   * ```blade-repl
   * %> bytes([19, 11]).clone()
   * (13 b)
   * ```
   * 
   * @returns {bytes}
   */
  clone() {}


  /**
   * Extends the byte stream with the bytes from the given byte stream.
   * 
   * For example,
   *  
   * ```blade-repl
   * %> var a = bytes([33, 91, 126])
   * %> var b = bytes([119, 42])
   * %> a
   * (21 5b 7e)
   * %> b
   * (77 2a)
   * %> a.extend(b)
   * (21 5b 7e 77 2a)
   * %> a
   * (21 5b 7e 77 2a)
   * ```
   * 
   * @note `extend()` is an in-place action so the original byte stream will be modified.
   * 
   * @param {bytes} n The byte stream to extend with.
   * @returns {bytes}
   */
  extend(n) {}


  /**
   * Returns the index of the first occurrence of the given byte in the byte stream.
   * 
   * ```blade-repl
   * %> bytes([25, 57, 25]).index_of(57)
   * 1
   * %> bytes([25, 57, 25]).index_of(25, 1)
   * 2
   * ```
   * 
   * @param {number} byte The byte to search for.
   * @param {number} start_index The index to start the search from. Defaults to 0.
   * @returns {number}
   */
  index_of(byte, start_index) {}


  /**
   * Removes the last item in a byte stream and returns it.
   * 
   * ```blade-repl
   * %> var a = bytes([79, 43, 9])
   * %> a.pop()
   * 9
   * %> a
   * (4f 2b)
   * ```
   * 
   * @returns {number}
   */
  pop() {}


  /**
   * Removes the item at the specified index in the byte stream and return the previous value at the specified index.
   * 
   * ```blade-repl
   * %> var a = bytes([25, 57, 25])
   * %> a.remove(1)
   * 57
   * %> a
   * (25 25)
   * ```
   * 
   * @param {number} index The index to remove.
   * @returns {bytes}
   */
  remove(index) {}


  /**
   * Reverses the items in the byte stream.
   * 
   * ```blade-repl
   * %> bytes([5, 4, 3, 2, 1]).reverse()
   * (1 2 3 4 5)
   * ```
   * 
   * @returns {bytes}
   */
  reverse() {}


  /**
   * Returns the first item in the byte stream or `nil` if the byte stream is empty.
   * 
   * ```blade-repl
   * %> bytes([25, 57, 42]).first()
   * 25
   * ```
   * 
   * @returns {number}
   */
  first() {}

  /**
   * Returns the last item in the byte stream or `nil` if the byte stream is empty.
   * 
   * ```blade-repl
   * %> bytes([25, 57, 42]).last()
   * 42
   * ```
   * 
   * @returns {number}
   */
  last() {}


  /**
   * Returns the item at the specified index in the byte stream.
   * 
   * @param {number} index The index to get the item from.
   * @returns {number}
   */
  get(index) {}


  /**
   * Splits the content of a byte stream based on the specified delimiter.
   *  
   * For example,
   *  
   * ```blade-repl
   * %> bytes(0).split(bytes(0))
   * []
   * %> echo 'test'.to_bytes().split(bytes(0))
   * [(74), (65), (73), (74)]
   * ```
   * 
   * @param {bytes} delimiter The delimiter to split on.
   * @returns {list}
   */
  split(delimiter) {}


  /**
   * Due to the nature of byte stream and their use-case (especially streaming data),
   * it is easy for the system memory to get filled up with data in the byte stream.
   * The method allows users to reset a byte stream and empty it.
   *  
   * > This method allows a fine-grained control on manual memory management of byte stream.
   *  
   * For example,
   *  
   * ```blade-repl
   * %> var a = bytes([13, 36])
   * %> a.dispose()
   * %> a
   * ()
   * ```
   */
  dispose() {}


  /**
   * Returns `true` if the byte stream only contains alpha characters, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([65, 66, 67]).is_alpha()
   * true
   * %> bytes([65, 66, 67, 128]).is_alpha()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_alpha() {}


  /**
   * Returns `true` if the byte stream only contains alpha characters and numbers, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([65, 66, 67, 48, 49, 50]).is_alnum()
   * true
   * %> bytes([65, 66, 67, 48, 49, 50, 8]).is_alnum()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_alnum() {}


  /**
   * Returns `true` if the byte stream only contains numbers, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([48, 49, 50]).is_number()
   * true
   * %> bytes([48, 49, 50, 68]).is_number()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_number() {}


  /**
   * Returns `true` if the byte stream only contains lower case characters, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([97, 98, 99]).is_lower()
   * true
   * %> bytes([97, 98, 99, 68]).is_lower()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_lower(n) {}


  /**
   * Returns `true` if the byte stream only contains upper case characters, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([65, 66, 67]).is_upper()
   * true
   * %> bytes([65, 66, 67, 98]).is_upper()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_upper(n) {}


  /**
   * Returns `true` if the byte stream only contains space characters, `false` otherwise.
   *  
   * ```blade-repl
   * %> bytes([32, 32, 32]).is_space()
   * true
   * %> bytes([32, 32, 32, 68]).is_space()
   * false
   * ```
   * 
   * @returns {boolean}
   */
  is_space(n) {}


  /**
   * Returns the byte stream as a list of bytes.
   * 
   * ```blade-repl
   * %> bytes([0x31, 0x55, 0x149, 0x215]).to_list()
   * [49, 85, 233, 33]
   * ```
   * 
   * @returns {list}
   */
  to_list(n) {}


  /**
   * Returns the byte stream as a string.
   * 
   * ```blade-repl
   * %> bytes([65, 66, 67, 68, 69]).to_string()
   * 'ABCDE'
   * ```
   * 
   * @returns {string}
   */
  to_string(n) {}


  @iter(n) {}
  @itern(n) {}
}
