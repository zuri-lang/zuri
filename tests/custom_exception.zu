class Error < Exception {
  var message = 'An unexpected error has occurred'

  Error(message) {
    self.message = message or self.message
  }
}

catch {
  raise Error()
} as error

if error {
  echo error.message
  echo error.stacktrace
}

class MyCustomException < Exception {}

var x = 50

catch {
  if x == 50 {
    raise MyCustomException('Something custom happened')
  } else {
    raise Exception('Exception happened')
  }
} as error

if instance_of(error, MyCustomException) {
  echo 'A custom exception was raised'
  echo error.message
} else {
  echo 'Regular exception raised'
  echo error.message
}

class ClientError < Error {}

catch {
  raise ClientError()
} as e

if e {
  echo e.message
}
