# `dict`

21 methods. See [Dictionaries](ch04-04-dictionaries.md) for the guided introduction.

| Method | Returns |
| --- | --- |
| [`length()`](#length) | `number` |
| [`add(key: string, value: any)`](#add) |  |
| [`set(key: string, value: any)`](#set) |  |
| [`clear()`](#clear) |  |
| [`clone()`](#clone) | `dict` |
| [`compact()`](#compact) | `dict` |
| [`contains(key: string)`](#contains) | `boolean` |
| [`extend(dict: dict)`](#extend) |  |
| [`get(key: string, default_value: any)`](#get) | `any\|nil` |
| [`keys()`](#keys) | `list` |
| [`values()`](#values) | `list` |
| [`remove(key)`](#remove) | `any\|nil` |
| [`is_empty()`](#is_empty) | `boolean` |
| [`find_key(value: any)`](#find_key) | `string\|nil` |
| [`to_list()`](#to_list) | `list` |
| [`each(callback: function)`](#each) | `void` |
| [`filter(callback: function)`](#filter) | `dict` |
| [`some(callback: function)`](#some) | `boolean` |
| [`every(callback: function)`](#every) | `boolean` |
| [`reduce(callback: function, initial: any)`](#reduce) | `any` |
| [`to_string()`](#to_string) | `string` |

## `length()`

Returns the length of the dictionary. The length of a Zuri
dictionary is equal to the number of keys it contains. i.e.
`dict.length() == dict.keys().length()`.

For example:

```zuri
%> {name: 'Zuri', version: 1}.length()
2
```

- **Returns** `number`

## `add(key: string, value: any)`

Adds a new key-value pair to the dictionary with the given
key and value.

For example:

```zuri
%> var dict = {}
%> dict.add('name', 'Zuri')
%> dict
{name: Zuri}
```

- **Parameter** `string` key
- **Parameter** `any` value

## `set(key: string, value: any)`

Sets the value of the given key to the given value in the
dictionary. If there is no existing entry for the key in the
dictionary, a new entry will be added.

For example:

```zuri
%> dict.set('name', 'New Zuri')
%> dict
{name: New Zuri}
%> dict.set('version', 1)
%> dict
{name: New Zuri, version: 1}
```

> **_@note_:** `dict.set(x, y)` is equivalent to the
> following Zuri code.
> ```zuri
> %> if dict.contains(x) {
> ..   dict[x] = 1
> .. } else {
> ..   dict.add(x, 1)
> .. }
> ```

- **Parameter** `string` key
- **Parameter** `any` value

## `clear()`

Clears the content of the dictionary.

For example:

```zuri
%> var a = {name: 'Zuri'}
%> a
{name: Zuri}
%> a.clear()
%> a
{}
```

## `clone()`

Returns a new dictionary which is a deep copy of the
original dictionary.

For example:

```zuri
%> var new_dict = dict.clone()
%> new_dict
{name: New Zuri, version: 1}
```

- **Returns** `dict`

## `compact()`

Returns a new dictionary that contains every key-value pair
in the original dictionary except for keys whose associated
value is `nil`.

For example:

```zuri
%> var dict2 = {name: 'James', age: 20, address: nil, country: nil}
%> dict2.compact()
{name: James, age: 20}
```

- **Returns** `dict`

## `contains(key: string)`

Returns `true` if any of the keys in the dictionary is equal
to _x_, `false` otherwise.

For example:

```zuri
%> dict2.contains('name')
true
%> dict2.contains('street')
false
```

- **Parameter** `string` key
- **Returns** `boolean`

## `extend(dict: dict)`

Adds all key-value pairs in dictionary _x_ to the original
dictionary.

For example:

```zuri
%> var dict = {name: 'Zuri'}
%> dict.extend({version: 1})
%> dict
{name: Zuri, version: 1}
```

- **Parameter** `dict` dict

## `get(key: string, default_value: any)`

Returns the value of the given _key_ in the dictionary. If
the given key is not defined in the dictionary and the
_default_ value is given, the default value will be
returned. Otherwise, `nil` is returned.

For example:

```zuri
%> dict.get('version')   # value exists
1
%> dict.get('age')   # value does not exist
%> dict.get('age', 6)   # value does not exist, but default is given
6
%> dict.get('version', 1.1)   # value exists and default is given
1
```

- **Parameter** `string` key
- **Parameter** `any|nil` default_value
- **Returns** `any|nil`

## `keys()`

Returns a list containing the keys in the dictionary.

For example:

```zuri
%> dict.keys()
[name, version]
```

- **Returns** `list`

## `values()`

Returns a list containing the value of all keys in the
dictionary.

For example:

```zuri
%> dict.values()
[Zuri, 1]
```

- **Returns** `list`

## `remove(key)`

Removes a given key and it's corresponding value from the
dictionary and returns the value of the key.

For example:

```zuri
%> dict = {username: 'james', email: 'a@b.c', active: true}
%> dict.remove('active')
true
%> dict
{username: james, email: a@b.c}
```

- **Parameter** `string` key
- **Returns** `any|nil`

## `is_empty()`

Returns `true` if the dictionary is empty, otherwise returns
`false`.

For example:

```zuri
%> dict.is_empty()
false
%> {}.is_empty()
true
```

- **Returns** `boolean`

## `find_key(value: any)`

Returns the key whose value is equal to _x_ in the
dictionary or `nil` if no key has the value _x_.

For example:

```zuri
%> dict.find_key('james')
'username'
%> dict.find_key('camel')
```

- **Parameter** `any` value
- **Returns** `string|nil`

## `to_list()`

Returns a list that contains a list of key and a list of
values from the dictionary.

For example:

```zuri
%> var dict = {username: 'james', email: 'a@b.c'}
%> dict.to_list()
[[username, email], [james, a@b.c]]
```

- **Returns** `list`

## `each(callback: function)`

Iterates over each key-value pair in the dictionary, calling
the provided callback function with the value and key as
arguments.

   key-value pair.
Example:
```zuri
var myDict = {a: 1, b: 2, c: 3}
myDict.each(@(value, key) {
  echo '${key}: ${value}'
})
# Output:
# a: 1
# b: 2
# c: 3
```

- **Parameter** `function` callback - The function to call for each
- **Raises** `Error` If the callback is not a function.
- **Returns** `void`

## `filter(callback: function)`

Creates a new dictionary containing only the key-value pairs
for which the provided callback function returns true. The
callback function is called with the value and key as
arguments.

   key-value pair. It should return true to keep the pair,
   or false to exclude it.
Example:
```zuri
var myDict = {a: 1, b: 2, c: 3}
var filteredDict = myDict.filter(@(value, key) {
  return value > 1
})
echo filteredDict
# Output: {'b': 2, 'c': 3}
```

- **Parameter** `function` callback - The function to test each
- **Raises** `Error` If the callback is not a function.
- **Returns** `dict`

## `some(callback: function)`

Tests whether at least one key-value pair in the dictionary
passes the test implemented by the provided callback
function. The callback function is called with the value and
key as arguments. The method returns true if the callback
returns true for any key-value pair, otherwise it returns
false.

   key-value pair. It should return true to indicate a
   passing pair, or false to indicate a failing pair.
Example:
```zuri
var myDict = {a: 1, b: 2, c: 3}
var hasGreaterThanTwo = myDict.some(@(value, key) {
  return value > 2
})
echo hasGreaterThanTwo
# Output: true
```

- **Parameter** `function` callback - The function to test each
- **Raises** `Error` If the callback is not a function.
- **Returns** `boolean`

## `every(callback: function)`

Tests whether all key-value pairs in the dictionary pass the
test implemented by the provided callback function. The
callback function is called with the value and key as
arguments. The method returns true if the callback returns
true for every key-value pair, otherwise it returns false.

   key-value pair. It should return true to indicate a
   passing pair, or false to indicate a failing pair.
Example:
```zuri
var myDict = {a: 1, b: 2, c: 3}
var allGreaterThanZero = myDict.every(@(value, key) {
  return value > 0
})
echo allGreaterThanZero
# Output: true
```

- **Parameter** `function` callback - The function to test each
- **Raises** `Error` If the callback is not a function.
- **Returns** `boolean`

## `reduce(callback: function, initial: any)`

Reduces the dictionary to a single value by iteratively
combining each key-value pair using the provided callback
function. The callback function is called with the
accumulator, value, key, and the dictionary itself as
arguments. The method returns the final accumulated value
after processing all key-value pairs in the dictionary.

   key-value pair in the dictionary. It should return the
   updated accumulator value after processing the pair.
   argument to the first call of the callback function.
Example:
```zuri
var myDict = {a: 1, b: 2, c: 3}
var sum = myDict.reduce(@(accumulator, value, key) {
  return accumulator + value
}, 0)
echo sum
# Output: 6
```

- **Parameter** `function` callback - The function to execute on each
- **Parameter** `any` initial - The initial value to use as the first
- **Raises** `Error` If the callback is not a function.
- **Returns** `any`

## `to_string()`

Returns the string representation of the dictionary.

```zuri
%> {a: 1, b: 2}.to_string()
'{a: 1, b: 2}'
```

- **Returns** `string`
