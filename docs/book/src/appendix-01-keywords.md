# Appendix A: Keywords

Zuri reserves thirty-one words. None of them can be used as a variable,
function, class or parameter name.

| Keyword | What it does | Covered in |
| --- | --- | --- |
| `and` | logical conjunction, short-circuiting | [Operators](ch03-03-operators.md) |
| `as` | binds an error in `catch`, or renames an import | [Errors](ch07-00-error-handling.md), [Modules](ch08-00-modules.md) |
| `assert` | raises `AssertError` when its condition is falsy | [Control Flow](ch03-05-control-flow.md) |
| `break` | leaves the innermost loop | [Control Flow](ch03-05-control-flow.md) |
| `catch` | runs a block, intercepting anything it raises | [Errors](ch07-00-error-handling.md) |
| `class` | declares a class | [Classes](ch06-01-defining-a-class.md) |
| `const` | declares a name that cannot be reassigned | [Variables](ch03-01-variables.md) |
| `continue` | skips to the next iteration | [Control Flow](ch03-05-control-flow.md) |
| `def` | declares a function, named or anonymous | [Functions](ch05-01-defining-functions.md) |
| `default` | the fall-through branch of a `using` | [Control Flow](ch03-05-control-flow.md) |
| `do` | begins a `do`/`while` loop | [Control Flow](ch03-05-control-flow.md) |
| `echo` | prints a value and a newline | [Hello, World!](ch01-02-hello-world.md) |
| `else` | the alternative branch of an `if` | [Control Flow](ch03-05-control-flow.md) |
| `false` | the boolean false | [Data Types](ch03-02-data-types.md) |
| `for` | iterates over anything iterable | [Control Flow](ch03-05-control-flow.md) |
| `if` | conditional branch | [Control Flow](ch03-05-control-flow.md) |
| `import` | loads a module | [Modules](ch08-00-modules.md) |
| `in` | separates a `for` loop's variables from its iterable | [Control Flow](ch03-05-control-flow.md) |
| `iter` | the counting loop | [Control Flow](ch03-05-control-flow.md) |
| `nil` | the absence of a value | [Data Types](ch03-02-data-types.md) |
| `or` | logical disjunction, short-circuiting | [Operators](ch03-03-operators.md) |
| `parent` | the superclass constructor, or a superclass method | [Inheritance](ch06-02-inheritance.md) |
| `raise` | raises an error | [Errors](ch07-00-error-handling.md) |
| `return` | leaves the current function | [Control Flow](ch03-05-control-flow.md) |
| `self` | the current instance, inside a method | [Classes](ch06-01-defining-a-class.md) |
| `static` | puts a field or method on the class, not the instance | [Classes](ch06-01-defining-a-class.md) |
| `true` | the boolean true | [Data Types](ch03-02-data-types.md) |
| `using` | multi-way branch on one subject | [Control Flow](ch03-05-control-flow.md) |
| `var` | declares a variable | [Variables](ch03-01-variables.md) |
| `when` | one branch of a `using` | [Control Flow](ch03-05-control-flow.md) |
| `while` | the conditional loop | [Control Flow](ch03-05-control-flow.md) |

## Names That Are Not Keywords

These are ordinary globals, not reserved words, so nothing stops you from
shadowing one. Doing so is a good way to confuse the next reader:

```text
time      sum       bytes     file      instance_of  typeof
delprop   getprop   hasprop   setprop   id           print
rand      is_bigint is_bool   is_bytes  is_callable  is_class
is_dict   is_file   is_function is_instance is_int    is_iterable
is_list   is_number is_object is_string
```

The built-in error classes are also globals: `Error`, `TypeError`,
`ValueError`, `NumericError`, `ArgumentError`, `NotImplementedError`,
`RangeError`, `AccessError`, `AssertError`, `PropertyError`,
`UndefinedError` and `ModuleNotFoundError`.

## Reserved by Convention

Two module-level names are provided by the runtime rather than declared by
you: `__file__` and `__root__`. See [Modules](ch08-00-modules.md).

Names beginning with `$` are never produced by the lexer, which is how the
compiler synthesises loop variables that cannot collide with yours.

A leading underscore marks something private, both for class members and
for module members. The compiler enforces it in both cases.
