# Test Assertions

When writing tests you often need to check that a value meets certain criterias. The `expect()` function gives you access to an array of assertions that lets you test against different conditions. This page describes assertions, how to use them and all possible assertions in Qi.


## expect()

The `expect` function is used every time you want to test a value. You will rarely call `expect` by itself. Instead, you will use `expect` along with a "matcher" function to assert something about a value. Let's say you have a method `name_of_app()` which is supposed to return the string `'qi'`. Here's how you would test that:

```zuri
test('Name of app test', @{
  it('should be qi', @{
    expect(name_of_app()).to_be('qi')
  })
})
```

In this case, `to_be` is the matcher function. There are a lot of different matcher functions, documented below, to help you test different things.

The argument to `expect` should be the value that your code produces, and any argument to the matcher should be the correct value. If you mix them up, your tests will still work, but the reporting messages show after running tests will look strange.

## Modifiers

### not()

If you know how to test something, `.not()` lets you test its opposite. For example, this code tests that the name of the application is `not` `'qi'`.

```zuri
test('Name of app test', @{
  it('should be qi', @{
    expect(name_of_app()).not().to_be('qi')
  })
})
```

## Matchers

> **NOTE:**
>
> Matchers can be nested. For example,
> 
> ```zuri
> expect(10.5).to_be_number().to_be_less_than(20)
> ```


### to_be(value)

Use `.to_be` to compare primitive values or to check referential identity of object instances. For example, this code will validate some properties of the `can` object:

```zuri
var can = {
  name: 'pamplemousse',
  ounces: 12,
}

test('the can', @{
  it('has 12 ounces', @{
    expect(can.ounces).to_be(12)
  })

  it('has a sophisticated name', @{
    expect(can.name).to_be('pamplemousse')
  })
})
```

### to_be_nil()

`.to_be_nil()` is the same as `.to_be(nil)` but the error messages are a bit nicer. So use `.to_be_nil()` when you want to check that something is nil.

```zuri
def bloop() {
  return nil
}

it('should return nil', @{
  expect(bloop()).to_be_nil()
})
```

### to_be_defined()

Use `.to_be_defined` to check that a variable is not `nil`. For example, if you want to check that a function `fetch_new_flavor_idea()` returns something, you can write:

```zuri
expect(fetch_new_flavor_idea()).to_be_defined()
```

You could also write `expect(fetch_new_flavor_idea()).not().to_be_nil()` as they are identical, but it's better practice to use the direct method.

### to_be_truthy()

Use `.to_be_truthy` when you don't care what a value is and you want to ensure a value is true in a boolean context. For example, let's say you have some application code that looks like:

```zuri
drink_some_lacroix()
if thirsty() {
  drink_more_lacroix()
}
```

You may not care what `get_errors` returns, specifically - it might return `true`, `[1]`, or anything that's true in Zuri, and your code would still work. So if you want to test you are thirsty before drinking some La Croix, you could write:

```zuri
it('should be thirsty before drinking La Croix', @{
  drink_some_lacroix()
  expect(thirsty()).to_be_truthy()
})
```

### to_be_falsy()

Use `.to_be_falsy` when you don't care what a value is and you want to ensure a value is false in a boolean context. For example, let's say you have some application code that looks like:

```zuri
drink_some_lacroix()
if !get_errors() {
  drink_more_lacroix()
}
```

You may not care what `get_errors` returns, specifically - it might return `false`, `nil`, or `-1`, and your code would still work. So if you want to test there are no errors after drinking some La Croix, you could write:

```zuri
it('does not lead to errors when drinking La Croix', @{
  drink_some_lacroix()
  expect(get_errors()).to_be_falsy()
})
```

### to_be_greater_than(number)

Use `.to_be_greater_than` to compare `received > expected` for number or `received.length() > expected` for string. For example, test that `ounces_per_can()` returns a value of more than 10 ounces:

```zuri
it('is more than 10 ounces per can', @{
  expect(ounces_per_can()).to_be_greater_than(10)
})
```

### to_be_greater_than_or_equal(number)

Use `.to_be_greater_than_or_equal` to compare `received >= expected` for number or `received.length() >= expected` for string. For example, test that `ounces_per_can()` returns a value of more than or equal to 10 ounces:

```zuri
it('is more than or equal to 10 ounces per can', @{
  expect(ounces_per_can()).to_be_greater_than_or_equal(10)
})
```

### to_be_less_than(number)

Use `.to_be_less_than` to compare `received < expected` for number or `received.length() < expected` for string. For example, test that `ounces_per_can()` returns a value of less than 10 ounces:

```zuri
it('is less than 10 ounces per can', @{
  expect(ounces_per_can()).to_be_less_than(10)
})
```

### to_be_less_than_or_equal(number)

Use `.to_be_less_than_or_equal` to compare `received <= expected` for number or `received.length() <= expected` for string. For example, test that `ounces_per_can()` returns a value of less than or equal to 10 ounces:

```zuri
it('is less than or equal to 10 ounces per can', @{
  expect(ounces_per_can()).to_be_less_than_or_equal(10)
})
```

### to_match(value)

Use `.to_match` to check that a string matches a regular expression.

For example, you might not know what exactly `essay_on_the_best_flavor()` returns, but you know it's a really long string, and the substring grapefruit should be in there somewhere. You can test this with:

```zuri
test('an essay on the best flavor', @{
  it('mentions grapefruit', @{
    expect(essay_on_the_best_flavor()).to_match('/grapefruit/i')
  })
})
```

This matcher also accepts a string, which it will try to match:

```zuri
test('grapefruits', @{
  it('should be a grape', @{
    expect('grapefruits').to_match('grape')
  })
})
```

### to_contain(item)

Use `.to_contain` when you want to check that an item is in an list or dictionary or whether a string is a substring of another string.

For example, if `get_all_flavors()` returns an list of flavors and you want to be sure that lime is in there, you can write:

```zuri
it('should contain lime', @{
  expect(get_all_flavors()).to_contain('lime')
})
```

### to_throw(error)

Use `.to_throw` to test that a function throws when it is called. For example, if we want to test that `drink_flavor('octopus')` throws, because octopus flavor is too disgusting to drink, we could write:

```zuri
it('throws on octopus', @{
  expect(@{
    drink_flavor('octopus')
  }).to_throw()
})
```

> **NOTE:**
> 
> You must wrap the code in a function, otherwise the error will not be caught and the assertion will fail.

You can provide an optional argument to test that a specific error is thrown:

- string: error message **includes** the substring
- regular expression: error message **matches** the pattern
- error object: error message is **equal to** the message property of the object
- error class: error object is **instance of** class

For example, let's say `drink_flavor()` looks like this:

```zuri
def drink_flavor(flavor) {
  if flavor == 'octopus' {
    die DisgustingFlavorError('yuck, octopus flavor')
  }
  # Do some other stuff
}
```

We could test the error thrown in several ways:

```zuri
it('throws on octopus', @{
  def drink_octopus() {
    drink_flavor('octopus')
  }

  # Test that the error message says "yuck" somewhere: these are equivalent
  expect(drink_octopus).to_throw('/yuck/')
  expect(drink_octopus).to_throw('yuck')

  # Test the exact error message
  expect(drink_octopus).to_throw('/^yuck, octopus flavor$/')
  expect(drink_octopus).to_throw(Exception('yuck, octopus flavor'))

  # Test that we get a DisgustingFlavorError
  expect(drink_octopus).to_throw(DisgustingFlavorError)
})
```

### to_have_length(number)

Use `.to_have_length` to check that an object has a .length property and it is set to a certain numeric value. For example:

```zuri
expect([1, 2, 3]).to_have_length(3)
expect('abc').to_have_length(3)
expect('').not().to_have_length(5)
```

### to_be_instance_of(class)

Use `.to_be_instance_of(class)` to check that an object is an instance of a class. This matcher uses `instance_of` underneath.

```zuri
class A {}

expect(A()).to_be_instance_of(A)
expect(A()).to_be_instance_of(Exception) # fails
```

### to_be_function(value)

Use `.to_be_function` when you want to check if a value is a function or a closure. For example, if `do_something()` is a function looking like this:

```zuri
def do_something(id) {
  if id == 1 return @{ do_another_thing() }
  else return @{ do_something_else() }
}
```

We can test that `do_something()` correctly returns a function.

```zuri
expect(do_something(1)).to_be_function()
```


### to_have_property(name, value?)

Use `.to_have_property` to check if an object has a given property. You can provide an optional value argument to compare the received property value against an expected value.

```zuri
class A {
  var name = 'something'
}

expect(A()).to_have_property('name')
expect(A()).to_have_property('name', 'something')
```

> It's important to note that when value is given, value must not be `nil`.

### to_have_method(name)

Use the `.to_have_method` to check if an object is an instance of a class having a particular method. For example, let's say you have a class `A` and `B` defined as follows:

```zuri
class A {
  testing() {}
}

class B {
  testing() {}
}
```

and you have a function `return_class()` that could return an instance of any of `A` or `B`, you can test the output of that method like,

```zuri
expect(return_class()).to_have_method('testing')
```

### to_have_decorator(name)

Use the `.to_have_decorator` to check if an object is an instance of a class having a particular decorator. For example, let's say you have a class `A` and `B` defined as follows:

```zuri
class A {
  @testing() {}
}

class B {
  @testing() {}
}
```

and you have a function `return_class()` that could return an instance of any of `A` or `B`, you can test the output of that method like,

```zuri
expect(return_class()).to_have_decorator('testing')
```

### to_be_boolean()

Use `.to_be_boolean` to check for `true` or `false` values. For example, test that `user_is_admin()` returns a value of `true` or `false`:

```zuri
it('should be true or false', @{
  expect(user_is_admin()).to_be_boolean()
})
```

### to_be_number()

Use `.to_be_number` to check that a value is a number without requiring any specific number. For example, test that `number_of_cans()` returns a valid number:

```zuri
it('should be a number', @{
  expect(number_of_cans()).to_be_number()
})
```

### to_be_string()

Use `.to_be_string` to check that a value is a string without requiring any specific content. For example, test that `name_of_king()` returns a valid string:

```zuri
it('should be a string', @{
  expect(name_of_king()).to_be_string()
})
```

### to_be_list()

Use `.to_be_list` to check that a value is a list without requiring any specific content. For example, test that `fruits()` returns a valid list:

```zuri
it('should be a string', @{
  expect(fruits()).to_be_list()
})
```

### to_be_dict()

Use `.to_be_dict` to check that a value is a dictionary without requiring any specific content. For example, test that `{age: 10}` returns a valid dictionary:

```zuri
it('should be a dictionary', @{
  expect({age: 10}).to_be_dict()
})
```

### to_be_class()

Use `.to_be_class` to check that a value is a class and not an instance. For example, test that `Exception` is actually a class:

```zuri
it('should be a list', @{
  expect(Exception).to_be_class()
})
```

### to_be_iterable()

Use `.to_be_iterable` to check that a value is an iterable whether its of basic types (e.g. String, List etc.) or an iterable class. For example, suppose we have a class `Set` defined ass follows:

```zuri
class Set {
  @iter() {}
  @itern() {}
}
```

The following test will show that it's as much an iterable as a list or dictionary can be.

```zuri
it('should be enumerable', @{
  expect([]).to_be_iterable()
  expect({}).to_be_iterable()
  expect(Set()).to_be_iterable()
})
```

### to_be_file()

Use `.to_be_file` to check that a value is a file object. For example, you can test that an handle `fh` returned by the function `get_config()` is actually a file like this:

```zuri
var fh = get_config()

expect(fh).to_be_file()
```

### to_be_bytes()

Use `.to_be_bytes` to check that a value is an array of bytes. For example,

```zuri
expect(bytes(0)).to_be_bytes()
```

