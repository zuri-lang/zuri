/**
 * This example shows the implementation of a simple rudimentary vim
 * like editor for Zuri with syntax highlighting.
 */
import io

# simple test
var tty = io.TTY(io.stdin)

var keywords = [
  'and', 'as', 'assert', 'break', 'catch', 'class',
  'continue', 'def', 'default', 'do', 'echo','else',
  'false', 'for', 'if', 'import','in', 'iter', 'nil',
  'or', 'parent','raise', 'return', 'self', 'static',
  'true', 'using', 'var', 'when', 'while'
]

def highlight(str) {
  if str.length() == 0
    return str

  var regex = '/(' +'|'.join(keywords)+ ')/'
  str = str.replace('/\\x1b\[\\d+m/', '') # firstly, strip the old highlights
  return str.replace(regex, '\x1b[32m$1\x1b[0m')
}

# go full screen by cleaning output
io.stdout.write("\x1b[2J")
io.stdout.write("\x1b[H")

echo '\x1b[33m'
echo 'A simple TTY based editor with syntax highlighting for Zuri using the io module'
echo 'Note that your cursor can move left and right.'
echo 'Nope! That\'s not how the terminal works by default.'
echo 'That\'s the power of Zuri!'
echo 'Press Ctrl+C to quit'
echo '-------------------------------------------------------------'
echo '\x1b[0m'

if !tty.set_raw() {
  echo 'Failed to enter raw mode for STDIN'
}

var history = ['']
var history_index = 0

var input = ''
var index = 0

var s
while s = io.stdin.read() {
  if ord(s) == 3 { # ctrl + c
    io.stdout.write('\x1b[1000D\n')
    io.stdout.flush()
    echo 'Exiting...'
    tty.exit_raw()
    break
  } else if ord(s) >= 32 and ord(s) <= 126 {
    if index < input.length()
      input = input[0,index] + s + input[index, input.length()]
    else
      input += s
    index++
  } else if [10, 13].contains(ord(s)) { # enter key
    io.stdout.write('\x1b[1000D\n')
    history.append(input)
    history_index++
    # you can take this out to see the output
    # echo '\nechoing...', input
    input = ''
    index = 0
  } else if ord(s) == 27 { # arrow keys
    var next1 = ord(io.stdin.read())
    var next2 = ord(io.stdin.read())
    if next1 == 91 {
      if next2 == 68 #left
        index = max(0, index - 1)
      else if next2 == 67 # right
        index = min(input.length(), index + 1)
      else if next2 == 65 { # up
        if history_index > 0 {
          input = history[history_index]
          index = input.length()
          history_index--
        } else {
          input = ''
          index = 0
        }
      } else if next2 == 66 { # down
        if history_index < history.length() - 1 {
          history_index++
          input = history[history_index]
          index = input.length()
        } else {
          input = ''
          index = 0
        }
      }
    }
  } else if ord(s) == 127 { # backspace
    if input.length() > 0 {
      if index < input.length()
        input = input[0,index - 1] + input[index,input.length()]
      else
        input = input[0,index - 1]
      index--
    }
  }

  # print out the current input-string
  io.stdout.write('\x1b[1000D') # move all the way left
  io.stdout.write('\x1b[0K') # clear the line
  io.stdout.write(highlight(input))
  io.stdout.write('\x1b[1000D') # move all the way left again
  if index > 0
    io.stdout.write('\x1b[' + index + 'C')
  io.stdout.flush()
}

# Clear the terminal (classic Nano and Vim style)
io.stdout.write("\x1b[2J")
io.stdout.write("\x1b[H")
io.stdout.flush()
