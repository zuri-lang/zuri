# Test Globals

Qi exposes a set of global functions for use in your test files by putting each of these methods and objects into the global environment. You don't have to require or import anything to use them from your test files.

## Methods

### describe(name, fn)

`describe(name, fn)` creates a block that groups together several related tests. It is the Test Suite. For example, if you have a `myBeverage` object that is supposed to be delicious but not sour, you could test it with:

```zuri
var myBeverage = {
  delicious: true,
  sour: false,
}

describe('my beverage', @{
  it('should be delicious', @{
    expect(myBeverage.delicious).to_be_truthy()
  });

  it('should be sour', @{
    expect(myBeverage.sour).to_be_falsy()
  })
})
```

You can also nest `describe` blocks if you have a hierarchy of tests:

```zuri
var binay_string_to_number = @( bin_string ) {
  if !bin_string.match('/^[01]+$/') {
    die CustomError('Not a binary number.')
  }

  return to_number('0b' + bin_string)
}

describe('binay string to number', @{
  describe('given an invalid binary string', @{
    it('throws CustomError when composed of non-numbers', @{
      expect(@{ binay_string_to_number('abc') }).to_throw(CustomError)
    })

    it('throws CustomError when having extra whitespace', @{
      expect(@{ binay_string_to_number('  100') }).to_throw(CustomError)
    })
  })

  describe('given a valid binary string', @{
    it('returns the correct number', @{
      expect(binay_string_to_number('100')).to_be(4)
    })
  })
})
```

### it(name, fn)

The `it(name, fn)` function is the entry point for tests in a test suite. For example, let's say there's a function `inches_of_rain()` that should return zero. Your whole test could be:

```zuri
it('did not rain', @{
  expect(inches_of_rain()).to_be(0)
})
```

The first argument is the test name; the second argument is a function that contains the expectations to test.

## Hooks

Qi provides a few set of hooks that allows us to meet different requirements at different stages of our test. There are essentially four stages that can be hooked into in a Qi test. And they are as expected:

- Before any test in a suite is run,
- After every test in a suite is run,
- Before a test is run,
- After a test is run.

### before_all(fn)

Runs a function before each of the tests in the test suite run. This is often useful if you want to set up some global state that will be used by many tests.

For example:

```zuri
var global_db = make_global_db()

before_all(@{
  # Clears the database and adds some testing data.
  return globalDatabase.clear(@{
    return globalDatabase.insert({testData: 'foo'})
  })
})

# Since we only set up the database once in this example, it's important
# that our tests don't modify it.
describe('Before all', @{
  it('can find things', @{
    return global_db.find('thing', {}, @(results) {
      expect(results.length()).to_be_greater_than(0)
    })
  })
})
```

Here the `before_all` ensures that the database is set up before tests run. If you want to run something before every test instead of before any test runs, use `before_each` instead.

### after_all(fn)

Runs a function after all the tests in a test suite have completed. This is often useful if you want to clean up some global setup state that is shared across tests. 

For example:

```zuri
var global_db = make_global_db()

def clean_up_db(db) {
  db.clean_up()
}

after_all(@{
  clean_up_db(global_db)
});

describe('confirming after_all works', @{
  it('can find things', @{
    return global_db.find('thing', {}, @(results) {
      expect(results.length()).to_be_greater_than(0)
    })
  })
  
  it('can insert a thing', @{
    return global_db.insert('thing', make_thing(), @(response) {
      expect(response.success).to_be_truthy()
    })
  })
})
```

Here the `after_all` ensures that `clean_up_db` is called after all tests in the describe run.

If you want to run some cleanup after every test instead of after all tests, use `after_each` instead.

### before_each(fn)

Runs a function before each of the tests in the test suite runs. This is often useful if you want to reset some global state that will be used by many tests.

For example:

```zuri
var global_db = make_global_db()

before_each(@{
  # Clears the database and adds some testing data.
  global_db.clear()
  global_db.insert({testData: 'foo'});
})

describe('confirming before_each works', @{
  it('can find things', @{
    return global_db.find('thing', {}, @(results) {
      expect(results.length()).to_be_greater_than(0)
    })
  })
  
  it('can insert a thing', @{
    return global_db.insert('thing', make_thing(), @(response) {
      expect(response.success).to_be_truthy()
    })
  })
})
```

Here the `before_each` ensures that the database is reset for each test. If you only need to run some setup code once, before any tests run, use `before_all` instead.

### after_each(fn)

Runs a function after each one of the tests in this file completes. This is often useful if you want to clean up some temporary state that is created by each test. 

For example:

```zuri
var global_db = make_global_db()

def clean_up_db(db) {
  db.clean_up()
}

after_each(@{
  clean_up_db(global_db)
})

describe('confirming after_each works', @{
  it('can find things', @{
    return global_db.find('thing', {}, @(results) {
      expect(results.length()).to_be_greater_than(0)
    })
  })
  
  it('can insert a thing', @{
    return global_db.insert('thing', make_thing(), @(response) {
      expect(response.success).to_be_truthy()
    })
  })
})
```

Here the `after_each` ensures that `clean_up_db` is called after each test runs.

If you want to run some cleanup just once, after all of the tests run, use `after_all` instead.
