/**
 * @module log
 * 
 * This module implements a simple and flexible event logging system for all 
 * Zuri applications and modules. With support for multiple transport systems
 * as well as custom transports, this module allows easy application logging 
 * and log shipping.
 * 
 * The module selects defaults that is familiar for most end use-cases in order 
 * to allow for a minimal need for configurations so that you can start logging 
 * right out of the box.
 * 
 * Below is a very simple but powerful and complete usage of this module:
 * 
 * ```zuri-repl
 * %> import log
 * %> log.info('Starting my application...')
 * '2025-03-07T08:00:33+01:00 INFO [.]: Starting my application...'
 * ```
 * 
 * > **IMPORTANT!**
 * >
 * > Did you notice that `[.]`? That's because we are running in a REPL. By default, 
 * > the `log` module provides information regarding the source of the log i.e. the 
 * > application from which the log came from thereby allowing multiple applications 
 * > log into the same transport pool without ambiguity.
 * >
 * > You can customize this name by setting the name on the transport via 
 * > [[log.Transport.set_name]].
 * 
 * This module provides all functionalities at the module level, allowing 
 * configurations to be carried across all files and modules in the lifetime of an 
 * application.
 * 
 * While allowing creation of custom transports for logs, the module provides 
 * transports for logging to the console ([[log.ConsoleTransport]]) and files 
 * ([[log.FileTransport]]) out of the box. This covers the most simple use-cases for most 
 * applications. 
 * 
 * The default transport enabled is the [[log.ConsoleTransport]] known as the 
 * [[log.default_transport()]] and need no extra work to enable unless you have previously 
 * disabled it. The example below shows how to enable the file transport to log to a 
 * file on disk.
 * 
 * ```zuri
 * import log
 * 
 * var transport = log.FileTransport('mylog.log')
 * log.add_transport(transport)
 * 
 * log.info('Finished setting up file log...')
 * ```
 * 
 * If you check the file `mylog.log` now, you should see something like this:
 * 
 * ```sh
 * 2025-03-01T12:00:00+01:00 INFO [tmp]: Finished setting up file log...
 * ```
 * 
 * In addition to the log appearing on the console, you can now see the log in a 
 * persistent file. There are many ways to turn off the console output and log to file 
 * alone. 
 * 
 * Firstly, you can simple disable the default transport.
 * 
 * ```zuri
 * log.default_transport().disable()
 * ```
 * 
 * The advantage to this approach is that while it disables the default transport, the 
 * transport is still registered and you can simple enable it at any time during the 
 * lifetime of the application by doing the reverse:
 * 
 * ```zuri
 * log.default_transport().enable()
 * ```
 * 
 * The same strategy applies to all transports as the `enable()` and `disable()` method 
 * will be inherited from the [[log.Transport]] class.
 * 
 * The second approach is to completely remove the transport from the list of registered 
 * transports.
 * 
 * ```zuri
 * log.remove_transport(log.default_transport())
 * ```
 * 
 * With this second approach, you'll need to register the transport again should you 
 * want to continue logging to the console. The same applies to all transport types.
 * 
 * For more complex uses, the process of creating a custom transport is really simple. 
 * To create a custom transport, you'll need to create a class that inherits from 
 * [[log.Transport]] and implement the `write()` method at a minimum.
 * 
 * The example below shows the creation of a custom transport that outputs structured 
 * JSON data to the console.
 * 
 * ```zuri
 * # my_custom_transport.b
 * 
 * import log { Transport }
 * import json
 * import enum
 * 
 * class JsonConsoleTransport < Transport {
 *   format(records, level) {
 *     return {
 *       records,
 *       level,
 *     }
 *   }
 * 
 *   write(message, level) {
 *     echo json.encode({
 *       logtime: time(),
 *       records: message.records,
 *       name: self.get_name(),
 *       level: enum.to_value_dict(log.LogLevel)[message.level]
 *     })
 *   }
 * }
 * ```
 * 
 * ```zuri
 * import log
 * import .my_custom_transport { JsonConsoleTransport }
 * 
 * log.add_transport(JsonConsoleTransport())
 * 
 * log.info('Finished setting up json console log transport...')
 * ```
 * 
 * You should be seeing something similar to the below if you run the code:
 * 
 * ```sh
 * {"logtime":1741376459,"records":["Finished setting up json console log transport..."],"name":"tmp","level":"Info"}
 * ```
 * 
 * You can set multiple transports at the same time as well as set them to only work at 
 * different log levels. Every transport inherits the method [[log.Transport.set_level]] 
 * which allows us set the minimum level at which a transport is available.
 * 
 * There is also a global [[log.set_level]] function that allows us to set the
 * minimum log level at which all transports can start logging.
 * 
 * ```zuri
 * import log
 * 
 * log.set_level(log.Warning)
 * 
 * log.info('Finished setting up json console log transport...')
 * ```
 * 
 * If you try the above code, you won't be seeing anything in the console. This is 
 * because level [[log.Info]] is lower than the minium required [[log.Warning]].
 * 
 * While every `Transport` implements the `format()` method, users can override the 
 * format method by providing a format function via the [[log.Transport.set_formatter]] 
 * method. This allows using the same formatter across different transports. 
 * 
 * For example, in the previous example, if we had wanted to serialize all logging 
 * irrespective of the transport into the JSON format, a more appropriate solution would 
 * have been to create a log format function and reuse it in all transports instead of
 * implementing a `JsonConsoleTransport`. The next example shows one such implementation.
 * 
 * ```zuri
 * import log
 * import json
 * import date
 * 
 * def json_format(records, level, transport) {
 *   return json.encode({
 *     time: date().format(transport.get_time_format()),
 *     level: log.get_level_name(level),
 *     name: transport.get_name(),
 *     records,
 *   })
 * }
 * 
 * var file_transport = log.FileTransport('mylog.log')
 * log.add_transport(file_transport)
 * 
 * log.default_transport().set_formatter(json_format)
 * file_transport.set_formatter(json_format)
 * 
 * log.debug('This is a debug information')
 * ```
 * 
 * Transport implementations should take note of this critical information.
 * 
 * > **IMPORTANT!**
 * > 
 * > Because the `log` module exports the a function, if you are not interested in all 
 * > the shenanigans of logging level and simply want to do some quick logging, you can
 * > ignore the whole logging levels altogether and log at level [[log.None]] by simply 
 * > calling the log module itself.
 * > 
 * > ```zuri
 * > import log
 * > 
 * > log('An anonymous log!')
 * > ```
 * 
 * @copyright 2025, Richard Ore and Zuri contributors
 */
import enum
import io
import os
import date
import json as js


/**
 * The Log levels in order
 * 
 * - None
 * - Debug
 * - Info
 * - Warning
 * - Error
 * - Critical
 * 
 * @type enum
 */
var LogLevel = enum([
  'None',
  'Debug',
  'Info',
  'Warning',
  'Error',
  'Critical',
])

var _level_lookup_table = enum.to_value_dict(LogLevel)
var _default_log_level = LogLevel.None

# LogLevel Exports

/**
 * Module level declaration of `LogLevel.None`
 */
var None =  LogLevel.None

/**
 * Module level declaration of `LogLevel.Debug`
 */
var Debug = LogLevel.Debug

/**
 * Module level declaration of `LogLevel.Info`
 */
var Info = LogLevel.Info

/**
 * Module level declaration of `LogLevel.Warning`
 */
var Warning = LogLevel.Warning

/**
 * Module level declaration of `LogLevel.Error`
 */
var Error = LogLevel.Error

/**
 * Module level declaration of `LogLevel.Critical`
 */
var Critical = LogLevel.Critical


/**
 * Returns the name of a log level as a string.
 * 
 * @param [[log.LogLevel]] level
 * @returns string
 */
def get_level_name(level) {
  if !is_number(level) {
    raise ValueError('invalid log level')
  }

  level = enum.ensure(LogLevel, level)
  return _level_lookup_table[level].upper()
}


/**
 * The Transport class acts as the base class for log transports and handle the actual 
 * logging of the specified log records.
 */
class Transport {
  var _level = LogLevel.None

  var _log_name = os.base_name(os.dir_name(__root__))

  # hidden at one more level to avoid accidental overrides.
  # should this be overridden, it should be deliberate.
  var __enabled = true
  var __time_format = 'c'
  var __max_level = LogLevel.Critical
  var __formatter

  var _show_name = true
  var _show_time = true
  var _show_level = true

  /**
   * Sets the threshold level for this transport to handle. Logging messages which are 
   * less severe than level will be ignored. Unless overridden by the transport
   * implementation, when a handler is created, the level is set to [[log.None]] (which 
   * causes all messages to be processed).
   * 
   * @param [[log.LogLevel]] level
   * @returns self
   */
  set_level(level) {
    if !is_number(level) {
      raise ValueError('invalid log level')
    }

    self._level = enum.ensure(LogLevel, level)
    return self
  }

  /**
   * The threshold level of this transport. The default level is [[log.LogLevel.None]].
   * 
   * @returns [[log.LogLevel]]
   */
  get_level() {
    return self._level
  }

  /**
   * Sets the maximum threshold level for this transport to handle. Logging 
   * messages which are more severe than level will be ignored. Unless 
   * overridden by the transport implementation, when a handler is created, the
   * maximum level is set to [[log.Critical]] (which causes all messages to be 
   * processed).
   * 
   * @param [[log.LogLevel]] level
   * @returns self
   */
  set_max_level(level) {
    if !is_number(level) {
      raise ValueError('invalid log level')
    }

    self.__max_level = enum.ensure(LogLevel, level)
    return self
  }

  /**
   * The maximum threshold level of this transport. The default maximum level is 
   * [[log.Critical]].
   * 
   * @returns [[log.LogLevel]]
   */
  get_max_level() {
    return self.__max_level
  }

  /**
   * Sets the name of the current transport.
   * 
   * @param string name
   * @returns self
   */
  set_name(name) {
    if !is_string(name) {
      raise TypeError('string expected, ${typeof(name)} given')
    }
  
    self._log_name = name
    return self
  }

  /**
   * Returns the name of the current transport. By default, name will be equal to the
   * name of the directory containing the root file.
   * 
   * @returns string
   */
  get_name() {
    return self._log_name
  }

  /**
   * Sets the time formatting string used by the transport when 
   * [[log.Transport.show_time]] is set to true.
   * 
   * @param string format
   * @returns self
   */
  set_time_format(format) {
    if !is_string(format) {
      raise TypeError('string expected, ${typeof(name)} given')
    }
  
    self.__time_format = format
    return self
  }

  /**
   * Returns the time formatting string used by the current transport. The default 
   * value is `c`.
   * 
   * @returns string
   */
  get_time_format() {
    return self.__time_format
  }

  /**
   * Enable or disable showing transport names in the logs based on the passed 
   * boolean value.
   * 
   * @param bool show
   * @returns self
   */
  show_name(show) {
    if show == nil show = true

    if !is_bool(show) {
      raise TypeError('boolean expected, ${typeof(show)} given')
    }

    self._show_name = show
    return self
  }

  /**
   * Enables or disables showing logging time in the logs based on the passed boolean 
   * value.
   * 
   * @param bool show
   * @returns self
   */
  show_time(show) {
    if show == nil show = true

    if !is_bool(show) {
      raise TypeError('boolean expected, ${typeof(show)} given')
    }

    self._show_time = show
    return self
  }

  /**
   * Enable or disable showing transport log level in the logs based on the passed 
   * boolean value.
   * 
   * @param bool show
   * @returns self
   */
  show_level(show) {
    if show == nil show = true

    if !is_bool(show) {
      raise TypeError('boolean expected, ${typeof(show)} given')
    }

    self._show_level = show
    return self
  }

  /**
   * Sets a formatter function that overrides a transports `format()` function. 
   * 
   * The formatter function is a function that when it is set overrides the transports 
   * default format method and must MUST have the contract 
   * `def my_function(records, level, transport)`. The `transport` parameter here is 
   * the instance of the current transport. See [[log.Transport.format]] for what 
   * `records`, and `level` means.
   * 
   * @param function formatter
   * @returns self
   */
  set_formatter(formatter) {
    if !is_function(formatter) {
      raise TypeError('function expected, ${typeof(formatter)} given')
    }
  
    self.__formatter = formatter
    return self
  }

  /**
   * Returns the format method override that has been set for the current transport or 
   * `nil` if none has been set.
   * 
   * @returns function?
   */
  get_formatter() {
    return self.__formatter
  }

  /**
   * Returns a boolean value which indicates if a message of severity *level* can be 
   * processed by this transport.
   * 
   * @param [[log.LogLeven]] level
   * @returns bool
   */
  can_log(level) {
    if !is_number(level) {
      raise ValueError('invalid log level')
    }
    
    return self.__enabled and self._level <= level and 
      level >= _default_log_level and
      level <= self.__max_level
  }

  /**
   * Enables and starts processing of logs by the current transport.
   * 
   * @returns self
   */
  enable() {
    self.__enabled = true
    return self
  }

  /**
   * Disables and stops the current transport from processing further logs.
   * 
   * @returns self
   */
  disable() {
    self.__enabled = false
    return self
  }

  /**
   * Formats the log records for the current level for writing to the transport's 
   * stream. The default implementation of this method is exactly as seen when using 
   * the [[log.default_transport]] which logs to the console. The method 
   * should be overridden by subclasses to get a custom formatting.
   * 
   * > __IMPORTANT!__
   * >
   * > The result of this function will be passed into the [[log.Transport.write()]] 
   * > function so transport implementations *MUST* ensure to expect the same type as 
   * > is returned from this function in the [[log.Transport.write()]] function message 
   * > parameter.
   * 
   * Transport implementations should be aware of the following available private 
   * fields in the transport class:
   * 
   * - _[[log.LogLevel]]_ `self._level`
   * - _string_ `self._log_name`
   * - _bool_ `self._show_name`
   * - _bool_ `self._show_time`
   * - _bool_ `self._show_level`
   * 
   * @params {list[any]} records
   * @param [[log.LogLevel]] level
   * @returns any
   */
  format(records, level) {
    var message = ''

    if self._show_time {
      message += date().format(self.__time_format)
    }

    if self._show_level {
      message += ' ${_level_lookup_table[level].upper()}'
    }

    if self._show_name {
      message += ' [${self._log_name}]'
    }

    message += ':'

    for arg in records {
      message += ' ' + to_string(arg)
    }

    return message.trim()
  }

  /**
   * Do whatever it takes to actually log the specified logging record. This method is 
   * intended to be implemented by subclasses and so raises an *Exception* if called 
   * directly from `Transport`.
   * 
   * @param any message
   * @param [[log.LogLevel]] level
   * @raises Exception
   */
  write(message, level) {
    raise NotImplementedError()
  }

  /**
   * Ensure all logging output has been flushed to the target stream. This default version 
   * does nothing and is intended to be implemented by subclasses.
   */
  flush() {}

  /**
   * Tidy up any resources used by the transport. This default version does no output but 
   * removes the handler from an internal list of handlers. Subclasses should ensure that 
   * this gets called from overridden close() methods.
   */
  close() {
    _transports.remove(self)
  }
}


# Verifies that obj is a valid Transport and returns obj as is if true 
# or raises an Exception if not.
def _assert_transport(obj) {
  if !instance_of(obj, Transport) {
    raise ValueError('invalid Transport')
  }

  return obj
}


/**
 * ConsoleTransport is a log transport that facilitates sending log streams to the console.
 */
class ConsoleTransport < Transport {
  write(message, level) {
    if level >= Error {
      io.stderr.write(message + '\n')
    } else {
      echo message
    }
  }

  flush() {
    io.flush(io.stdout)
  }
}


/**
 * FileTransport is a log transport that facilitates sending log streams to an on-disk file.
 */
class FileTransport < Transport {
  
  /**
   * Returns a new instance of FileTransport and opens a file handle to the the file 
   * specified in the path.
   * 
   * @param string path
   * @constructor
   */
  FileTransport(path) {
    if !is_string(path) {
      raise Exception('path must be string')
    }

    # Create the parent log directory if it does not exist.
    var dir_name = os.dir_name(path)
    if !os.dir_exists(dir_name) {
      os.create_dir(dir_name, 0c777, true)
    }

    self.file = file(path, 'a')
  }

  write(message, level) {
    self.file.write('${message}\n')
  }

  flush() {
    self.file.flush()
  }

  close() {
    parent.close()
    self.file.close()
  }
}


# This is the default transport that will be added to a transport.
# we have it here as a module variable so we can use it to remove
# the transport should the user decide to (if they prefer not to)
# simply disable it.
var _default_transport = ConsoleTransport()


/**
 * Sets the threshold level for the default transport to handle.
 * 
 * @param [[log.LogLevel]] level
 * @returns default_transport
 */
def set_level(level) {
  if !is_number(level) {
    raise ValueError('invalid log level')
  }

  _default_log_level = enum.ensure(LogLevel, level)
  _default_transport.set_level(_default_log_level)

  return _default_transport
}


/**
 * Sets the name of the default transport.
 * 
 * @param string name
 * @returns default_transport
 */
def set_name(name) {
  if !is_string(name) {
    raise TypeError('string expected, ${typeof(name)} given')
  }

  _default_transport.set_name(name)
  return _default_transport
}


# A list of registered transports
var _transports = [ _default_transport ]

/**
 * Adds a new transport service to the list of registered transports. If the transport 
 * has been previously added, this function will do nothing.
 * 
 * @param [[log.Transport]] transport
 */
def add_transport(transport) {
  # ensure we are not adding the same transport again
  if _transports.contains(transport) {
    return
  }

  _transports.append(transport)
}

/**
 * Removes the given transport service from the list of registered transports.
 * 
 * @param [[log.Transport]] transport
 */
def remove_transport(transport) {
  _transports.remove(transport)
}

/**
 * Returns the instance [[log.ConsoleTransport]] which is used as the default transport 
 * by the module.
 * 
 * @returns [[log.ConsoleTransport]]
 */
def default_transport() {
  return _default_transport
}


# Reusable write function for the log level helpers.
def _write(level, args) {
  for trans in _transports {
    if trans.can_log(level) {
      var formatter = trans.get_formatter()

      trans.write(
        formatter ? formatter(args, level, trans) : trans.format(args, level), 
        level
      )
    }
  }
}


/**
 * Logs a message with level [[log.None]] on all registered transports.
 * 
 * The format of the output log is dependent on the specific transport service as well 
 * as limitations on the nature and type of arguments that is passed to the function.
 * 
 * @params any...
 * @default
 */
def log(...) {
  _write(LogLevel.None, __args__)
}

/**
 * Logs a message with level [[log.Info]] on all registered transports. The arguments and 
 * limitations as same as in [[log.log()]].
 * 
 * @params any...
 */
def info(...) {
  _write(LogLevel.Info, __args__)
}

/**
 * Logs a message with level [[log.Debug]] on all registered transports. The arguments and 
 * limitations as same as in [[log.log()]].
 * 
 * @params any...
 */
def debug(...) {
  _write(LogLevel.Debug, __args__)
}

/**
 * Logs a message with level [[log.Warning]] on all registered transports. The arguments and 
 * limitations as same as in [[log.log()]].
 * 
 * @params any...
 */
def warn(...) {
  _write(LogLevel.Warning, __args__)
}

/**
 * Logs a message with level [[log.Error]] on all registered transports. The arguments and 
 * limitations as same as in [[log.log()]].
 * 
 * @params any...
 */
def error(...) {
  _write(LogLevel.Error, __args__)
}

/**
 * Logs a message with level [[log.Critical]] on all registered transports. The arguments and 
 * limitations as same as in [[log.log()]].
 * 
 * @params any...
 */
def critical(...) {
  _write(LogLevel.Critical, __args__)
}

/**
 * Logs a message with level [[log.Error]] on all registered transports along with an exception 
 * message and stacktrace. The arguments and limitations as same as in [[log.log()]].
 * 
 * @param Exception ex
 * @param any? message
 */
def exception(ex, message) {
  if instance_of(ex, Exception) {
    if message {
      _write(LogLevel.Error, [ 
        '${message}\n', 
        'Unhandled ${typeof(ex)}: ${ex.message}', 
        '\n${ex.stacktrace}', 
      ])
    } else {
      _write(LogLevel.Error, [ 
        'Unhandled ${typeof(ex)}: ${ex.message}', 
        '\n${ex.stacktrace}' 
      ])
    }
  } else if message {
    _write(LogLevel.Error, [ 
      'Unhandled ${typeof(ex)}: ${message}' 
    ])
  }
}
