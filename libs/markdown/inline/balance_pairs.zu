# For each opening emphasis-like marker find a matching closing one

def _process_delimiters(delimiters) {
  var closer_idx, opener_idx, closer, opener, min_opener_idx, new_min_opener_idx,
      is_odd_match, last_jump,
      openers_bottom = {},
      max = delimiters.length()

  if max <= 0 return

  # header_idx is the first delimiter of the current (where closer is) delimiter run
  var header_idx = 0
  var last_token_idx = -2 # needs any value lower than -1
  var jumps = []

  iter closer_idx = 0; closer_idx < max; closer_idx++ {
    closer = delimiters[closer_idx]

    jumps.append(0)

    # markers belong to same delimiter run if:
    #  - they have adjacent tokens
    #  - AND markers are the same
    if delimiters[header_idx].marker != closer.marker or last_token_idx != closer.token - 1 {
      header_idx = closer_idx
    }

    last_token_idx = closer.token

    # Length is only used for emphasis-specific "rule of 3",
    # if it's not defined (in strikethrough or 3rd party plugins),
    # we can default it to 0 to disable those checks.
    closer.length = closer.length or 0

    if !closer.close continue

    # Previously calculated lower bounds (previous fails)
    # for each marker, each delimiter length modulo 3,
    # and for whether this closer can be an opener;
    # https://github.com/commonmark/cmark/commit/34250e12ccebdc6372b8b49c44fab57c72443460
    if !openers_bottom.contains(closer.marker) {
      openers_bottom[closer.marker] = [ -1, -1, -1, -1, -1, -1 ]
    }

    min_opener_idx = openers_bottom[closer.marker][(closer.open ? 3 : 0) + (closer.length % 3)]

    opener_idx = header_idx - jumps[header_idx] - 1

    new_min_opener_idx = opener_idx

    iter ; opener_idx > min_opener_idx; opener_idx -= jumps[opener_idx] + 1 {
      opener = delimiters[opener_idx]

      if opener.marker != closer.marker continue

      if opener.open and opener.end < 0 {

        is_odd_match = false

        # from spec:
        #
        # If one of the delimiters can both open and close emphasis, then the
        # sum of the lengths of the delimiter runs containing the opening and
        # closing delimiters must not be a multiple of 3 unless both lengths
        # are multiples of 3.
        if opener.close or closer.open {
          if (opener.length + closer.length) % 3 == 0 {
            if opener.length % 3 != 0 or closer.length % 3 != 0 {
              is_odd_match = true
            }
          }
        }

        if !is_odd_match {
          # If previous delimiter cannot be an opener, we can safely skip
          # the entire sequence in future checks. This is required to make
          # sure algorithm has linear complexity (see *_*_*_*_*_... case).
          last_jump = opener_idx > 0 and !delimiters[opener_idx - 1].open ?
            jumps[opener_idx - 1] + 1 :
            0

          jumps[closer_idx] = closer_idx - opener_idx + last_jump
          jumps[opener_idx] = last_jump

          closer.open  = false
          opener.end   = closer_idx
          opener.close = false
          new_min_opener_idx = -1
          # treat next token as start of run,
          # it optimizes skips in **<...>**a**<...>** pathological case
          last_token_idx = -2
          break
        }
      }
    }

    if new_min_opener_idx != -1 {
      # If match for this delimiter run failed, we want to set lower bound for
      # future lookups. This is required to make sure algorithm has linear
      # complexity.
      #
      # See details here:
      # https://github.com/commonmark/cmark/issues/178#issuecomment-270417442
      openers_bottom[closer.marker][(closer.open ? 3 : 0) + ((closer.length or 0) % 3)] = new_min_opener_idx
    }
  }
}

def balance_pairs(state) {
  var curr,
      tokens_meta = state.tokens_meta,
      max = state.tokens_meta.length()

  _process_delimiters(state.delimiters)

  iter curr = 0; curr < max; curr++ {
    if tokens_meta[curr] and tokens_meta[curr].delimiters {
      _process_delimiters(tokens_meta[curr].delimiters)
    }
  }
}

