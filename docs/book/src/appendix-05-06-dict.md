# Dictionary Methods

Every method on the built-in `dict` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`length()`](#length) | `number` | Returns the length of the dictionary. |
| [`add(key: string, value)`](#add) |  | Adds a new key-value pair to the dictionary with the given key and value. |
| [`set(key: string, value)`](#set) |  | Sets the value of the given key to the given value in the dictionary. |
| [`clear()`](#clear) |  | Clears the content of the dictionary. |
| [`clone()`](#clone) | `dict` | Returns a new dictionary which is a deep copy of the original dictionary. |
| [`compact()`](#compact) | `dict` | Returns a new dictionary that contains every key-value pair in the original dictionary except for keys whose associated value is `nil`. |
| [`contains(key: string)`](#contains) | `boolean` | Returns `true` if any of the keys in the dictionary is equal to _x_, `false` otherwise. |
| [`extend(dict: dict)`](#extend) |  | Adds all key-value pairs in dictionary _x_ to the original dictionary. |
| [`get(key: string, default_value)`](#get) | `any\|nil` | Returns the value of the given _key_ in the dictionary. |
| [`keys()`](#keys) | `list` | Returns a list containing the keys in the dictionary. |
| [`values()`](#values) | `list` | Returns a list containing the value of all keys in the dictionary. |
| [`remove(key)`](#remove) | `any\|nil` | Removes a given key and it's corresponding value from the dictionary and returns the value of the key. |
| [`is_empty()`](#is_empty) | `boolean` | Returns `true` if the dictionary is empty, otherwise returns `false`. |
| [`find_key(value)`](#find_key) | `string\|nil` | Returns the key whose value is equal to _x_ in the dictionary or `nil` if no key has the value _x_. |
| [`to_list()`](#to_list) | `list` | Returns a list that contains a list of key and a list of values from the dictionary. |
| [`each(callback: function)`](#each) | `void` | Iterates over each key-value pair in the dictionary, calling the provided callback function with the value and key as arguments. |
| [`filter(callback: function)`](#filter) | `dict` | Creates a new dictionary containing only the key-value pairs for which the provided callback function returns true. |
| [`some(callback: function)`](#some) | `boolean` | Tests whether at least one key-value pair in the dictionary passes the test implemented by the provided callback function. |
| [`every(callback: function)`](#every) | `boolean` | Tests whether all key-value pairs in the dictionary pass the test implemented by the provided callback function. |
| [`reduce(callback: function, initial)`](#reduce) | `any` | Reduces the dictionary to a single value by iteratively combining each key-value pair using the provided callback function. |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the dictionary. |

## `length()`

```zuri,ignore
length() -> number
```

Returns the length of the dictionary. The length of a Zuri dictionary is
equal to the number of keys it contains. i.e. `dict.length() == dict.keys().length()`.

For example:

```zuri,ignore
%> {name: 'Zuri', version: 1}.length()
2
```

**Returns** `number`

## `add()`

```zuri,ignore
add(key: string, value)
```

Adds a new key-value pair to the dictionary with the given key and
value.<br>

For example:

```zuri,ignore
%> var dict = {}
%> dict.add('name', 'Zuri')
%> dict
{name: Zuri}
```

**Parameters**

- `key` (`string`)
- `value` (`any`)

## `set()`

```zuri,ignore
set(key: string, value)
```

Sets the value of the given key to the given value in the dictionary. If
there is no existing entry for the key in the dictionary, a new entry
will be added.<br>

For example:

```zuri,ignore
%> dict.set('name', 'New Zuri')
%> dict
{name: New Zuri}
%> dict.set('version', 1)
%> dict
{name: New Zuri, version: 1}
```

> **_@note_:** `dict.set(x, y)` is equivalent to the
> following Zuri code.
> ```zuri-repl
> %> if dict.contains(x) {
> ..   dict[x] = 1
> .. } else {
> ..   dict.add(x, 1)
> .. }
> ```

**Parameters**

- `key` (`string`)
- `value` (`any`)

## `clear()`

```zuri,ignore
clear()
```

Clears the content of the dictionary.<br>

For example:

```zuri,ignore
%> var a = {name: 'Zuri'}
%> a
{name: Zuri}
%> a.clear()
%> a
{}
```

## `clone()`

```zuri,ignore
clone() -> dict
```

Returns a new dictionary which is a deep copy of the original
dictionary.<br>

For example:

```zuri,ignore
%> var new_dict = dict.clone()
%> new_dict
{name: New Zuri, version: 1}
```

**Returns** `dict`

## `compact()`

```zuri,ignore
compact() -> dict
```

Returns a new dictionary that contains every key-value pair in the
original dictionary except for keys whose associated value is `nil`.

For example:

```zuri,ignore
%> var dict2 = {name: 'James', age: 20, address: nil, country: nil}
%> dict2.compact()
{name: James, age: 20}
```

**Returns** `dict`

## `contains()`

```zuri,ignore
contains(key: string) -> boolean
```

Returns `true` if any of the keys in the dictionary is equal to _x_,
`false` otherwise.<br>

For example:

```zuri,ignore
%> dict2.contains('name')
true
%> dict2.contains('street')
false
```

**Parameters**

- `key` (`string`)

**Returns** `boolean`

## `extend()`

```zuri,ignore
extend(dict: dict)
```

Adds all key-value pairs in dictionary _x_ to the original
dictionary.<br>

For example:

```zuri,ignore
%> var dict = {name: 'Zuri'}
%> dict.extend({version: 1})
%> dict
{name: Zuri, version: 1}
```

**Parameters**

- `dict` (`dict`)

## `get()`

```zuri,ignore
get(key: string, default_value) -> any|nil
```

Returns the value of the given _key_ in the dictionary. If the given key
is not defined in the dictionary and the _default_ value is given, the
default value will be returned. Otherwise, `nil` is returned.

For example:

```zuri,ignore
%> dict.get('version')   # value exists
1
%> dict.get('age')   # value does not exist
%> dict.get('age', 6)   # value does not exist, but default is given
6
%> dict.get('version', 1.1)   # value exists and default is given
1
```

**Parameters**

- `key` (`string`)
- `default_value` (`any|nil`)

**Returns** `any|nil`

## `keys()`

```zuri,ignore
keys() -> list
```

Returns a list containing the keys in the dictionary.<br>

For example:

```zuri,ignore
%> dict.keys()
[name, version]
```

**Returns** `list`

## `values()`

```zuri,ignore
values() -> list
```

Returns a list containing the value of all keys in the dictionary.<br>

For example:

```zuri,ignore
%> dict.values()
[Zuri, 1]
```

**Returns** `list`

## `remove()`

```zuri,ignore
remove(key) -> any|nil
```

Removes a given key and it's corresponding value from the dictionary and
returns the value of the key.

For example:

```zuri,ignore
%> dict = {username: 'james', email: 'a@b.c', active: true}
%> dict.remove('active')
true
%> dict
{username: james, email: a@b.c}
```

**Parameters**

- `key` (`string`)

**Returns** `any|nil`

## `is_empty()`

```zuri,ignore
is_empty() -> boolean
```

Returns `true` if the dictionary is empty, otherwise returns
`false`.<br>

For example:

```zuri,ignore
%> dict.is_empty()
false
%> {}.is_empty()
true
```

**Returns** `boolean`

## `find_key()`

```zuri,ignore
find_key(value) -> string|nil
```

Returns the key whose value is equal to _x_ in the dictionary or `nil`
if no key has the value _x_.<br>

For example:

```zuri,ignore
%> dict.find_key('james')
'username'
%> dict.find_key('camel')
```

**Parameters**

- `value` (`any`)

**Returns** `string|nil`

## `to_list()`

```zuri,ignore
to_list() -> list
```

Returns a list that contains a list of key and a list of values from the
dictionary. <br>

For example:

```zuri,ignore
%> var dict = {username: 'james', email: 'a@b.c'}
%> dict.to_list()
[[username, email], [james, a@b.c]]
```

**Returns** `list`

## `each()`

```zuri,ignore
each(callback: function) -> void
```

Iterates over each key-value pair in the dictionary, calling the
provided callback function with the value and key as arguments.

Example:

```zuri,ignore
var myDict = {a: 1, b: 2, c: 3}
myDict.each(@(value, key) {
  echo '${key}: ${value}'
})

# Output:
# a: 1
# b: 2
# c: 3
```

**Parameters**

- `callback` (`function`) — The function to call for each key-value
  pair.

**Returns** `void`

**Raises** `Error` If the callback is not a function.

## `filter()`

```zuri,ignore
filter(callback: function) -> dict
```

Creates a new dictionary containing only the key-value pairs for which
the provided callback function returns true. The callback function is
called with the value and key as arguments.

Example:

```zuri,ignore
var myDict = {a: 1, b: 2, c: 3}
var filteredDict = myDict.filter(@(value, key) {
  return value > 1
})
echo filteredDict

# Output: {'b': 2, 'c': 3}
```

**Parameters**

- `callback` (`function`) — The function to test each key-value pair. It
  should return true to keep the pair, or false to exclude it.

**Returns** `dict`

**Raises** `Error` If the callback is not a function.

## `some()`

```zuri,ignore
some(callback: function) -> boolean
```

Tests whether at least one key-value pair in the dictionary passes the
test implemented by the provided callback function. The callback
function is called with the value and key as arguments. The method
returns true if the callback returns true for any key-value pair,
otherwise it returns false.

Example:

```zuri,ignore
var myDict = {a: 1, b: 2, c: 3}
var hasGreaterThanTwo = myDict.some(@(value, key) {
  return value > 2
})
echo hasGreaterThanTwo

# Output: true
```

**Parameters**

- `callback` (`function`) — The function to test each key-value pair. It
  should return true to indicate a passing pair, or false to indicate a
  failing pair.

**Returns** `boolean`

**Raises** `Error` If the callback is not a function.

## `every()`

```zuri,ignore
every(callback: function) -> boolean
```

Tests whether all key-value pairs in the dictionary pass the test
implemented by the provided callback function. The callback function is
called with the value and key as arguments. The method returns true if
the callback returns true for every key-value pair, otherwise it returns
false.

Example:

```zuri,ignore
var myDict = {a: 1, b: 2, c: 3}
var allGreaterThanZero = myDict.every(@(value, key) {
  return value > 0
})
echo allGreaterThanZero

# Output: true
```

**Parameters**

- `callback` (`function`) — The function to test each key-value pair. It
  should return true to indicate a passing pair, or false to indicate a
  failing pair.

**Returns** `boolean`

**Raises** `Error` If the callback is not a function.

## `reduce()`

```zuri,ignore
reduce(callback: function, initial) -> any
```

Reduces the dictionary to a single value by iteratively combining each
key-value pair using the provided callback function. The callback
function is called with the accumulator, value, key, and the dictionary
itself as arguments. The method returns the final accumulated value
after processing all key-value pairs in the dictionary.

Example:

```zuri,ignore
var myDict = {a: 1, b: 2, c: 3}
var sum = myDict.reduce(@(accumulator, value, key) {
  return accumulator + value
}, 0)
echo sum

# Output: 6
```

**Parameters**

- `callback` (`function`) — The function to execute on each key-value
  pair in the dictionary. It should return the updated accumulator value
  after processing the pair.
- `initial` (`any`) — The initial value to use as the first argument to
  the first call of the callback function.

**Returns** `any`

**Raises** `Error` If the callback is not a function.

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the dictionary.

```zuri,ignore
%> {a: 1, b: 2}.to_string()
'{a: 1, b: 2}'
```

**Returns** `string`
