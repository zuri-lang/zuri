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
# By Richard Ore
# Copied from https://github.com/smarr/are-we-fast-yet/blob/master/benchmarks/JavaScript/permute.js


class Permute {
  Permute() {
    self.count = 0
    self.v = 0
  }

  benchmark() {
    self.count = 0
    self.v = [0] * 6
    self.permute(6)
    return self.count
  }

  permute(n) {
    self.count++
    if n != 0 {
      var n1 = n - 1
      self.permute(n1)
      
      for i in n1..(-1) {
        self.swap(n1, i)
        self.permute(n1)
        self.swap(n1, i)
      }
    }
  }

  swap(i, j) {
    var tmp = self.v[i]
    self.v[i] = self.v[j]
    self.v[j] = tmp
  }
}

var start = microtime()
echo Permute().benchmark() == 8660
var end = microtime()

echo 'Time taken = ${(end - start) / 1000000} seconds'

