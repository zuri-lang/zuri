# Normalize input string

# https://spec.commonmark.org/0.29/#line-ending
var NEWLINES_RE  = '/\r\n?|\n/'
var NULL_RE      = '/\0/'

def normalize(state) {
  var str

  # Normalize newlines
  str = state.src.replace(NEWLINES_RE, '\n')

  # Replace NULL characters
  # str = str.replace(NULL_RE, '\uFFFD')

  state.src = str
}

