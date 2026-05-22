/**
 * @module date
 * 
 * This modules provides Zuri's implementation of date and time
 * manipulation methods. This module implements civil dates as well as julian dates.
 * 
 * ### Definitions
 * 
 * - The calendar date (`class Date`) is a particular day of a calendar year, 
 * identified by its ordinal number within a calendar month within that year.
 * 
 * - The Julian date number (`jd`) is in elapsed days and time since noon 
 * (Greenwich Mean Time) on January 1, 4713 BCE (in the Julian calendar).
 * 
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import _date


/**
 * Minimum year supported.
 * @type number
 */
var MIN_YEAR = 1

/**
 * Maximum year supported.
 * @type number
 */
var MAX_YEAR = 9999

/**
 * Minimum day supported.
 * @type number
 */
var MIN_DAY = 1

/**
 * Maximum day supported.
 * @type number
 */
var MAX_DAY = 31

/**
 * Minimum month supported.
 * @type number
 */
var MIN_MONTH = 1

/**
 * Maximum year supported.
 * @type number
 */
var MAX_MONTH = 12

/**
 * Maximum hour supported.
 * @type number
 */
var MAX_HOUR = 23

/**
 * Maximum minute supported.
 * @type number
 */
var MAX_MINUTE = 59

/**
 * Maximum seconds supported.
 * @type number
 */
var MAX_SECONDS = 59

# the number of days in each month of the year
# the -1 is a placeholder for proper indexing
var _days_in_month = [-1, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 31, 31]

var _date_ends = ['th', 'st', 'nd', 'rd', 'th', 'th', 'th', 'th', 'th', 'th']


def _check_int_field(field, name) {
  if !is_int(field)
    raise TypeError('${name} must be an integer')
}

def _check_date_fields(year, month, day, hour, minute, seconds) {
  _check_int_field(year, 'year')
  _check_int_field(month, 'month')
  _check_int_field(day, 'day')

  if MIN_YEAR > year or year > MAX_YEAR
    raise ValueError('year must be in ${MIN_YEAR}..${MAX_YEAR}')
  if 1 > month or month > 12
    raise ValueError('month must be in 1..12')
  var dim = _dim(year, month)   
  if 1 > month or month > dim
    raise ValueError('month must be in 1..${dim}')

  _check_int_field(hour, 'hour')
  _check_int_field(minute, 'minute')
  _check_int_field(seconds, 'seconds')

  if 0 > hour or hour > 23
    raise ValueError('hour must be in 0..23')
  if 0 > minute or minute > 59
    raise ValueError('minute must be in 0..59')
  if 0 > seconds or seconds > 59
    raise ValueError('seconds must be in 0..59')
}



# the name of months in a year
# the nil is meant for proper indexing
var _months_in_year = [
  nil,
  'January', 'February', 'March', 'April', 'May', 'June', 
  'July', 'August', 'September', 'October', 'November', 'December'
]

# the short name of months in a year
# the nil is meant for proper indexing
var _months_in_year_short = [
  nil,
  'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun',
  'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'
]

# the short name of days in a week
# the nil is meant for proper indexing
var _weekdays_short = [
  'Sun',
  'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'
]

# the long name of days in a week
# the nil is meant for proper indexing
var _weekdays_long = [
  'Sunday',
  'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday', 'Sunday'
]

# internal declaration to get the
# days before month
def _days_before_month(m, g) {
  var days_before_month = []
  var days = 0
  for f in _days_in_month[m,MAX_MONTH+1] {
    days_before_month.append(days)
    days += f 
  }
  return days_before_month[g - m]
}

def _days_before_year(year) {
  var y = year - 1
  return y*365 + y//4 - y//100 + y//400
}

def _is_leap(year) {
  return year % 4 == 0 and 
    (year % 100 != 0 or year % 400 == 0)
}

def _ymd_to_ordinal(year, month, day) {
  assert month >= 1 and month <= 12, 'month must be in 1..12'

  var dim = month == 2 and _is_leap(year) ? 29 : _days_in_month[month]
  
  assert day >= 1 and day <= dim, 'day must be in 1..${dim}'
  return (_days_before_year(year) +
          _days_before_month(0, month) +
          day)
}

def _dim(year, month) {
  if month == 2 and _is_leap(year)
    return 29
  return _days_in_month[month]
}

/**
 * Returns a dictionary representing the current time without
 * timezone adjustment.
 * 
 * Example,
 * 
 * ```zuri-repl
 * %> echo date.gmtime()
 * {year: 2022, month: 3, day: 5, week_day: 6, year_day: 63, hour: 17, minute: 30, 
 * seconds: 55, microseconds: 620290, is_dst: false, zone: UTC, gmt_offset: 0}
 * ```
 * 
 * @returns dictionary
 */
def gmtime() {
  return _date.gmtime()
}

/**
 * Returns a dictionary representing the current time after
 * adjusting for the current timezone
 * 
 * Example:
 * 
 * ```zuri-repl
 * %> echo date.localtime()
 * {year: 2022, month: 3, day: 5, week_day: 6, year_day: 63, hour: 18, minute: 18, 
 * seconds: 35, microseconds: 598166, is_dst: false, zone: WAT, gmt_offset: 3600}
 * ```
 * 
 * @returns dictionary
 */
def localtime() {
  return _date.localtime()
}

/**
 * Convert the broken-out time into a time value with the same encoding as 
 * that of the values returned by the time() function (that is, seconds
 *  from the Epoch, UTC) according to the timezone settings.
 * 
 * <br>
 * Example:
 * 
 * ```zuri-repl
 * %> import date
 * %> echo date.mktime(2021, 2, 12, 13, 43, 11, false)
 * 1613133791
 * ```
 * 
 * @param number year
 * @param number month
 * @param number day
 * @param number hour
 * @param number minute
 * @param number seconds
 * @param bool is_dst
 * @returns number
 */
def mktime(year, month, day, hour, minute, seconds, is_dst) {
  return _date.mktime(year, month, day, hour, minute, seconds, is_dst)
}

/**
 * Date and Time manipulation class
 * 
 * A date here refers to a calendar datetime consisting of year, month, day, hour, 
 * minute and seconds. 
 * 
 * The `Date` class manages both Date and DateTime and this 
 * module does not make any distinction between the two as Date is a subset of 
 * DateTime.
 * 
 * Example,
 * 
 * ```zuri-repl
 * %> import date
 * %> var d = date(2021)
 * %> to_string(d)
 * '<Date year: 2021, month: 1, day: 1, hour: 0, minute: 0, seconds: 0>'
 * %> d = date()
 * %> to_string(d)
 * '<Date year: 2022, month: 3, day: 5, hour: 19, minute: 25, seconds: 58>'
 * ```
 * 
 * @serializable
 * @printable
 * @number
 */
class Date {

  /**
   * @note All arguments are optional
   * @note When no argument is given, the date will be set to the current system date.
   * @param number? year
   * @param number? month
   * @param number? day
   * @param number? hour
   * @param number? minute
   * @param number? seconds
   * @param number? microseconds
   * @param bool? is_dst
   * @constructor 
   */
  Date(year, month, day, hour, minute, seconds, microseconds) {

    if year {
      self.year = year

      if month self.month = month
      else self.month = MIN_MONTH

      if day self.day = day
      else self.day = MIN_DAY

      if hour self.hour = hour
      else self.hour = 0

      if minute self.minute = minute
      else self.minute = 0

      if seconds self.seconds = seconds
      else self.seconds = 0

      if microseconds self.microseconds = microseconds
      else self.microseconds = 0
    
      # check for valid input values
      _check_date_fields(self.year, self.month, self.day, 
          self.hour, self.minute, self.seconds)


      self.week_day = self.weekday()
      self.year_day = _days_before_month(1, self.month) + self.day
      self.zone = 'UTC'
      self.is_dst = false
      self.gmt_offset = 0

    } else {
      var date = localtime()

      self.year = date.year
      self.month = date.month
      self.day = date.day
      self.hour = date.hour
      self.minute = date.minute
      self.seconds = date.seconds
      self.week_day = date.week_day
      self.year_day = date.year_day
      self.zone = date.zone
      self.is_dst = date.is_dst
      self.gmt_offset = date.gmt_offset
      self.microseconds = date.microseconds
    }
  }

  /** 
   * Returns true if the year is a leap year or false otherwise.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2018).is_leap()
   * false
   * %> date(2020).is_leap()
   * true
   * ```
   * 
   * @returns bool
   */
  is_leap() {
    return _is_leap(self.year)
  }

  /** 
   * Returns the number of days in the year preceding the first
   * day of the month.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 5, 11).days_before_month(7)
   * 142
   * ```
   * 
   * @returns number
   */
  days_before_month(month) {
    if !is_number(month) {
      raise TypeError('number expected, ${typeof(month)} given')
    }

    assert month >= 1 and month <= 12, 'month must be in 1..12'

    var start = self.month, end = month, day = self.day, comparator = end < 2
    if self.month > month {
      start = month
      end = self.month 
      day = -(self.day)
      comparator = start <= 2
    }

    var days_before = _days_before_month(start, end - start)
    if comparator and self.is_leap() {
      return days_before + 1 - day
    }

    # when going back in time, we must remember not to count today
    if self.month > month
      return -(days_before - day - 1)

    return days_before - day
  }

  /** 
   * Returns the number of days before January 1st of year.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 5, 11).days_before_year(2024)
   * 811
   * ```
   * 
   * @param int year
   * @returns number
   */
  days_before_year(year) {
    assert is_int(year), 'integer expected'
    assert year >= MIN_YEAR and year <= MAX_YEAR,
      'year must be in 1..' + MAX_YEAR
    assert year > self.year, 'year must be greater than current year'

    var days_left_in_year
    if self.month < MAX_MONTH {
      days_left_in_year = self.days_before_month(MAX_MONTH) + 
          _days_in_month[MAX_MONTH]
    } else {
      days_left_in_year = _days_in_month[MAX_MONTH] - self.day
    }

    var days_in_year = (self.is_leap() ? 366 : 365) - days_left_in_year
      
    var days_till_today = _days_before_year(self.year) + days_in_year
    var days_before_year = _days_before_year(year)

    return days_before_year - days_till_today
  }

  /** 
   * Returns the number of days in month for the specified year.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 6).days_in_month()
   * 30
   * ```
   * 
   * @returns number
   */
  days_in_month() {
    return _dim(self.year, self.month)
  }

  /** 
   * Returns the numbered day of the week.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 5, 11).weekday()
   * 2
   * ```
   * 
   * @returns number
   */
  weekday() {
    var day = self.day, month = self.month, year = self.year

    if month < 3 {
      day += year--
    } else day += year - 2

    return int(23 * month/9 + day + 4 + year/4 - year/100 + year/400) % 7
  }

  /**
   * Returns the number of the current week in the year.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 5, 11).week_number()
   * 19
   * ```
   * 
   * @returns number
   */
  week_number() {
    var year = self.year
    var d1w1 = ((11 - Date(year, 1, 1).week_day) % 7) - 3
    var tv

    if self.year_day < d1w1 {
      d1w1 = ((11 - Date(year - 1, 1, 1).week_day) % 7) - 3
    } else {
      tv = ((11 - Date(year + 1, 1, 1).week_day) % 7) - 3
      if self.year_day >= tv {
        d1w1 = tv
        year++
      }
    }

    return int((self.year_day - d1w1) / 7) + 1
  }

  /**
   * Formats the current date based on the specified string
   * 
   * Zuri's Date formatting table
   * 
   * Character | Description                                               | Example
   * ----------|-----------------------------------------------------------|-----------------------------------
   * A         | uppercase Ante meridian and Post meridian                 | AM or PM
   * a         | lowercase Ante meridian and Post meridian                 | am or pm
   * d         | day of the month with leading zero                        | 01 to 31
   * D         | textual representation of a day, three letters            | Mon - Sun
   * j         | day of the month without leading zero                     | 1 to 31
   * l         | full textual representation of the day of the week        | Monday - Sunday
   * N         | ISO-8601 numeric representation of the day of the week    | 1 - 7
   * S         | English ordinal suffix for the day of the month           | st, nd, rd or th
   * w         | numeric representation of the day of the week             | 0 - 6
   * z         | the day of the year (starting from 0)                     | 0 - 365
   * W         | ISO-8601 week number of year, weeks starting on Monday    | E.g. 33 (the 33rd week of the year)
   * F         | full textual representation of a month                    | January - December
   * m         | numeric representation of a month, with leading zeros     | 01 - 12
   * n         | numeric representation of a month, without leading zeros  | 1 - 12
   * M         | short textual representation of a month, three letters    | Jan - Dec
   * t         | number of days in the given month                         | 28 - 31
   * L         | whether it's a leap year                                  | 1 if true, 0 otherwise
   * y         | two digit representation of a year                        | e.g. 09 or 99
   * Y         | full numeric representation of a year using 4 digits      | e.g. 2009 or 1999
   * h         | 12 hour format of an hour with leading zeros              | 01 - 12
   * H         | 24 hour format of an hour with leading zeros              | 01 - 24
   * g         | 12 hour format of an hour without leading zeros           | 1 - 12
   * G         | 24 hour format of an hour without leading zeros           | 1 - 24
   * i         | minutes with leading zero                                 | 00 - 59
   * s         | seconds with leading zero                                 | 00 - 59
   * u         | microseconds                                              | e.g. 987654
   * v         | milliseconds                                              | e.g. 987
   * e         | timezone identifier                                       | e.g. GMT, UTC, WAT
   * I         | whether or not the date is in daylight saving time        | 1 for true, 0 otherwise
   * O         | difference to GMT without colon between hours and minutes | e.g. +0100
   * P         | difference to GMT with colon between hours and minutes    | e.g. +01:00
   * Z         | timezone offset in seconds                                | -43200 - 50400
   * c         | ISO 8601 date                                             | e.g. 2020-03-04T15:19:21+00:00
   * r         | RFC 2822 formatted date                                   | e.g. Thu, 21 Dec 2000 16:01:07 +0200
   * 
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date().format('F d, Y g:i A')
   * 'March 05, 2022 6:24 PM'
   * ```
   * 
   * You can prevent a format character in the format string from being expanded by escaping it with a 
   * preceding backslash. If the character with a backslash is already a special sequence, you may need 
   * to also escape the backslash.
   * 
   * For example:
   * 
   * ```zuri-repl
   * %> date().format('l jS \o\\f F Y h:i:s A')
   * 'Wednesday 17th of May 2021 01:39:08 PM'
   * ```
   * 
   * @param string format
   * @returns string
   */
  format(format) {
    if !is_string(format) {
      raise TypeError('string expected, ${typeof(format)} given')
    }

    var result = ''
    iter var i = 0; i < format.length(); i++ {
      using format[i] {
        when '\\' {
          i++
          result += format[i]
        }
        when 'a' {
          if self.hour >= 12 result += 'pm'
          else result += 'am'
        }
        when 'A' {
          if self.hour >= 12 result += 'PM'
          else result += 'AM'
        }
        when 'd' {
          if self.day <= 9 result += '0${self.day}'
          else result += self.day
        }
        when 'j' {
          result += self.day
        }
        when 'D' {
          result += _weekdays_short[self.week_day]
        }
        when 'l' {
          result += _weekdays_long[self.week_day]
        }
        when 'N' {
          result += self.week_day
        }
        when 'w' {
          result += self.week_day - 1
        }
        when 'z' {
          result += _days_before_month(0,self.month) + self.day
        }
        when 'W' {
          result += self.week_number()
        }
        when 'm' {
          if self.month <= 9 result += '0${self.month}'
          else result += self.month
        }
        when 'n' {
          result += self.month
        }
        when 't' {
          result += _days_in_month[self.month]
        }
        when 'L' {
          if self.is_leap() result += 1
          else result += 0
        }
        when 'H' {
          result += self.hour <= 9 ? '0${self.hour}' : self.hour
        }
        when 'h' {
          var hour = self.hour
          if hour > 12 hour -= 12

          result += hour <= 9 ? '0${hour}' : hour
        }
        when 'M' {
          result += _months_in_year_short[self.month]
        }
        when 'F' {
          result += _months_in_year[self.month]
        }
        when 'g' {
          result += self.hour <= 12 ? self.hour : self.hour - 12
        }
        when 'G' {
          result += self.hour
        }
        when 'i' {
          result += self.minute <= 9 ? '0${self.minute}' : self.minute
        }
        when 's' {
          result += self.seconds <= 9 ? '0${self.seconds}' : self.seconds
        }
        when 'u' {
          result += self.microseconds
        }
        when 'v' {
          result += int(self.microseconds / 1000)
        }
        when 'S' {
          if (self.day % 100) >= 11 and (self.day % 100) <= 13 {
            result += 'th'
          } else {
            result += _date_ends[self.day % 10]
          }
        }
        when 'y' {
          result += to_string(self.year)[2,-1]
        }
        when 'Y' {
          result += self.year
        }
        when 'e' {
          result += self.zone
        }
        when 'I' {
          result += self.is_dst ? 1 : 0
        }
        when 'O' {
          var hour = int(self.gmt_offset / 3600)
          var minute = self.gmt_offset % 60

          if hour > 0 {
            result += hour <= 9 ? '+0${hour}' : '+${hour}'
          } else if hour == 0 {
            result += minute < 0 ? '-00' : '+00'
          } else {
            result +=  hour >= -9 ? '-0${hour}' : '-${hour}'
          }

          result += minute <= 9 ? '0${minute}' : minute
        }
        when 'P' {
          var hour = int(self.gmt_offset / 3600)
          var minute = self.gmt_offset % 60

          if hour > 0 {
            result += hour <= 9 ? '+0${hour}' : '+${hour}'
          } else if hour == 0 {
            result += minute < 0 ? '-00' : '+00'
          } else {
            result += hour >= -9 ? '-0${hour}' : '-${hour}'
          }

          result += minute <= 9 ? ':0${minute}' : ':${minute}'
        }
        when 'Z' {
          result += self.gmt_offset
        }
        when 'c' {
          result += self.format('Y-m-dTh:i:sP')
        }
        when 'r' {
          result += self.format('D, d M Y h:i:s O')
        }
        default {
          result += format[i]
        }
      }
    }

    return result
  }

  /**
   * Returns the HTTP date representation of the current date.
   * 
   * For example,
   * 
   * ```zuri-repl
   * %> date().http()
   * 'Sat, 05 Mar 2022 06:23:32 GMT'
   * ```
   * 
   * @returns string
   */
  http() {
    return self.format('D, d M Y h:i:s') + ' GMT'
  }

  /**
   * Converts the current date to a julian day and time.
   * 
   * Example,
   * 
   * ```zuri-repl
   * %> date(2021, 5, 11).jd()
   * 2459345
   * ```
   * 
   * @returns number
   */
  jd() {
    # calculate the julian day i.e. Y-m-d value
    var jday = int(367 * self.year - 7 * (self.year + (self.month + 9) / 12) / 4 +
              275 * self.month / 9 +
              self.day + 1721014)

    # calculate the time i.e. h:m:s value
    var jdate = jday - 0.5
    jdate += (self.hour + 12) / 24.0
    jdate += self.minute / 1440.0
    jdate += self.seconds / 86400.0

    return jdate
  }

  /**
   * Returns unix `mktime` equivalent of the current date.
   * 
   * @returns number
   * @deprecated - Use `to_time()` instead as it offers more precision.
   */
  unix_time() {
    return _date.mktime(self.year, self.month, self.day, self.hour, 
              self.minute, self.seconds, self.is_dst)
  }

  /**
   * Returns the Epoch timestamp in seconds for the given date.
   * 
   * @returns number
   */
  to_time() {
    var years = self.year - 1970
    
    var tm = (_days_before_year(self.year) - _days_before_year(1970)) * 86400
        tm += (_days_before_month(1, self.month) + self.day) * 86400
        tm += self.hour * 3600
        tm += self.minute * 60
        tm += self.seconds
  
    tm -= self.gmt_offset
    tm += self.microseconds == 0 ? 0 : (self.microseconds / 1000000)
  
    return tm
  }

  /**
   * Returns a string representation of the date
   * 
   * @returns string
   */
  to_string() {
    return '<Date year: ${self.year}, month: ${self.month}, day: ${self.day}, hour: ' +
        '${self.hour}, minute: ${self.minute}, seconds: ${self.seconds}, micro_seconds: ${self.microseconds}>'
  }

  /**
   * Returns the date object as a dictionary.
   * 
   * @returns dict
   */
  to_dict() {
    return {
      year: self.year,
      month: self.month,
      day: self.day,
      hour: self.hour,
      minute: self.minute,
      seconds: self.seconds,
      microseconds: self.microseconds,
      gmt_offset: self.gmt_offset,
      is_dst: self.is_dst,
      zone: self.zone,
    }
  }

  @to_string() {
    return self.to_string()
  }

  @to_json() {
    return self.to_dict()
  }

  @to_number() {
    return self.to_time()
  }

  @to_abs() {
    return self.to_time()
  }
}


/** 
 * Returns a date object from a unix timestamp.
 * 
 * Example,
 * 
 * ```zuri-repl
 * %> to_string(date.from_time(time()))
 * '<Date year: 2022, month: 3, day: 5, hour: 18, minute: 34, seconds: 1>'
 * ```
 * 
 * @note Time must be in seconds.
 * @param number time
 * @returns Date
 */
def from_time(time) {
  if !is_number(time) {
    raise TypeError('number expected, ${typeof(time)} given')
  }

  var microseconds = 0
  if time > 9999999999 { 
    if time <= 9999999999999 {
      # its in milliseconds
      time /= 1000
      microseconds = ((time - (time // 1)) * 1000 % 1000 // 1) / 1000
      time //= 1
    } else if time <= 9999999999999999 {
      # its in microseconds
      time /= 1000000
      microseconds = ((time - (time // 1)) * 1000000 % 1000000 // 1) / 1000000
      time //= 1
    } else {
      raise RangeError('time out of range')
    }
  }

  var _DI400Y = _days_before_year(401) # number of days in 400 years
  var _DI100Y = _days_before_year(101) # number of days in 100 years
  var _DI4Y = _days_before_year(5)     # number of days in 4 years

  assert _DI4Y == 4 * 365 + 1
  assert _DI400Y == 4 * _DI100Y + 1
  assert _DI100Y == 25 * _DI4Y - 1

  var _SI1D = 86400 # seconds in a day
  var _DTUNIX = 719162 # days from UTC year 1 to unix year 1970

  var n = to_int(time / _SI1D + _DTUNIX)

  # n is a 1-based index, starting at 1-Jan-1.  The pattern of leap years
  # repeats exactly every 400 years.  The basic strategy is to find the
  # closest 400-year boundary at or before n, then work with the offset
  # from that boundary to n.  Life is much clearer if we subtract 1 from
  # n first -- then the values of n at 400-year boundaries are exactly
  # those divisible by _DI400Y:
  #
  #     D  M   Y            n              n-1
  #     -- --- ----        ----------     ----------------
  #     31 Dec -400        -_DI400Y       -_DI400Y -1
  #      1 Jan -399         -_DI400Y +1   -_DI400Y      400-year boundary
  #     ...
  #     30 Dec  000        -1             -2
  #     31 Dec  000         0             -1
  #      1 Jan  001         1              0            400-year boundary
  #      2 Jan  001         2              1
  #      3 Jan  001         3              2
  #     ...
  #     31 Dec  400         _DI400Y        _DI400Y -1
  #      1 Jan  401         _DI400Y +1     _DI400Y      400-year boundary
  n--
  var n400 = to_int(n / _DI400Y)
  n %= _DI400Y
  var year = n400 * 400 + 1   # ..., -399, 1, 401, ...

  # Now n is the (non-negative) offset, in days, from January 1 of year, to
  # the desired   Now compute how many 100-year cycles precede n.
  # Note that it's possible for n100 to equal 4!  In that case 4 full
  # 100-year cycles precede the desired day, which implies the desired
  # day is December 31 at the end of a 400-year cycle.
  var n100 = to_int(n / _DI100Y)
  n %= _DI100Y

  # Now compute how many 4-year cycles precede it.
  var n4 = to_int(n / _DI4Y)
  n %= _DI4Y

  # And now how many single years.  Again n1 can be 4, and again meaning
  # that the desired day is December 31 at the end of the 4-year cycle.
  var n1 = to_int(n / 365)
  n %= 365

  year += n100 * 100 + n4 * 4 + n1
  var month, day

  if n1 == 4 or n100 == 4 {
    assert n == 0
    year -= 1
    month = 12
    day = 31
    # return Date(year - 1, 12, 31)
  } else {

    # Now the year is correct, and n is the offset from January 1.  We find
    # the month via an estimate that's either exact or one too large.
    var leapyear = n1 == 3 and (n4 != 24 or n100 == 3)
    assert leapyear == _is_leap(year)

    month = (n + 50) >> 5
    var preceding = _days_before_month(0, month) + 
          to_number(month > 2 and leapyear)
    
    if preceding > n {  # estimate is too large
      month -= 1
      preceding -= _days_in_month[month] + 
          to_number(month == 2 and leapyear)
    }
    day = n - preceding + 1
  }

  var today = time - ((_ymd_to_ordinal(year, month, day) - _DTUNIX) * _SI1D)
  var hour = to_int(today / 3600) - MAX_HOUR # where 3600 is number of seconds in hour
  
  if hour == MAX_HOUR + 1 hour = 0
  # else if hour < 1 hour += MAX_HOUR + 1
  else if hour < 1 {
    if hour + MAX_HOUR == MAX_HOUR {
      hour = 0
      day++
    }
    else hour += MAX_HOUR + 1
  }

  var now = today % 3600

  var minute = to_int(now / 60) # 60 seconds in a minute
  now %= 60

  # Now the year and month are correct, and n is the offset from the
  # start of that month:  we're done!
  return Date(year, month, day, hour, minute, int(now))
}



/**
 * Returns a date instance representing the julian date.
 * 
 * Example,
 * 
 * ```zuri-repl
 * %> to_string(date.from_jd(22063))
 * '<Date year: 2022, month: 3, day: 5, hour: 18, minute: 35, seconds: 0>'
 * ```
 * 
 * @param number jdate
 * @returns number
 */
def from_jd(jdate) {
  if !is_number(jdate) {
    raise TypeError('number expected, ${typeof(jdate)} given')
  }

  if jdate < 0 {
    raise ValueError('Invalid julian date')
  }

  var x = jdate + 0.5, z = int(x), f = x - z
  var y = int((z - 1867216.25) / 36524.25)
  var a = z + 1 + y - int(y / 4), b = a + 1524
  var c = int((b - 122.1) / 365.25), d = int(365.25 * c)
  var g = int((b - d) / 30.6001)

  var month = g - (g < 13.5 ? 1 : 13)

  # if month is january or february use 4715 else, 4716
  var year = c - (month < 2.5 ? 4715 : 4716)

  var ut = b - d - int(30.6001 * g) + f
  var day = int(ut)

  # calculate time
  ut -= int(ut)
  ut *= 24

  var hour = int(ut)

  ut -= int(ut)
  ut *= 60

  var minute = int(ut)

  ut -= int(ut)
  ut *= 60

  var seconds = int(ut)

  return Date(year, month, day, hour, minute, seconds)
}

/**
 * Returns a new `Date` instance representing the given system date or the current date if no argument is specified.
 * 
 * @param number? year
 * @param number? month
 * @param number? day
 * @param number? hour
 * @param number? minute
 * @param number? seconds
 * @param bool? is_dst
 * @returns Date
 * @default
 */
def date(year, month, day, hour, minute, seconds) {
  return Date(year, month, day, hour, minute, seconds)
}
