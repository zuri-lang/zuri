import reflect
import colors
import io


var _before_eachs = []
var _before_alls = []
var _after_eachs = []
var _after_alls = []

var total_time = 0

var _total_suites = 0
var _total_tests = 0
var _total_assertions = 0

var _passed_suites = 0
var _passed_tests = 0
var _passed_assertions = 0

var _failed_suites = 0
var _failed_tests = 0
var _failed_assertions = 0

var _file

var _stats = []
var _curr_desc = {
  it: [],
  file: _file,
  time: 0,
  skipped: false,
}
var _curr_it = {}

def before_each(fn) {
  _before_eachs.append(fn)
}

def after_each(fn) {
  _after_eachs.append(fn)
}

def before_all(fn) {
  _before_alls.append(fn)
}

def after_all(fn) {
  _after_alls.append(fn)
}

def _get_mark(state) {
  return state ? 
  colors.text('\u2714', colors.text_color.green) :
  colors.text('\u2715', colors.text_color.light_red)
}

def _print(text, state) {
  return state ? 
  colors.text('\u2714 ' + text, colors.text_color.green) :
  colors.text('\u2715 ' + text, colors.text_color.light_red)
}

def _print_skipped(text) {
  return colors.text('\u25CB ' + text, colors.text_color.yellow)
}

def _gray(txt) {
  return colors.text(txt, colors.text_color.dark_grey)
}

def _orange(txt) {
  return colors.text(txt, colors.text_color.orange)
}

def _red(txt) {
  return colors.text(txt, colors.text_color.light_red)
}

def _report(text, state) {
  return state ? 
  colors.text(text, colors.text_color.green) :
  colors.text(text, colors.text_color.light_red)
}

def _time(time) {
  if time < 1000 {
    return time + 'µs'
  } else if time < 1000000 {
    return (time / 1000) + 'ms'
  }

  return (time / 1000000) + 's'
}

def _default_fn(x, y) {
  return x == y
}

import hash
class expect {
  var value
  var _is_not = false

  expect(value) {
    self.value = value
  }

  _run(name, expected, fn) {
    if self._is_not name = 'not ${name}'
    if !fn fn = _default_fn

    var v = to_string(self.value).replace('\r', '\\r').replace('\n', '\\n')
    var w = expected != nil ? to_string(expected).replace('\r', '\\r').replace('\n', '\\n') : nil

    var state = {
      name: w != nil ? 
        'Expect value ${name}:\n          ${_orange(w)}\n        Got:\n          ${_red(v)}' :
        'Expected "${_red(v)}" ${name}', 
      status: false,
    }
    
    catch {
      var test_value = fn == _default_fn ? fn(v, w) :
          fn(self.value, expected)

      if !self._is_not and test_value {
        _passed_assertions++
        state.status = true
      } else if self._is_not and !test_value {
        _passed_assertions++
        state.status = true
      } else {
        _failed_assertions++
      }
    } as e

    _curr_it.expects.append(state)
    _total_assertions++

    if e {
      _failed_assertions++
      io.stderr.write(e.message + '\r\n')
      io.stderr.write(e.stacktrace + '\r\n')
    }
  }

  not() {
    self._is_not = true
    return self
  }

  to_be(e) {
    self._run('to be', e)
    return self
  }

  to_be_nil() {
    self._run('to be nil', nil)
    return self
  }

  to_be_defined() {
    self._run('to be defined', nil, @(x, y) {
      return x != nil
    })
    return self
  }

  to_be_truthy() {
    self._run('to be truthy', nil, @(x, y) {
      return !!x
    })
    return self
  }

  to_be_falsy() {
    self._run('to be falsy', nil, @(x, y) { return !x })
    return self
  }

  to_be_greater_than(e) {
    self._run('to be greather than', e, @(x, y) { 
      if is_string(x) return x.length() > y
      return x > y 
    })
    return self
  }

  to_be_greater_than_or_equal(e) {
    self._run('to be greather than or equal to', e, @(x, y) { 
      if is_string(x) return x.length() >= y
      return x >= y 
    })
    return self
  }

  to_be_less_than(e) {
    self._run('to be less than', e, @(x, y) { 
      if is_string(x) return x.length() < y
      return x < y 
    })
    return self
  }

  to_be_less_than_or_equal(e) {
    self._run('to be less than or equal to', e, @(x, y) { 
      if is_string(x) return x.length() <= y
      return x <= y 
    })
    return self
  }

  to_match(e) {
    self._run('to match', e, @(x, y) { return x.match(y) })
    return self
  }

  to_contain(e) {
    self._run('to contain', e, @(x, y) {
      if is_dict(x) return x.contains(y)
      return x.count(y) > 0
    })
    return self
  }

  to_throw(e) {
    if !e e = Exception

    if is_function(self.value) {
      self._run('to throw', e, @(x, y) {

        catch {
          x()
          return false
        } as ex

        if ex {
          if is_string(e) return ex.message.match(e)
          if is_class(e) return instance_of(ex, e)
          if instance_of(e, Exception) return ex == e
          return true
        }

        return false
      })
    } else {
      self._run('to throw', e, @(x, y) { return false })
    }

    return self
  }

  to_have_length(e) {
    self._run('to have length', e, @(x, y) { return x.length() == y })
    return self
  }

  to_be_instance_of(e) {
    self._run('to be an instance of', e, @(x, y) { return instance_of(x, y) })
    return self
  }

  to_have_property(e, value) {
    var name = 'to have a property'
    if value != nil name = 'to have value "${to_string(value)}" in property'

    self._run(name, e, @(x, y) {
      var res = is_instance(x) and reflect.has_prop(x, y)
      if res and value != nil {
        return reflect.get_prop(x, y) == value
      }
      return res
    })

    return self
  }

  to_have_method(e) {
    self._run('to have a method', e, @(x, y) {
      return is_instance(x) and reflect.has_method(x, y)
    })
    return self
  }

  to_have_decorator(e) {
    self._run('to have a decorator', e, @(x, y) {
      return is_instance(x) and reflect.has_decorator(x, y)
    })
    return self
  }

  to_be_boolean() {
    self._run('to be a boolean', nil, @(x, y) { return is_bool(x) })
    return self
  }

  to_be_number() {
    self._run('to be a number', nil, @(x, y) { return is_number(x) })
    return self
  }

  to_be_string() {
    self._run('to be a string', nil, @(x, y) { return is_string(x) })
    return self
  }

  to_be_list() {
    self._run('to be a list', nil, @(x, y) { return is_list(x) })
    return self
  }

  to_be_dict() {
    self._run('to be a dict', nil, @(x, y) { return is_dict(x) })
    return self
  }

  to_be_function() {
    self._run('to be a function', nil, @(x, y) { return is_function(x) })
    return self
  }

  to_be_class() {
    self._run('to be a class', nil, @(x, y) { return is_class(x) })
    return self
  }

  to_be_iterable() {
    self._run('to be an iterable', nil, @(x, y) { return is_iterable(x) })
    return self
  }

  to_be_file() {
    self._run('to be a file', nil, @(x, y) { return is_file(x) })
    return self
  }

  to_be_bytes() {
    self._run('to be bytes', nil, @(x, y) { return is_bytes(x) })
    return self
  }
}

def it(desc, fn) {
  _total_tests++
  var start = microtime()
  for be in _before_eachs {
    be()
  }

  _curr_it = {
    name: desc,
    expects: [],
    time: 0,
    skipped: false,
  }

  var pre_run_count = _total_assertions
  catch {
    fn()
  } as e

  if e {
    io.stderr.write(e.message + '\r\n')
    io.stderr.write(e.stacktrace + '\r\n')
  }

  if pre_run_count == _total_assertions {
    _curr_it.skipped = true
  }

  for ae in _after_eachs {
    ae()
  }

  _curr_it.time = microtime() - start

  _curr_desc.it.append(_curr_it)
}

def describe(desc, fn) {
  _curr_desc = {
    it: [],
    file: _file,
    time: 0,
    skipped: false,
  }

  catch {
    var start = microtime()

    for ba in _before_alls {
      ba()
    }

    _curr_desc.name = desc
    var pre_run_count = _total_assertions

    fn()

    if pre_run_count == _total_assertions {
      _curr_desc.skipped = true
    }

    for aa in _after_alls {
      aa()
    }

    _curr_desc.time = microtime() - start
  } as e

  _total_suites++

  if e {
    io.stderr.write(e.message + '\r\n')
    io.stderr.write(e.stacktrace + '\r\n')
  }

  show_result(_curr_desc)
}

def run(f) {
  _file = f
  reflect.run_script(f)
}

def show_result(e) {
  total_time += e.time

  var fails = e.it.filter(@(x) {
    return x.expects.filter(@(y) { return !y.status }).length() > 0
  }).length() > 0
  if fails _failed_suites++

  if e.skipped {
    echo colors.text(' SKIPPED ', colors.background.yellow) + ' ' + e.file
    _total_suites--
    return
  }

  echo colors.text(
    !fails ? ' PASS ' : ' FAIL ', 
    fails ? 
      colors.background.light_red :
      colors.background.green
  ) + ' ' + e.file

  echo '  ${e.name}'
  iter var i = 0; i < e.it.length(); i++ {
    var _e = e.it[i]
    var it_fails = _e.expects.filter(@(x) { return !x.status }).length() > 0
    if it_fails _failed_tests++

    if _e.skipped {
      echo '    ' + _print_skipped('${_e.name} (${_time(_e.time)}) - Skipped')
      _total_tests--
      continue
    }

    echo '    ' + _print('${_e.name} (${_time(_e.time)})', !it_fails)

    if it_fails {
      iter var j = 0; j < _e.expects.length(); j++ {
        var expect = _e.expects[j]
        if !expect.status {
          echo '      ${_get_mark(expect.status)} ${expect.name}'
        }
      }
    }
  }
  
  echo ''
}

def show_tests_results() {
  echo ''

  var passed_suites = _report((_total_suites - _failed_suites) + ' passed', true)
  var passed_tests = _report((_total_tests - _failed_tests) + ' passed', true)
  var passes = _passed_assertions > 0 ? _report(_passed_assertions + ' passed', true) : '0 passed'
  var suite_fails = _failed_suites > 0 ? _report(_failed_suites + ' failed', false) : '0 failed'
  var test_fails = _failed_tests > 0 ? _report(_failed_tests + ' failed', false) : '0 failed'
  var assert_fails = _failed_assertions > 0 ? _report(_failed_assertions + ' failed', false) : '0 failed'

  echo colors.text('Test suites:  ${passed_suites}, ${suite_fails}, ${_total_suites} total', colors.style.bold)
  echo colors.text('Tests:        ${passed_tests}, ${test_fails}, ${_total_tests} total', colors.style.bold)
  echo colors.text('Assertions:   ${passes}, ${assert_fails}, ${_total_assertions} total', colors.style.bold)
  echo colors.text('Time:         ${_time(total_time)}', colors.style.bold)
  echo colors.text(_gray('Ran all test suites.'), colors.style.bold)

  return _failed_tests == 0
}
