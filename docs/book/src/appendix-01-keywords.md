# Appendix A: Keywords

Zuri reserves 31 words. None of them can be used as the name of a variable,
function, class or parameter.

| Keyword | What it does | Covered in |
| --- | --- | --- |
| `and` | Logical "and", with short-circuiting | [Operators](ch03-03-operators.md) |
| `as` | Binds an error in `catch`, or renames an import | [Errors](ch07-00-error-handling.md), [Modules](ch08-00-modules.md) |
| `assert` | Raises an `AssertError` when its condition is falsy | [Control Flow](ch03-05-control-flow.md) |
| `break` | Exits the innermost loop | [Control Flow](ch03-05-control-flow.md) |
| `catch` | Runs a block and handles any error it raises | [Errors](ch07-00-error-handling.md) |
| `class` | Declares a class, or with `>`, an extension to a class | [Classes](ch06-01-defining-a-class.md), [Extensions](ch06-06-class-extensions.md) |
| `const` | Declares a name that can't be reassigned | [Variables](ch03-01-variables.md) |
| `continue` | Skips to the next iteration | [Control Flow](ch03-05-control-flow.md) |
| `def` | Declares a function, named or anonymous | [Functions](ch05-01-defining-functions.md) |
| `default` | The branch of a `using` that runs when nothing else matches | [Control Flow](ch03-05-control-flow.md) |
| `do` | Starts a `do`/`while` loop | [Control Flow](ch03-05-control-flow.md) |
| `echo` | Prints a value followed by a newline | [Hello, World!](ch01-02-hello-world.md) |
| `else` | The alternative branch of an `if` | [Control Flow](ch03-05-control-flow.md) |
| `false` | The boolean value false | [Data Types](ch03-02-data-types.md) |
| `for` | Iterates over anything iterable | [Control Flow](ch03-05-control-flow.md) |
| `if` | A conditional branch | [Control Flow](ch03-05-control-flow.md) |
| `import` | Loads a module | [Modules](ch08-00-modules.md) |
| `in` | Separates a `for` loop's variables from what it iterates over | [Control Flow](ch03-05-control-flow.md) |
| `iter` | The counting loop | [Control Flow](ch03-05-control-flow.md) |
| `nil` | The absence of a value | [Data Types](ch03-02-data-types.md) |
| `or` | Logical "or", with short-circuiting | [Operators](ch03-03-operators.md) |
| `parent` | The parent class's constructor, or a parent class's method | [Inheritance](ch06-02-inheritance.md) |
| `raise` | Raises an error | [Errors](ch07-00-error-handling.md) |
| `return` | Returns from the current function | [Control Flow](ch03-05-control-flow.md) |
| `self` | The current instance, inside a method | [Classes](ch06-01-defining-a-class.md) |
| `static` | Attaches a field or method to the class rather than to its instances | [Classes](ch06-01-defining-a-class.md) |
| `true` | The boolean value true | [Data Types](ch03-02-data-types.md) |
| `using` | A multi-way branch on a single value | [Control Flow](ch03-05-control-flow.md) |
| `var` | Declares a variable | [Variables](ch03-01-variables.md) |
| `when` | One branch of a `using` | [Control Flow](ch03-05-control-flow.md) |
| `while` | The conditional loop | [Control Flow](ch03-05-control-flow.md) |

## Names That Aren't Keywords

These are ordinary global names rather than reserved words, so nothing stops
you from shadowing them. But doing so is a good way to confuse whoever reads
your code next:

```text
time      sum       bytes     file      instance_of  typeof
delprop   getprop   hasprop   setprop   id           print
rand      is_bigint is_bool   is_bytes  is_callable  is_class
is_dict   is_file   is_function is_instance is_int    is_iterable
is_list   is_number is_object is_string
```

The built-in error classes are global names too: `Error`, `TypeError`,
`ValueError`, `NumericError`, `ArgumentError`, `NotImplementedError`,
`RangeError`, `AccessError`, `AssertError`, `PropertyError`,
`UndefinedError` and `ModuleNotFoundError`.

## Names Reserved by Convention

Two module-level names are provided by the runtime instead of being declared
by you: `__file__` and `__root__`. See [Modules](ch08-00-modules.md).

The lexer never produces names that start with `$`, which is how the
compiler can generate loop variables that never clash with yours.

A leading underscore marks something as private, for both class members and
module members, and the compiler enforces it in both cases.
