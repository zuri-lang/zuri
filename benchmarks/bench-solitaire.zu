/**
 * This program solves the (English) peg solitaire board game.
 * http://en.wikipedia.org/wiki/Peg_solitaire 
 * 
 * Adapted from the Golang playground solution
 * at https://play.golang.org/p/njnvlXVIrRd
 */

var N = 11 + 1 # length of a row (+1 for \n)

# The board must be surrounded by 2 illegal
# fields in each direction so that move()
# doesn't need to check the board boundaries.
# Periods represent illegal fields,
# ● are pegs, and ○ are holes.

var board = '...........
...........
....●●●....
....●●●....
..●●●●●●●..
..●●●○●●●..
..●●●●●●●..
....●●●....
....●●●....
...........
...........'.to_list()

var dirs = [-1, -N, 1, N]

/* center is the position of the center hole if
there is a single one; otherwise it is -1. 
moves is the number of times move is called
*/
var center = -1, moves = 0

def init() {
  var n = 0

  for pos, field in board {
    if field == '○' {
      center = pos
      n++
    }
  }

  if n != 1 center = -1 # no single hole
}

/* 
move tests if there is a peg at position pos that
can jump over another peg in direction dir. If the
move is valid, it is executed and move returns true.
Otherwise, move returns false. */
def move(pos, dir) {
  moves++

  if board[pos] == '●' and board[pos+dir] == '●' and
    board[pos+2*dir] == '○' {
      board[pos] = '○'
      board[pos+dir] = '○'
      board[pos+2*dir] = '●'
      return true
  }
  return false
}

# unmove reverts a previously executed valid move.
def unmove(pos, dir) {
  board[pos] = '●'
	board[pos+dir] = '●'
	board[pos+2*dir] = '○'
}

/* 
solve tries to find a sequence of moves such that
there is only one peg left at the end; if center is
>= 0, that last peg must be in the center position.
If a solution is found, solve prints the board after
each move in a backward fashion (i.e., the last
board position is printed first, all the way back to
the starting board position). */
def solve() {
  var last = 0, n = 0

  for pos, field in board {

    # try each board position
    if field == '●' {

      # found a peg
      for dir in dirs {
        
        # try each direction
        if move(pos, dir) {
          # a valid move was found and executed,
					# see if this new board has a solution
          if solve() {
            unmove(pos, dir)
            echo ''.join(board)
            echo ''
            return true
          }

          unmove(pos, dir)
        }
      }

      last = pos
      n = n + 1
    }
  }

  # tried each possible move
  if n == 1 and (center < 0 or last == center) {
    # there's only one peg left
    echo ''.join(board)
    echo ''
    return true
  }

  # no solution found for this board
  return false
}

var start = time()

init()

if !solve() echo 'No solution found!'

echo '${moves} moves tried in ${time() - start}s'
