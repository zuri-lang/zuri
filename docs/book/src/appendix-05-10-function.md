# Function Methods

Every method carried by a callable value, with its signature, what it
returns, and its edge cases. These apply to a named `def`, an anonymous
function, a bound method, and a built-in native function alike.

Unlike the other pages in this appendix, this one is written by hand: the
methods below live in the runtime rather than in a standard library stub,
so there is no doc block to generate them from.

| Method | Returns |
| --- | --- |
| [`name()`](#name) | `string` |
| [`arity()`](#arity) | `number` |
| [`is_variadic()`](#is_variadic) | `bool` |
| [`call(...args)`](#call) | `any` |
| [`to_string()`](#to_string) | `string` |

## `name()`

The function's declared name. An anonymous function is named `@anonN`,
numbered in the order the compiler encountered it; a bound method reports
the method's own name, not the class's.

```zuri
%> def named(a, b, ...c) {}
%> named.name()
'named'
%> print.name()
'print'
```

- **Returns** `string`

## `arity()`

How many parameters the function declares, counting a variadic one as a
single parameter.

```zuri
%> def named(a, b, ...c) {}
%> named.arity()
3
```

A bound method's arity does not count its receiver.

- **Returns** `number`

## `is_variadic()`

Whether the last parameter is variadic (`...name`).

```zuri
%> def named(a, b, ...c) {}
%> named.is_variadic()
true
```

- **Returns** `bool`

## `call()`

Calls the function with the given arguments and returns its result. The
same as calling it directly; useful when the function is held in a variable
and the call site reads better spelled out.

```zuri
%> def add(a, b) { return a + b }
%> add.call(2, 3)
5
```

- **Parameter** `...any` args The arguments to call with.
- **Returns** `any` Whatever the function returns.
- **Raises** Anything the called function raises.

## `to_string()`

The function rendered for display, as `<function NAME(ARITY)>`, with a
trailing `...` on the arity when the function is variadic.

```zuri
%> def named(a, b, ...c) {}
%> named.to_string()
'<function named(3...)>'
```

- **Returns** `string`
