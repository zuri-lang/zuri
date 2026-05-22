# Qi

A testing framework for Zuri programming language.

### Package Information

- **Name:** qi
- **Version:** 1.0.0
- **Homepage:** https://github.com/mcfriend99/qi.git
- **Tags:** test, zuri, testing, tdd, test-driven
- **Author:** Richard Ore <eqliqandfriends@gmail.com>
- **License:** MIT
- **Requires:** Zuri v0.0.86+

### Getting started

Qi is a Nyssa package that ships along with it and is designed to run tests are out the directory `tests` and for this reason, all tests files must reside inside the `tests` directory.

### Writing a simple test

Let's write a test for a hypothetical function that returns the product of two numbers. First, we'll create a file `prod.zu` that contains the following code:

```py
def prod(x, y) {
  return x * y
}
```

Now, let's create a test for it by creating a file `prod.test.zu` in the `tests` directory and add the following code to it.

```py
import ..prod

describe('Product test suite', @{
  it('should return 6 for 2 and 3', @{
    expect(prod(2, 3)).to_be(6)
  })
})
```

Now let's run the test. If you have installed Qi using `nyssa` (which is recommended), then you can run the following command at the root directory (the directory that contains the `tests` folder).

```
nyssa test
```

You should get an output similar to this:

```
 PASS  tests/prod.test.zu
  Product test suite
    ✔ should return 6 for 2 and 3 (1.09ms)
      ✔ expect "6" to be "6"

Test suites:  1 passed, 0 failed, 1 total
Tests:        1 passed, 0 failed, 1 total
Assertions:   1 passed, 0 failed, 1 total
Time:         1.092ms
Ran all test suites.
```

**You have successfully created your first Qi test!**

Read more documentation at: [mcfriend99.github.io/qi](https://mcfriend99.github.io/qi/)
