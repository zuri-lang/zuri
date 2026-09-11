# Appendix D: Built-in Functions

These functions are available in every file with no import. They are the
parts of the language that happen to be spelled as calls rather than as
syntax.

| Function | Returns | Summary |
| --- | --- | --- |
| [`time()`](#time) | `number` | Returns the current epoch time to the microseconds resolution. |
| [`sum(...values: list)`](#sum) | `number` | Calculates the sum of all the elements passed as arguments. |
| [`bytes(x: number\|list)`](#bytes) | `bytes\|any` | If x is a number, this function returns a new `bytes` object with length x having all its bytes set to `0x0`. |
| [`file(path: string, mode: ?string)`](#file) | `file` | Returns an open file handle to the file specified in the path in the specified mode. |
| [`instance_of(x, y)`](#instance_of) | `boolean` | Returns `true` if x is an instance of the given class y or `false` otherwise. |
| [`typeof(x)`](#typeof) | `string` | Returns the type of the given value as a string. |
| [`delprop(object: instance, name: string)`](#delprop) | `void` | Deletes the property name from the given instance of object. |
| [`getprop(object: instance, name: string)`](#getprop) | `any\|nil` | Returns the value of the property name from the given instance of object. |
| [`hasprop(object: instance, name: string)`](#hasprop) | `boolean` | Returns true if the property name exists in the given instance of object. |
| [`setprop(obj: instance, prop: string, value)`](#setprop) | `boolean` | Sets the value of the object's property with the matching name to the given value. |
| [`id(x)`](#id) | `number` | Returns the unique identifier of value x within the system. |
| [`print(...values: list)`](#print) | `void` | Prints the given arguments to standard output. |
| [`rand(x: ?number, y: ?number)`](#rand) | `number` | If no argument is given, returns a random number between 0 and 1. |
| [`is_bigint(x)`](#is_bigint) | `boolean` | Returns `true` if x is a bigint or `false` otherwise. |
| [`is_bool(x)`](#is_bool) | `boolean` | Returns `true` if x is a boolean or `false` otherwise. |
| [`is_callable(x)`](#is_callable) | `boolean` | Returns `true` if x is a callable or `false` otherwise. |
| [`is_class(x)`](#is_class) | `boolean` | Returns `true` if x is a class or `false` otherwise. |
| [`is_dict(x)`](#is_dict) | `boolean` | Returns `true` if x is a dictionary or `false` otherwise. |
| [`is_function(x)`](#is_function) | `boolean` | Returns `true` if x is a function or `false` otherwise. |
| [`is_instance(x)`](#is_instance) | `boolean` | Returns `true` if x is an instance of any class or `false` otherwise. |
| [`is_int(x)`](#is_int) | `boolean` | Returns `true` if x is an integer or `false` otherwise. |
| [`is_list(x)`](#is_list) | `boolean` | Returns `true` if x is a list or `false` otherwise. |
| [`is_number(x)`](#is_number) | `boolean` | Returns `true` if x is a number or `false` otherwise. |
| [`is_object(x)`](#is_object) | `boolean` | Returns `true` if x is an object or `false` otherwise. |
| [`is_string(x)`](#is_string) | `boolean` | Returns `true` if x is a string or `false` otherwise. |
| [`is_bytes(x)`](#is_bytes) | `boolean` | Returns `true` if x is bytes or `false` otherwise. |
| [`is_file(x)`](#is_file) | `boolean` | Returns `true` if x is a file or `false` otherwise. |
| [`is_iterable(x)`](#is_iterable) | `boolean` | Returns `true` if x is an iterable object or `false` otherwise. |

## `time()`

```zuri,ignore
time() -> number
```

Returns the current epoch time to the microseconds resolution.

Example:

```zuri,ignore
%> time()
1686787200.123456
```

The time is returned as a floating point number where the integer part
represents the number of seconds since the epoch and the fractional part
represents the microseconds.

**Returns** `number`

> **Note:** The epoch time is the number of seconds that have elapsed
> since January 1, 1970 (midnight UTC/GMT).

## `sum()`

```zuri,ignore
sum(...values: list) -> number
```

Calculates the sum of all the elements passed as arguments. Returns `0`
when no argument is passed in.

Example:

```zuri,ignore
%> math.sum([1, 2, [3, 4, [5, 6]]])
21
```

**Parameters**

- `values` (`...number`)

**Returns** `number`

## `bytes()`

```zuri,ignore
bytes(x: number|list) -> bytes|any
```

If x is a number, this function returns a new `bytes` object with length
x having all its bytes set to `0x0`.

If x is a list, it returns a new `bytes` object whose contents are the
bytes specified in the list.

**Parameters**

- `x` (`number|list`) — The number or list to convert to bytes.

**Returns** `bytes|any`

> **Note:** If x is a list, then the list must only contain valid bytes
> which can be any number between 0 and 255.

## `file()`

```zuri,ignore
file(path: string, mode: ?string) -> file
```

Returns an open file handle to the file specified in the path in the
specified mode. If the mode is not specified, the file will be opened in
the read only mode.

**Parameters**

- `path` (`string`) — The path to the file to open.
- `mode` (`?string`) — The mode to open the file in.

**Returns** `file`

## `instance_of()`

```zuri,ignore
instance_of(x, y) -> boolean
```

Returns `true` if x is an instance of the given class y or `false`
otherwise.

**Parameters**

- `x` (`any`) — The value to check.
- `y` (`class`) — The class to check for.

**Returns** `boolean`

## `typeof()`

```zuri,ignore
typeof(x) -> string
```

Returns the type of the given value as a string.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `string`

## `delprop()`

```zuri,ignore
delprop(object: instance, name: string) -> void
```

Deletes the property name from the given instance of object.

**Parameters**

- `object` (`instance`) — The instance to delete the property from.
- `name` (`string`) — The name of the property to delete.

**Returns** `void`

## `getprop()`

```zuri,ignore
getprop(object: instance, name: string) -> any|nil
```

Returns the value of the property name from the given instance of
object. If the object has no such property, `nil` is returned.

**Parameters**

- `object` (`instance`) — The instance to get the property from.
- `name` (`string`) — The name of the property to get.

**Returns** `any|nil`

## `hasprop()`

```zuri,ignore
hasprop(object: instance, name: string) -> boolean
```

Returns true if the property name exists in the given instance of
object. If the object has no such property, `false` is returned.

**Parameters**

- `object` (`instance`) — The instance to check for the property.
- `name` (`string`) — The name of the property to check.

**Returns** `boolean`

## `setprop()`

```zuri,ignore
setprop(obj: instance, prop: string, value) -> boolean
```

Sets the value of the object's property with the matching name to the
given value. If the property already exists, it overwrites it and
returns `true`, otherwise it returns `false`.

**Parameters**

- `obj` (`instance`) — The object to set the property of.
- `prop` (`string`) — The property to set.
- `value` (`any`) — The value to set the property to.

**Returns** `boolean`

## `id()`

```zuri,ignore
id(x) -> number
```

Returns the unique identifier of value x within the system. This value
is also equivalent to the current address of object x in memory.

**Parameters**

- `x` (`any`) — The value to get the identifier of.

**Returns** `number`

## `print()`

```zuri,ignore
print(...values: list) -> void
```

Prints the given arguments to standard output.

Unlike `echo` (which always appends a newline and only ever prints one
value), `print()` writes every argument back-to-back with no separator
and no trailing newline. It also critically writes a `bytes` object as
RAW bytes rather than its `Display` text. That raw-byte path is what
lets a script stream binary output (e.g. a PBM/PNG image body one
scanline at a time).

**Parameters**

- `values` (`...any`) — Any number of arguments to print

**Returns** `void`

> **Note:** In the REPL, it also appends a newline at the end.

## `rand()`

```zuri,ignore
rand(x: ?number, y: ?number) -> number
```

If no argument is given, returns a random number between 0 and 1. If x
is given, returns a random number between 0 and x. If y is given,
returns a random number between x and y.

**Parameters**

- `x` (`?number`) — The lower bound of the random number.
- `y` (`?number`) — The upper bound of the random number.

**Returns** `number`

## `is_bigint()`

```zuri,ignore
is_bigint(x) -> boolean
```

Returns `true` if x is a bigint or `false` otherwise. A bigint is a
distinct type from `number` created either with the `n` literal suffix
(`123n`) or by an operation whose result overflows what a regular
`number` can represent exactly. `is_number(x)` and `is_int(x)` are both
`false` for a bigint even though it holds an integer value; check
`is_bigint(x)` separately when a value might be either.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_bool()`

```zuri,ignore
is_bool(x) -> boolean
```

Returns `true` if x is a boolean or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_callable()`

```zuri,ignore
is_callable(x) -> boolean
```

Returns `true` if x is a callable or `false` otherwise. Callables
includes classes, functions, methods and closures.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_class()`

```zuri,ignore
is_class(x) -> boolean
```

Returns `true` if x is a class or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_dict()`

```zuri,ignore
is_dict(x) -> boolean
```

Returns `true` if x is a dictionary or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_function()`

```zuri,ignore
is_function(x) -> boolean
```

Returns `true` if x is a function or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_instance()`

```zuri,ignore
is_instance(x) -> boolean
```

Returns `true` if x is an instance of any class or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_int()`

```zuri,ignore
is_int(x) -> boolean
```

Returns `true` if x is an integer or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_list()`

```zuri,ignore
is_list(x) -> boolean
```

Returns `true` if x is a list or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_number()`

```zuri,ignore
is_number(x) -> boolean
```

Returns `true` if x is a number or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_object()`

```zuri,ignore
is_object(x) -> boolean
```

Returns `true` if x is an object or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_string()`

```zuri,ignore
is_string(x) -> boolean
```

Returns `true` if x is a string or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_bytes()`

```zuri,ignore
is_bytes(x) -> boolean
```

Returns `true` if x is bytes or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_file()`

```zuri,ignore
is_file(x) -> boolean
```

Returns `true` if x is a file or `false` otherwise.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`

## `is_iterable()`

```zuri,ignore
is_iterable(x) -> boolean
```

Returns `true` if x is an iterable object or `false` otherwise.
Iterables includes lists, dictionaries, strings, bytes, and instances of
any class that defines both `@key()` and `@value()` decorator functions.

**Parameters**

- `x` (`any`) — The value to check.

**Returns** `boolean`
