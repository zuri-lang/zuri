# This code is derived from the SOM benchmarks, see AUTHORS.md file.
#
# Copyright (c) 2015-2016 Stefan Marr <git@stefan-marr.de>
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the 'Software'), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in
# all copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED 'AS IS', WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN
# THE SOFTWARE.
# 
# Created by Richard Ore
# based on https://github.com/smarr/are-we-fast-yet/blob/master/benchmarks/JavaScript/queens.js


class Queens {
  Queens() {
    self.free_maxs = nil
    self.free_rows = nil
    self.free_mins = nil
    self.queen_rows = nil
  }

  benchmark() {
    var result = true
    for i in 0..10 {
      result = result and self.queens()
    }
    return result
  }

  queens() {
    self.free_rows = [true] * 8
    self.free_maxs = [true] * 16
    self.free_mins = [true] * 16
    self.queen_rows = [-1] * 8

    return self.place_queen(0)
  }

  place_queen(c) {
    for r in 0..8 {
      if self.get_row_column(r, c) {
        self.queen_rows[r] = c
        self.set_row_column(r, c, false)

        if c == 7 {
          return true
        }

        if self.place_queen(c + 1) {
          return true
        }

        self.set_row_column(r, c, true)
      }
    }

    return false
  }

  get_row_column(r, c) {
    return self.free_rows[r] and self.free_maxs[c + r] and
      self.free_mins[c - r + 7]
  }

  set_row_column(r, c, v) {
    self.free_rows[r] = v
    self.free_maxs[c + r] = v
    self.free_mins[c - r + 7] = v
  }
}

var start = microtime()
echo Queens().benchmark() == true
var end = microtime()

echo 'Time taken = ${(end - start) / 1000000} seconds'
