# Appendix D: Built-in Functions

These 28 functions are globals. Nothing has to be imported to reach them.

| Function | Returns |
| --- | --- |
| [`time()`](#time) | `number` |
| [`sum(...values)`](#sum) | `number` |
| [`bytes(x: number\|list)`](#bytes) | `bytes\|any` |
| [`file(path: string, mode: ?string)`](#file) | `file` |
| [`instance_of(x: any, y)`](#instance_of) | `boolean` |
| [`typeof(x: any)`](#typeof) | `string` |
| [`delprop(object: instance, name: string)`](#delprop) | `void` |
| [`getprop(object: instance, name: string)`](#getprop) | `any\|nil` |
| [`hasprop(object: instance, name: string)`](#hasprop) | `boolean` |
| [`setprop(obj: instance, prop: string, value: any)`](#setprop) | `boolean` |
| [`id(x: any)`](#id) | `number` |
| [`print(...values)`](#print) | `void` |
| [`rand(x: ?number, y: ?number)`](#rand) | `number` |
| [`is_bigint(x: any)`](#is_bigint) | `boolean` |
| [`is_bool(x: any)`](#is_bool) | `boolean` |
| [`is_callable(x: any)`](#is_callable) | `boolean` |
| [`is_class(x: any)`](#is_class) | `boolean` |
| [`is_dict(x: any)`](#is_dict) | `boolean` |
| [`is_function(x: any)`](#is_function) | `boolean` |
| [`is_instance(x: any)`](#is_instance) | `boolean` |
| [`is_int(x: any)`](#is_int) | `boolean` |
| [`is_list(x: any)`](#is_list) | `boolean` |
| [`is_number(x: any)`](#is_number) | `boolean` |
| [`is_object(x: any)`](#is_object) | `boolean` |
| [`is_string(x: any)`](#is_string) | `boolean` |
| [`is_bytes(x: any)`](#is_bytes) | `boolean` |
| [`is_file(x: any)`](#is_file) | `boolean` |
| [`is_iterable(x: any)`](#is_iterable) | `boolean` |

## `time()`

Returns the current epoch time to the microseconds resolution.

The time is returned as a floating point number where the integer 
part represents the number of seconds since the epoch and the 
fractional part represents the microseconds.

```zuri-repl
%> time()
1686787200.123456
```

- **Returns** `number`

## `sum(...values)`

Calculates the sum of all the elements passed as arguments.
Returns `0` when no argument is passed in.

Example:

```zuri
%> math.sum([1, 2, [3, 4, [5, 6]]])
21
```

- **Parameter** ...number values
- **Returns** `number`

## `bytes(x: number|list)`

If x is a number, this function returns a new `bytes` object
with length x having all its bytes set to `0x0`.

If x is a list, it returns a new `bytes` object whose contents
are the bytes specified in the list.

   bytes which can be any number between 0 and 255.

- **Note** If x is a list, then the list must only contain valid
- **Parameter** `number|list` x The number or list to convert to bytes.
- **Returns** `bytes|any`

## `file(path: string, mode: ?string)`

Returns an open file handle to the file specified in the path
in the specified mode. If the mode is not specified, the file
will be opened in the read only mode.

- **Parameter** `string` path The path to the file to open.
- **Parameter** `?string` mode The mode to open the file in.
- **Returns** `file`

## `instance_of(x: any, y)`

Returns `true` if x is an instance of the given class y or
`false` otherwise.

- **Parameter** `any` x The value to check.
- **Parameter** `class` y The class to check for.
- **Returns** `boolean`

## `typeof(x: any)`

Returns the type of the given value as a string.

- **Parameter** `any` x The value to check.
- **Returns** `string`

## `delprop(object: instance, name: string)`

Deletes the property name from the given instance of object.

   from.

- **Parameter** `instance` object The instance to delete the property
- **Parameter** `string` name The name of the property to delete.
- **Returns** `void`

## `getprop(object: instance, name: string)`

Returns the value of the property name from the given instance
of object. If the object has no such property, `nil` is
returned.

   from.

- **Parameter** `instance` object The instance to get the property
- **Parameter** `string` name The name of the property to get.
- **Returns** `any|nil`

## `hasprop(object: instance, name: string)`

Returns true if the property name exists in the given instance
of object. If the object has no such property, `false` is
returned.

   property.

- **Parameter** `instance` object The instance to check for the
- **Parameter** `string` name The name of the property to check.
- **Returns** `boolean`

## `setprop(obj: instance, prop: string, value: any)`

Sets the value of the object's property with the matching name
to the given value. If the property already exists, it
overwrites it and returns `true`, otherwise it returns
`false`.

- **Parameter** `instance` obj The object to set the property of.
- **Parameter** `string` prop The property to set.
- **Parameter** `any` value The value to set the property to.
- **Returns** `boolean`

## `id(x: any)`

Returns the unique identifier of value x within the system.
This value is also equivalent to the current address of object
x in memory.

- **Parameter** `any` x The value to get the identifier of.
- **Returns** `number`

## `print(...values)`

Prints the given arguments to standard output.

Unlike `echo` (which always appends a newline and only ever
prints one value), `print()` writes every argument
back-to-back with no separator and no trailing newline. It
also critically writes a `bytes` object as RAW bytes rather
than its `Display` text. That raw-byte path is what lets a
script stream binary output (e.g. a PBM/PNG image body one
scanline at a time).

- **Note** In the REPL, it also appends a newline at the end.
- **Parameter** ...vany values: Any number of arguments to print
- **Returns** `void`

## `rand(x: ?number, y: ?number)`

If no argument is given, returns a random number between 0 and
1. If x is given, returns a random number between 0 and x. If
   y is given, returns a random number between x and y.

- **Parameter** `?number` x The lower bound of the random number.
- **Parameter** `?number` y The upper bound of the random number.
- **Returns** `number`

## `is_bigint(x: any)`

Returns `true` if x is a bigint or `false` otherwise. A bigint is
a distinct type from `number` created either with the `n`
literal suffix (`123n`) or by an operation whose result overflows
what a regular `number` can represent exactly. `is_number(x)` and
`is_int(x)` are both `false` for a bigint even though it holds an
integer value; check `is_bigint(x)` separately when a value might
be either.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_bool(x: any)`

Returns `true` if x is a boolean or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_callable(x: any)`

Returns `true` if x is a callable or `false` otherwise.
Callables includes classes, functions, methods and closures.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_class(x: any)`

Returns `true` if x is a class or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_dict(x: any)`

Returns `true` if x is a dictionary or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_function(x: any)`

Returns `true` if x is a function or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_instance(x: any)`

Returns `true` if x is an instance of any class or `false`
otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_int(x: any)`

Returns `true` if x is an integer or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_list(x: any)`

Returns `true` if x is a list or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_number(x: any)`

Returns `true` if x is a number or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_object(x: any)`

Returns `true` if x is an object or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_string(x: any)`

Returns `true` if x is a string or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_bytes(x: any)`

Returns `true` if x is bytes or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_file(x: any)`

Returns `true` if x is a file or `false` otherwise.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`

## `is_iterable(x: any)`

Returns `true` if x is an iterable object or `false`
otherwise. Iterables includes lists, dictionaries, strings,
bytes, and instances of any class that defines both `@key()`
and `@value()` decorator functions.

- **Parameter** `any` x The value to check.
- **Returns** `boolean`
