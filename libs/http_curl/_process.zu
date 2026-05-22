#!-- part of the http module


/**
 * process_header(header, meta_callback)
 * 
 * processes raw http headers into a dictionary and calls the meta_callback
 * function if given with the argument list [version, status]
 */
def process_header(header, meta_callback) {
  var result = {
    headers: {},
    cookies: []
  }

  if header {
    # Follow redirect headers...
    var data = header.trim().split('\r\n')

    iter var i = 0; i < data.length(); i++ {
      var d = data[i].index_of(':')
      if d > -1 {
        var key = data[i][0,d]
        var value = data[i][d + 1,].trim()

        # According to: https://datatracker.ietf.org/doc/html/rfc7230#section-3.2.6
        # A string of text is parsed as a single value if it is quoted using
        # double-quote marks
        # if value.starts_with('"') and value.ends_with('"')
        #   value = value[1,-1]

        # handle cookies in header
        if key.lower() == 'set-cookie' {
          if result.cookies.contains(key) {
            result.cookies.append(value)
          } else {
            result.cookies = [value]
          }
        } else {
          result.headers.set(key, value)
        }
      } else if(data[i].lower().starts_with('http/')){
        var split = data[i].split(' ')
        var http_version = split[0].replace('~http/~', '')

        # call back with (version, status code)
        if meta_callback meta_callback(http_version, to_number(split[1]))
      }
    }
  }

  return result
}