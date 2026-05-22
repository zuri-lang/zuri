# io.TTY implementation

import _io

/**
 * class TTY is an interface to TTY terminals this class contains definitions
 * to control TTY terminals
 */
class TTY {

  # TTY flags

  /**
   * TTY attribute for input flags.
   *
   * @type int
   * @static
   */
  static var TTY_IFLAG = 0

  /**
   * TTY attribute for output flags.
   *
   * @type int
   * @static
   */
  static var TTY_OFLAG = 1

  /**
   * TTY attribute for control flags.
   *
   * @type int
   * @static
   */
  static var TTY_CFLAG = 2

  /**
   * TTY attribute for local flags.
   *
   * @type int
   * @static
   */
  static var TTY_LFLAG = 3

  /**
   * TTY attribute for input speed.
   *
   * @type int
   * @static
   */
  static var TTY_ISPEED = 4

  /**
   * TTY attribute for output speed.
   *
   * @type int
   * @static
   */
  static var TTY_OSPEED = 5

  # input flags for input processing

  /**
   * Ignore BREAK condition.
   *
   * @type int
   * @static
   */
  static var IGNBRK   = 0x00000001

  /**
   * Map BREAK to SIGINTR.
   *
   * @type int
   * @static
   */
  static var BRKINT   = 0x00000002

  /**
   * Ignore (discard) parity errors.
   *
   * @type int
   * @static
   */
  static var IGNPAR   = 0x00000004

  /**
   * Mark parity and framing errors.
   *
   * @type int
   * @static
   */
  static var PARMRK   = 0x00000008

  /**
   * Enable checking of parity errors.
   *
   * @type int
   * @static
   */
  static var INPCK    = 0x00000010

  /**
   * Strip 8th bit off chars.
   *
   * @type int
   * @static
   */
  static var ISTRIP   = 0x00000020

  /**
   * Map NL into CR.
   *
   * @type int
   * @static
   */
  static var INLCR    = 0x00000040

  /**
   * Ignore CR.
   *
   * @type int
   * @static
   */
  static var IGNCR    = 0x00000080

  /**
   * Map CR to NL (ala CRMOD).
   *
   * @type int
   * @static
   */
  static var ICRNL    = 0x00000100

  /**
   * Enable output flow control.
   *
   * @type int
   * @static
   */
  static var IXON     = 0x00000200

  /**
   * Enable input flow control.
   *
   * @type int
   * @static
   */
  static var IXOFF    = 0x00000400

  /**
   * Any char will restart after stop.
   *
   * @type int
   * @static
   */
  static var IXANY    = 0x00000800

  /**
   * Maintain state for UTF-8 VERASE.
   *
   * @type int
   * @static
   */
  static var IUTF8    = 0x00004000

  # output flags

  /**
   * Enable following output processing.
   *
   * @type int
   * @static
   */
  static var OPOST    = 0x00000001

  /**
   * Map NL to CR-NL (ala CRMOD).
   *
   * @type int
   * @static
   */
  static var ONLCR    = 0x00000002

  # control flags

  /**
   * Character size mask .
   *
   * @type int
   * @static
   */
  static var CSIZE    = 0x00000300

  /**
   * 5 bits (pseudo).
   *
   * @type int
   * @static
   */
  static var CS5      = 0x00000000

  /**
   * 6 bits.
   *
   * @type int
   * @static
   */
  static var CS6      = 0x00000100

  /**
   * 7 bits.
   *
   * @type int
   * @static
   */
  static var CS7      = 0x00000200

  /**
   * 8 bits.
   *
   * @type int
   * @static
   */
  static var CS8      = 0x00000300

  /**
   * Send 2 stop bits.
   *
   * @type int
   * @static
   */
  static var CSTOPB   = 0x00000400

  /**
   * Enable receiver.
   *
   * @type int
   * @static
   */
  static var CREAD    = 0x00000800

  /**
   * Parity enable.
   *
   * @type int
   * @static
   */
  static var PARENB   = 0x00001000

  /**
   * Odd parity, else even.
   *
   * @type int
   * @static
   */
  static var PARODD   = 0x00002000

  /**
   * Hang up on last close.
   *
   * @type int
   * @static
   */
  static var HUPCL    = 0x00004000

  /**
   * Ignore modem status lines.
   *
   * @type int
   * @static
   */
  static var CLOCAL   = 0x00008000

  # "Local" flags - dumping ground for other state
  # Warning: some flags in this structure begin with
  # the letter "I" and look like they belong in the
  # input flag.

  /**
   * Visually erase chars.
   *
   * @type int
   * @static
   */
  static var ECHOE    = 0x00000002

  /**
   * Echo NL after line kill
   * @static
   */
  static var ECHOK    = 0x00000004

  /**
   * Enable echoing.
   *
   * @type int
   * @static
   */
  static var ECHO     = 0x00000008

  /**
   * Echo NL even if ECHO is off.
   *
   * @type int
   * @static
   */
  static var ECHONL   = 0x00000010

  /**
   * Enable signals INTR, QUIT, [D]SUSP.
   *
   * @type int
   * @static
   */
  static var ISIG     = 0x00000080

  /**
   * Canonicalize input lines.
   *
   * @type int
   * @static
   */
  static var ICANON   = 0x00000100

  /**
   * Enable DISCARD and LNEXT.
   *
   * @type int
   * @static
   */
  static var IEXTEN   = 0x00000400

  /**
   * Stop background jobs from output.
   *
   * @type int
   * @static
   */
  static var TOSTOP   = 0x00400000

  /**
   * Don't flush after interrupt.
   *
   * @type int
   * @static
   */
  static var NOFLSH   = 0x80000000

  #-----------------------------------------------------------------------

  # Commands passed to set_attr() for setting the TTY attributes.

  /**
   * Make change immediate.
   *
   * @type int
   * @static
   */
  static var TCSANOW    = 0

  /**
   * Drain output, then change.
   *
   * @type int
   * @static
   */
  static var TCSADRAIN  = 1

  /**
   * Drain output, flush input.
   *
   * @type int
   * @static
   */
  static var TCSAFLUSH  = 2

  # Special Control Characters

  /**
   * ICANON.
   *
   * @type int
   * @static
   */
  static var VEOF       = 0

  /**
   * ICANON.
   *
   * @type int
   * @static
   */
  static var VEOL       = 1

  /**
   * ICANON.
   *
   * @type int
   * @static
   */
  static var VERASE     = 3

  /**
   * ICANON.
   *
   * @type int
   * @static
   */
  static var VKILL      = 5

  /**
   * ISIG.
   *
   * @type int
   * @static
   */
  static var VINTR      = 8

  /**
   * ISIG.
   *
   * @type int
   * @static
   */
  static var VQUIT      = 9

  /**
   * ISIG.
   *
   * @type int
   * @static
   */
  static var VSUSP      = 10

  /**
   * IXON, IXOFF.
   *
   * @type int
   * @static
   */
  static var VSTART     = 12

  /**
   * IXON, IXOFF.
   *
   * @type int
   * @static
   */
  static var VSTOP      = 13

  /**
   * !ICANON.
   *
   * @type int
   * @static
   */
  static var VMIN       = 16

  /**
   * !ICANON.
   *
   * @type int
   * @static
   */
  static var VTIME      = 17

  /**
   * TTY(std: file)
   *
   * @note _file_ must be one of stdout and stderr
   * @param file std
   * @constructor
   */
  TTY(std) {
    if !is_file(std) {
      raise TypeError('TTY expects a standard file as argument, ${typeof(std)} given')
    }

    self.std = std
  }

  /**
   * Returns the attribute of the current tty session
   * The returned attributes is a dict containing the TTY_ flags
   *
   * @returns dict
   */
  get_attr() {
    return _io.TTY.tcgetattr(self.std)
  }

  /**
   * sets the attributes of the current tty session
   *
   * >  __NOTE__: 
   * > - _option_ must be one ot the TCSA options above (see their description above)
   * > - _attrs_ must be a dictionary of the TTY_ flags listed above
   * > - one can safely omit any of the TTY_ flags listed above and Zuri will
   * >   fill in the default values as it exists.
   * > - This flags will be merged and not overwritten
   * @param number option
   * @param dict attr
   * @returns bool
   */
  set_attr(option, attrs) {
    if !is_int(option)
      raise TypeError('integer expected as first argument, ${typeof(option)} given')
    if !is_dict(attrs)
      raise TypeError('dictionary expected as second argument, ${typeof(attrs)} given')
    return _io.TTY.tcsetattr(self.std, option, attrs)
  }

  /**
   * Sets the current tty to raw mode.
   *
   * @returns bool
   */
  set_raw() {
    var new_attr = _io.TTY.tcgetattr(self.std, false)

    new_attr[TTY.TTY_IFLAG] &= ~(TTY.IGNBRK | TTY.BRKINT | TTY.PARMRK | TTY.ISTRIP | TTY.INLCR | TTY.IGNCR | TTY.ICRNL | TTY.IXON)
    new_attr[TTY.TTY_OFLAG] &= ~TTY.OPOST
    new_attr[TTY.TTY_LFLAG] &= ~(TTY.ECHO | TTY.ECHONL | TTY.ICANON | TTY.ISIG | TTY.IEXTEN)
    new_attr[TTY.TTY_CFLAG] &= ~(TTY.CSIZE | TTY.PARENB)
    new_attr[TTY.TTY_CFLAG] |= TTY.CS8

    return self.set_attr(TTY.TCSAFLUSH, new_attr)
  }

  /**
   * Disables the raw mode flags on the current tty.
   *
   * @returns bool
   */
  exit_raw() {
    # return _io.TTY.exit_raw(self.std)
    return self.set_attr(TTY.TCSAFLUSH, _io.TTY.tcgetattr(self.std, true))
  }

  /**
   * Returns the size of the current TTY device as a dictionary of cols and rows.
   * 
   * - `cols`: The number of text columns that can fit into the TTY device.
   * - `rows`: The number of text rows that can fit into the TTY device.
   * 
   * @returns dict
   */
  get_size() {
    var size = _io.TTY.getsize(self.std)

    return {
      cols: size.first(),
      rows: size.last(),
    }
  }

  /**
   * Flushes the standard output and standard error interface
   */
  flush() {
    _io.TTY.flush(self.std)
  }
}
