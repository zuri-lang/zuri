catch {
  raise Exception('there was an exception here')
} as error

if error {
  echo error.type
  echo error.message
  echo error.stacktrace
}

def test() {
  def another_test() {
    def a_more_nested_test() {
      x = [1, 2, 3, 4, 5]
      raise Exception('a_more_nested_test died')
      echo [6, 7, 8, 9, 10]
    }

    a_more_nested_test()
  }

  another_test()
}

def test_run() {
  catch {
    test()
  } as e

  if e {
    echo 'Error occurred: ' + e.message
  }

  echo e
}

test_run()

catch {
  echo [][25]
} # nothing should happen

echo "The last catch didn't throw anything and wasn't bound to any variable."

catch {
  # raise Exception('First exception thrown')
  catch {
    raise Exception('Second exception thrown')
  } as e

  if e {
    echo e.message
  }
} as e

if e {
  echo e.message
}

catch {
  var i = 10
  echo i

  catch {
    echo [1,2,3][8]
  } as e

  echo 'Despite the error, I run because I am in finally'

  if e {
    echo '\nMessage: ${e.message}'
  }
} as e

if e {
  echo e
}

def run() {
  catch {
    raise Exception('I am a thrown exception')
    raise Exception('Second exception we will never reach')
  } as e

  if e {
    echo '\nCatching exception...'
    echo 'Exception message: ${e.message}'
    echo 'Exception trace: ${e.stacktrace}'
  }
}
run()

catch {
  echo '\nTry block called'
}

echo 'Final block called\n'

catch {
  echo [][10]
}

echo 'Error occurred, but I will still run'
