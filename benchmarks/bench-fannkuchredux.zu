/* The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 *
 * fannkuch-redux Zuri program
 * Based on the C solution by Jeremy Zerfas
 * that was based on the Ada program by Jonathan Parker and Georg Bauhaus which in turn
 * was based on code by Dave Fladebo, Eckehard Berns, Heiner Marxen, Hongwei Xi,
 * and The Anh Tran and also the Java program by Oleg Mazurov.
 *
 * contributed by Richard Ore
 * *reset*
*/

def fannkuchredux(n) {

  # create and initialize factorial_lookup_table
  var factorial_lookup_table = [1] * (n + 1)
  iter var i = 1; i <= n; i++
    factorial_lookup_table[i] = i * factorial_lookup_table[i - 1]

  # Determine the block_size to use.
  var block_size = factorial_lookup_table[n]

  var maximum_flip_count = 0, checksum = 0,
      initial_permutation_index = 0

  var count = [0] * n, temp_permutation, current_permutation = [0] * n

  for i in 0..n current_permutation[i] = i

  iter var i = n - 1, permutation_index = initial_permutation_index; i > 0; i-- {
    var d = permutation_index / factorial_lookup_table[i]
    permutation_index %= factorial_lookup_table[i]
    count[i] = d

    temp_permutation = current_permutation.clone()

    iter var j = 0; j <= i; j++ {
      current_permutation[j] = j + d <= i ? 
        temp_permutation[j + d] : temp_permutation[j + d - i - 1]
    }
  }

  # Iterate over each permutation in the block.
  var last_permutation_index = initial_permutation_index + block_size - 1

  iter var permutation_index = initial_permutation_index; ; permutation_index++ {

    # If the first value in the current_permutation is not 1 (0) then
    # we will need to do at least one flip for the current_permutation.
    if current_permutation[0] > 0 {

      # Make a copy of current_permutation[] to work on.
      iter var i = 0; i++ < n; {
        temp_permutation[i] = current_permutation[i]
      }

      var flip_count = 1

      # Flip temp_permutation until the element at the first_value
      # index is 1 (0).
      iter var first_value = current_permutation[0]; temp_permutation[first_value] > 0; flip_count++ {

        # Record the new_first_value and restore the old
        # first_value at its new flipped position.
        var new_first_value = temp_permutation[first_value]
        temp_permutation[first_value] = first_value

        # If first_value is greater than 3 (2) then we are flipping
        # a series of four or more values so we will also need to
        # flip additional elements in the middle of the
        # temp_permutation.
        if first_value >= 3 {
          var low_index = 1, high_index = first_value - 1

          # This won't work right when n is greater than 35. 
          # This would probably be the least of your concerns 
          # since 21! won't fit into 64 bit integers and even if 
          # it did you probably wouldn't want to run this program 
          # with a value that large since it would take thousands 
          # of years to do on a modern desktop computer. ;-)
          do {
            var temp = temp_permutation[high_index]
            temp_permutation[high_index] = temp_permutation[low_index]
            temp_permutation[low_index] = temp
            low_index++
            high_index--
          } while low_index < high_index and low_index < 16 
        }

        # Update first_value to new_first_value that we recorded
        # earlier.
        first_value = new_first_value
      }

      # Update the checksum.
      if permutation_index % 2 == 0 checksum += flip_count
      else checksum -= flip_count

      # Update maximum_flip_count if necessary.
      maximum_flip_count = max(maximum_flip_count, flip_count)
    }

    # Break out of the loop when we get to the
    # last_permutation_index.
    if permutation_index >= last_permutation_index
      break

    # Generate the next permutation.
    var first_value = current_permutation[1]
    current_permutation[1] = current_permutation[0]
    current_permutation[0] = first_value

    iter var i = 1; count[i]++ > i; {
      count[i++ - 1] = 0
      
      var new_first_value = current_permutation[1] 
      current_permutation[0] = current_permutation[1]

      iter var j = 0; j++ < i; {
        current_permutation[j] = current_permutation[j + 1]
      }

      current_permutation[i] = first_value
      first_value = new_first_value
    }
  }

  # output the result
  echo '${checksum}\nPfannkuchen(${n}) = ${maximum_flip_count}'
}

var start = time()
fannkuchredux(12)
echo '\nTime taken = ${time() - start}s'
