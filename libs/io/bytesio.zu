# io.BytesIO implementation

/**
 * The BytesIO class implements a bytearray based I/O system 
 * that allows you use treat bytearray (bytes) as if they were 
 * a file.
 * 
 * The class implements the essentials of a file except those 
 * that ties it to the operating system filesystem such as 
 * symbolic links, chmod and set time.
 * 
 * See the tutorial on [working with files](/tutorial/working-with-files) 
 * for more information.
 */
class BytesIO {
  # trackers
  var _open = true
  var _number = 0
  var _max_length = 0
  var _position = 0

  /**
   * Returns a new instance of BytesIO
   * 
   * @param bytes source
   * @param string mode: The I/O open mode - Default is `r`
   * @constructor
   */
  BytesIO(source, mode) {
    if !mode mode = 'r'

    if !is_bytes(source) {
      raise TypeError('BytesIO must take source from a bytearray')
    } else if !is_string(mode) {
      raise ValueError('invalid I/O mode')
    }

    self.source = source
    self._mode = mode
    self._number = rand(0, 65535)
    self._max_length = self.source.length()

    self._ctime = time()
    self._atime = 0
    self._mtime = 0
  }

  /**
   * Returns `true` as BytesIO always exist.
   * 
   * @returns bool
   */
  exists() {
    return true
  }

  /**
   * Closes the stream to an opened BytesIO. You'll rarely ever 
   * need to call this method yourself in most use cases.
   */
  close() {
    self._do_close()
  }

  /**
   * Opens the stream to a BytesIO for the operation originally 
   * specified on the BytesIO object during creation. 
   * 
   * You may need to call this method after a call to `read()` 
   * if the length isn't specified or `write()` if you wish to 
   * read or write again as the BytesIO will already be closed.
   */
  open() {
    return self._do_open()
  }

  /**
   * Reads the content of an opened BytesIO up to the specified length 
   * and returns it as string or bytes if the BytesIO was opened in the 
   * binary mode. If the length is not specified, the BytesIO will be 
   * read to the end.
   * 
   * This method requires that the BytesIO be opened in the read mode 
   * (default mode) or a mode that supports reading. If you aren't 
   * reading the full length of the BytesIO, you'll need to call the 
   * close() method to free the BytesIO for further reading, otherwise, 
   * the close() method will be automatically called for you.
   * 
   * @param number length: Default = -1
   * @returns bytes
   * @throws Exception
   */
  read(length) {
    if !length {
      length = self._max_length - self._position
    }

    if !is_number(length) {
      raise TypeError('length must be a number')
    }

    if !self._open {
      self._do_open()
    }

    var result = self._read(length)
    self._do_close()
    return result
  }

  /**
   * Same as `read()`, but doesn't open or close the BytesIO automatically.
   * 
   * @param number length: Default = -1
   * @returns bytes
   * @throws Exception
   */
  gets(length) {
    if !length {
      length = self._max_length - self._position
    }

    if !is_number(length) {
      raise TypeError('length must be a number')
    }

    return self._read(length)
  }

  /**
   * Writes a string or bytes to an opened BytesIO at the current insertion 
   * point. When the BytesIO is opened with the a mode enabled, write will 
   * always start from the end of the BytesIO. 
   * 
   * If the seek() method has been previously called, write will begin 
   * from the seeked position, otherwise it will start at the beginning 
   * of the BytesIO.
   * 
   * @param bytes|string
   * @returns number
   */
  write(data) {
    if !is_bytes(data) and !is_string(data) {
      raise TypeError('string or bytes expected')
    }

    if is_string(data) data = data.to_bytes()

    if !self._open {
      self._do_open()
    }

    return self._write(data)
  }

  /**
   * Same as `write()`, but doesn't open or close the BytesIO automatically.
   * 
   * @param bytes|string
   * @returns number
   */
  puts(data) {
    if !is_bytes(data) and !is_string(data) {
      raise TypeError('string or bytes expected')
    }

    if is_string(data) data = data.to_bytes()

    return self._write(data)
  }

  /**
   * Returns the integer file descriptor number that is used by the 
   * underlying implementation to request I/O operations from the 
   * operating system. This can be very useful for low-level interfaces 
   * that uses or act as BytesIO descriptors.
   * 
   * @returns number
   */
  number() {
    return self._number
  }

  /**
   * Always returns `false` as a BytesIO is not a TTY device.
   * 
   * @returns bool
   */
  is_tty() {
    return false
  }

  /**
   * Returns `true` if the BytesIO is open for reading or writing and 
   * `false` otherwise.
   * 
   * @returns bool
   */
  is_open() {
    return self._open
  }

  /**
   * Returns `true` if the BytesIO is closed for reading or writing and 
   * `false` otherwise.
   */
  is_closed() {
    return !self._open
  }

  /**
   * Does nothing for a BytesIO
   */
  flush() {
    # do nothing...
  }

  /**
   * Does nothing for BytesIO but simply returns `false` because 
   * BytesIO cannot be symbolically linked.
   * 
   * @returns bool
   */
  symlink(path) {
    return false
  }

  /**
   * Returns the statistics or details of the BytesIO.
   * 
   * See the working with files documentation for more information 
   * about the `stats()` method.
   * 
   * @returns dict
   */
  stats() {
    return {
      is_readable: self._can_read(),
      is_writable: self._can_write(),
      is_executable: self._can_read() and !self._can_write(),
      is_symbolic: false,
      size: self.source.length(),
      mode: 0,
      dev: 0,
      ino: 0,
      nlink: 0,
      uid: 0,
      gid: 0,
      mtime: self._mtime,
      atime: self._atime,
      ctime: self._ctime,
      blocks: max(self.source.length() // 4096, 1),
      blksize: 4096,
    }
  }

  /**
   * Clears the bytearray and closes it for reading or writing.
   * 
   * Any further attempt to perform most operations on the BytesIO 
   * after calling `delete()` will raise an exception.
   * 
   * @returns bool
   */
  delete() {
    self.source.dispose()
    self._do_close()
    self.source = nil
    self._max_length = -1
    self._mtime = -1
    self._position = -1
    self._number = -1
    self._mode = nil
    self._ctime = -1
    self._mtime = -1
    self._atime = -1
    return true
  }

  /**
   * Returns `false` because BytesIO cannot be renamed.
   * 
   * @returns bool
   */
  rename(new_name) {
    return false
  }

  /**
   * Returns a new BytesIO with the source cloned and opened with 
   * the same mode as the current BytesIO.
   * 
   * @returns [[io.BytesIO]]
   */
  copy() {
    return BytesIO(self.source.clone(), self._mode)
  }

  /**
   * Returns an empty string because BytesIO do not have any 
   * physical path.
   * 
   * @returns string
   */
  path() {
    return ''
  }

  /**
   * Same as [[io.BytesIO.path()]].
   * 
   * @returns string
   */
  abs_path() {
    return ''
  }

  /**
   * Truncates the entire BytesIO if length is not given or truncates 
   * the BytesIO such that only length number of bytes is left in it.
   * 
   * @returns bool
   */
  truncate(length) {
    if !length length = 0
    if !is_number(length) {
      raise TypeError('invalid I/O data truncation length')
    }

    self._max_length = length
    return true
  }
  
  /**
   * Returns `false` because BytesIO do not have a permission scheme.
   * 
   * @returns bool
   */
  chmod(number) {
    return false
  }

  /**
   * Sets the last access time and last modified time of the BytesIO.
   * 
   * @return bool
   */
  set_times(atime, mtime) {
    if !is_number(atime) {
      raise TypeError('atime must be a number')
    } else if !is_number(mtime) {
      raise TypeError('mtime must be a number')
    }

    if atime != -1 self._atime = atime
    if mtime != -1 self._mtime = mtime
    return true
  }

  /**
   * Sets the position of a BytesIO reader or writer in a BytesIO. 
   * 
   * The position must be within the range of the BytesIO size. The 
   * `seek_type` argument must be on of [[io.SEEK_SET]], 
   * [[io.SEEK_CUR]] or [[io.SEEK_END]].
   * 
   * @returns bool
   */
  seek(position, seek_type) {
    if !is_number(position) {
      raise TypeError('invalid I/O seek position')
    }

    if seek_type == nil seek_type = 0
    if !is_number(seek_type) or seek_type < 0 or seek_type > 2 {
      raise ValueError('invalid seek type')
    }

    using seek_type {
      when 0 self._position = position
      when 1 self._position += position
      when 2 self._position = self.source.length() - position
    }
  }

  /**
   * Returns the current position of the reader/writer in the BytesIO.
   * 
   * @return number
   */
  tell() {
    return self._position
  }

  /**
   * Returns the mode in which the current BytesIO was opened.
   * 
   * @return string
   */
  mode() {
    return self._mode
  }

  /**
   * Returns an empty string since BytesIO do not have a name.
   * 
   * @returns string
   */
  name() {
    return ''
  }

  _can_read() {
    return self._open and to_bool(self._mode.index_of('r') or self._mode.index_of('+') or self._mode.index_of('a'))
  }

  _can_write() {
    return self._open and to_bool(self._mode.index_of('w') or self._mode.index_of('+') or self._mode.index_of('a'))
  }

  _do_open() {
    self._open = true
    self._max_length = self.source.length()
    self._position = 0
  }

  _do_close() {
    self._open = false
    self._max_length = 0
    self._position = self.source.length() + 1
  }

  _read(length) {
    if !self._can_read() {
      raise Exception('I/O not open for reading')
    }

    self._atime = time()

    var max_readable = self._max_length - self._position
    if max_readable < 1 {
      return bytes(0)
    }

    var result
    if length >= max_readable {
      result = self.source[self._position,]
    } else {
      result = self.source[self._position, self._position + length]
    }

    self._position += length
    return result
  }

  _write(data) {
    if !self._can_write() {
      raise Exception('I/O not open for writing')
    }

    if self._max_length - self._position < 0 {
      raise Exception('cannot write beyond I/O offsets')
    }

    self.source += data

    self._max_length += data.length()
    self._mtime = time()

    return data.length()
  }
}
