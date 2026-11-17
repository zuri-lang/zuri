# Function Methods

Every method of the built-in `function` type, with its signature, what
it returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`name()`](#name) | `string` | The function's declared name. |
| [`arity()`](#arity) | `number` | How many parameters the function declares, counting a variadic one as a single parameter. |
| [`is_variadic()`](#is_variadic) | `bool` | Whether the last parameter is variadic (`...name`). |
| [`call(...args: list)`](#call) | `any` | Calls the function with the given arguments, and returns its result. |
| [`apply(args: list)`](#apply) | `any` | Calls the function with the arguments in a list, and returns its result. |
| [`to_string()`](#to_string) | `string` | The function rendered for display, as `<function NAME(ARITY)>`, with a trailing `...` on the arity when the function is variadic. |

## `name()`

```zuri,ignore
name() -> string
```

The function's declared name. An anonymous function is named `@anon`
followed by a number, in the order the compiler found it. A bound method
reports the method's own name, not its class's name.

```zuri-repl
%> def named(a, b, ...c) {}
%> named.name()
'named'
%> print.name()
'print'
```

**Returns** `string`

## `arity()`

```zuri,ignore
arity() -> number
```

How many parameters the function declares, counting a variadic one as a
single parameter.

```zuri-repl
%> def named(a, b, ...c) {}
%> named.arity()
3
```

A method read off an instance counts the instance as its first
parameter, so a method declaring two parameters reports `3`. A foreign
function from `ffi` has no receiver and reports its C parameter list.

**Returns** `number`

## `is_variadic()`

```zuri,ignore
is_variadic() -> bool
```

Whether the last parameter is variadic (`...name`).

```zuri-repl
%> def named(a, b, ...c) {}
%> named.is_variadic()
true
```

**Returns** `bool`

## `call()`

```zuri,ignore
call(...args: list) -> any
```

Calls the function with the given arguments, and returns its result.
This is the same as calling it directly. It can make the code clearer
when the function is stored in a variable.

```zuri-repl
%> def add(a, b) { return a + b }
%> add.call(2, 3)
5
```

**Parameters**

- `args` (`...any`): the arguments to call it with

**Returns** `any`: Whatever the function returns.

**Raises** anything the called function raises.

## `apply()`

```zuri,ignore
apply(args: list) -> any
```

Calls the function with the arguments in a list, and returns its result.
`call()` takes the arguments separately, and this method takes them in a
list.

```zuri-repl
%> def add(a, b) { return a + b }
%> add.apply([2, 3])
5
%> def collect(first, ...rest) { return [first, rest] }
%> collect.apply([1, 2, 3])
[1, [2, 3]]
```

If the list is shorter than the number of parameters, the remaining
parameters are `nil`, just as when you call the function directly with
too few arguments. If it's longer, the extra arguments go into a
variadic parameter, or are discarded if there isn't one.

**Parameters**

- `args` (`list`): the arguments to call it with, in order

**Returns** `any`: Whatever the function returns.

**Raises** `TypeError` if `args` isn't a list, and anything the called
function raises.

## `to_string()`

```zuri,ignore
to_string() -> string
```

The function rendered for display, as `<function NAME(ARITY)>`, with a
trailing `...` on the arity when the function is variadic.

```zuri-repl
%> def named(a, b, ...c) {}
%> named.to_string()
'<function named(3...)>'
```

**Returns** `string`
