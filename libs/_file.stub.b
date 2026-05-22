/**
 * # The `file` object.
 * 
 * The _file_ object makes it pretty easy to create, read and/or modify files in Zuri and is accessible
 * via the built-in function `file()`.
 * 
 * ```zuri-repl
 * %> file('sample.txt')
 * <file at sample.txt in mode r>
 * ```
 * 
 * The _`file`_ function accepts an optional second parameter specifying the mode in which the file should be opened. 
 * Below is a list of supported modes.
 * 
 * | Mode | Name          | Description |
 * |:----:| --------------|-------------|
 * | `r`  | Read          | Opens the file for reading it's content. The file must exist.  |
 * | `w`  | Write         | Opens the file for writing to it. Creates file it does not exist. |
 * | `a`  | Append        | Opens a file for writing to it starting from the end. This mode ignores any previous mode to `seek()`. Creates file if it does not exist.  |
 * | `r+` | Read/Update   | Opens a file for update. The file must exist. |
 * | `w+` | Write/Update  | Opens a file for update. If the file does not exits, it creates it. Unlike in C and Python, if the file exists, this method does not truncate the file. |
 * | `a+` | Append/Update | Opens a file for updating starting from the end. This mode ignores any previous mode to `seek()` for outputs, but it's input will respect any previous call to `seek()`. Creates file if it does not exist. |
 * 
 * Like in C, while not usually necessary, the modes can be mixed for clarity. For example, the mode `r+w` opens a file for 
 * reading and writing.
 * 
 * ## Reading files
 * 
 * When the file function is called without the mode argument or with mode is set to `r`, the file is opened in the read-only 
 * mode. This is the most effective mode for reading files. 
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> file('sample.txt').read()
 * 'This is a sample file.
 * If you can read this file, then your code worked.'
 * ```
 * 
 * When the length argument is not specifying in a call to the `read()` method, the entire file will be read and the file will 
 * be _automatically_ closed. Otherwise, the file will be read to the specified length, but a `close()` operation will not 
 * occur. A file object _MUST_ be manually closed after reading the entire file by calling the `close()` function.
 * 
 * 
 * ## Writing to files
 * 
 * To write to a file, the file must be opened in a non read-only mode such as `w`, `a`, `r+`. An automatic `flush()` will be 
 * performed after writing to the file and if the entire data was successfully written, the `close()` method will be 
 * automatically called.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> file('test.txt', 'w').write('It works!')
 * true
 * %> file('test.txt').read()  # reading the file to confirm
 * 'It works!'
 * ```
 * 
 * ## Working with binary files
 * 
 * To open a file object in the binary mode for reading and writing binary data such as images, the binary mark (`b`) must 
 * be appended to the mode.
 * 
 * For example:
 * 
 * ```zuri-repl
 * %> var data = bytes([
 * ..   0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x1, 0x0, 0x1, 0x0, 
 * ..   0x80, 0x1, 0x0, 0xff, 0xff, 0xff, 0x0, 0x0, 0x0, 0x21, 
 * ..   0xf9, 0x4, 0x1, 0xa, 0x0, 0x1, 0x0, 0x2c, 0x0, 0x0, 0x0, 
 * ..   0x0, 0x1, 0x0, 0x1, 0x0, 0x0, 0x2, 0x2, 0x4c, 0x1, 0x0, 0x3b
 * ])
 * %> 
 * %> file('sample.gif', 'wb').write(data)
 * true
 * ```
 * 
 * Now check your current directory and you should see the empty but valid gif image courtesy of
 * [Probably Programming](http://probablyprogramming.com/2009/03/15/the-tiniest-gif-ever).
 */


class file {

  /**
   * Create a new file object.
   * 
   * If the mode is not specified, the file is opened in the read-only mode.
   * 
   * Valid modes include:
   * 
   * ```zuri-repl
   * %> file('sample.txt', 'r')
   * <file at sample.txt in mode r>
   * %> file('sample.txt', 'w')
   * <file at sample.txt in mode w>
   * %> file('sample.txt', 'a')
   * <file at sample.txt in mode a>
   * %> file('sample.txt', 'r+')
   * <file at sample.txt in mode r+>
   * %> file('sample.txt', 'w+')
   * <file at sample.txt in mode w+>
   * %> file('sample.txt', 'a+')
   * <file at sample.txt in mode a+>
   * ```
   * 
   * @param {string} path - The path to the file.
   * @param {string?} mode - The mode in which the file should be opened.
   * @constructor
   */
  file(path, mode) {}

  /**
   * Returns `true` if a file exists or `false` otherwise.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').exists()
   * true
   * ```
   * 
   * @return {boolean}
   */
  exists() {}


  /**
   * Closes the stream to an opened file. You'll rarely ever need to call this method yourself in most use cases.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> var f = file('sample.txt')
   * %> f.close()
   * ```
   * 
   * @return {void}
   */
  close() {}


  /**
   * Opens the stream to a file for the operation originally specified on the file object during creation. You may need to 
   * call this method after a call to read() if the length isn't specified or write() if you wish to read or write again as 
   * the file will already be closed.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> f.open()
   * ```
   * 
   * @return {void}
   */
  open() {}


  /**
   * Reads the content of an opened file up to the specified length and returns it as string or bytes if the file was opened 
   * in the binary mode. If the length is not specified, the file will be read to the end.
   * 
   * This method requires that the file be opened in the read mode (default mode) or a mode that supports reading. If you 
   * aren't reading the full length of the file, you'll need to call the `close()` method to free the file for further reading, 
   * otherwise, the `close()` method will be automatically called for you.
   * 
   * _An example has been given above._
   * 
   * @param {int?} length
   * @return {string|bytes}
   */
  read(length) {}


  /**
   * Same as `read()`, but doesn't open or close the file automatically.
   * 
   * @param {int?} length
   * @return {string|bytes}
   */
  gets(length) {}


  /**
   * Writes a string or bytes to an opened file at the current insertion point. When the file is opened with the `a` mode 
   * enabled, write will always start from the end of the file. If the `seek()` method has been previously called, write 
   * will begin from the seeked position, otherwise it will start at the beginning of the file.
   * 
   * _An example has been given above._
   * 
   * @param {string|bytes} data
   * @return {string|bytes}
   */
  write(data) {}


  /**
   * Same as `write()`, but doesn't open or close the file automatically.
   * 
   * @param {string|bytes} data
   * @return {string|bytes}
   */
  puts(data) {}


  /**
   * Returns the integer file descriptor number that is used by the underlying implementation to request I/O operations from 
   * the operating system. This can be very useful for low-level interfaces that uses or act as file descriptors.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').number()
   * 6
   * ```
   * 
   * @return {int}
   */
  number() {}


  /**
   * Returns `true` if the file is connected to a TTY like device or `false` otherwise.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').is_tty()
   * false
   * %> import io
   * %> io.stdout.is_tty()   # io.stdin is a file...
   * true
   * ```
   * 
   * @return {boolean}
   */
  is_tty() {}


  /**
   * Returns `true` if the file is open for reading or writing and `false` otherwise.
   * 
   * > **_@note:_** `std` files are always open.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').is_open()
   * true
   * ```
   * 
   * @return {boolean}
   */
  is_open() {}


  /**
   * Returns `true` if the file is closed for reading or writing and `false` otherwise.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').is_closed()
   * false
   * ```
   * 
   * @return {boolean}
   */
  is_closed() {}


  /**
   * Flushes the buffer held by a file. This could be useful for writable files as file 
   * writes are buffered.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> w.flush()
   * ```
   * 
   * @return {void}
   */
  flush() {}


  /**
   * Returns the statistics or details of a file.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').stats()
   * {is_readable: true, is_writable: true, is_executable: false, is_symbolic: false, size: 72, mode: 33188, dev: 16777230, 
   * ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631395239, atime: 1631395271, ctime: 1631395239, blocks: 8, 
   * blksize: 4096}
   * ```
   * 
   * @return {dict}
   */
  stats() {}


  /**
   * Creates a symbolic link for the original file at the specified path.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').symlink('sample2.txt')
   * true
   * ```
   * 
   * @return {boolean}
   */
  symlink() {}


  /**
   * Deletes a file. 
   * 
   * @note If the file is opened by one or more processes or threads outside of the current process or thread, 
   *    the file will not be deleted until the last process frees it.
   * @note This method throws Exception on failure.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('test-2.b').delete()
   * true
   * ```
   * 
   * @return {boolean}
   */
  delete() {}


  /**
   * Renames a file to to `new_name`. The new name can be a full path in another location in which case the file will 
   * be moved.
   * 
   * @note The new name cannot be empty
   * @note This method throws Exception on failure.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample copy.txt').rename('sample-2.txt')
   * true
   * ```
   * 
   * @param {string} new_name
   * @return {boolean}
   */
  rename(new_name) {}


  /**
   * Returns the path to the file.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').path()
   * 'sample.txt'
   * ```
   * 
   * @return {string}
   */
  path() {}


  /**
   * Returns the absolute path to the file.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').abs_path()
   * 'C:\Users\username\zuri-docs\sample.txt'
   * ```
   * 
   * @return {string}
   */
  abs_path() {}


  /**
   * Copies a file from the path specified in the original file to the given path.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('./sample.txt').copy('samp.txt')
   * true
   * ```
   * 
   * @param {string} new_name
   * @return {boolean}
   */
  copy(path) {}


  /**
   * Truncates the entire file if length is not given or truncates the file such that only length number of bytes
   * is left in it.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('./samp.txt').truncate()
   * true
   * ```
   * 
   * @param {number?} length
   * @return {boolean}
   */
  truncate(length) {}


  /**
   * Changes the permission on the file to the one specified in the number given.
   * 
   * > **_@note:_** The number is required to be an octal number. e.g. 0c755
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').chmod(0c755)
   * true
   * ```
   * 
   * @param {int} mode
   * @return {boolean}
   */
  chmod(mode) {}


  /**
   * Sets the last access time and last modified time of the file.
   * 
   * > **_@note:_** Time is expected in UTC seconds<br>
   * > **_@note:_** set argument -1 to leave the current value.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').set_times(time(), time())
   * true
   * %> file('sample.txt').stats()
   * {is_readable: true, is_writable: true, is_executable: true, is_symbolic: false, size: 72, mode: 33261, 
   * dev: 16777230, ino: 4865113, nlink: 1, uid: 501, gid: 20, mtime: 1631477099, atime: 1631477100, ctime: 
   * 1631477099, blocks: 8, blksize: 4096}
   * ```
   * 
   * @param {number} atime
   * @param {number} mtime
   * @return {boolean}
   */
  set_times(atime, mtime) {}


  /**
   * Sets the position of a file reader or writer in a file. The position must be within the range of the file size. 
   * _seek_type_ must be on of `SEEK_SET`, `SEEK_CUR` or `SEEK_END` from the `io` package.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> f.seek(5, io.SEEK_SET)
   * true
   * ```
   * 
   * @param {number} offset
   * @param {int} seek_type
   * @return {boolean}
   */
  seek(offset, seek_type) {}


  /**
   * Returns the current position of the reader/writer in a file.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> import io
   * %> var f = file('sample.txt')
   * %> f.seek(5, io.SEEK_SET)
   * true
   * %> f.tell()
   * 5
   * ```
   * 
   * @return {number}
   */
  tell() {}


  /**
   * Returns the mode in which the current file was opened.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('sample.txt').mode()
   * 'r'
   * 
   * @return {string}
   */
  mode() {}


  /**
   * Returns the name of the current file.<br>
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> file('./sample.txt').name()
   * 'sample.txt'
   * ```
   * 
   * @return {string}
   */
  name() {}
}
