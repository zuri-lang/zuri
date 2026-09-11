# Boolean Methods

Every method on the built-in `bool` type, with its signature, what it
returns, and the cases where it does something other than the obvious
thing.

| Method | Returns | Summary |
| --- | --- | --- |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the boolean. |

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the boolean.

```zuri,ignore
%> true.to_string()
'true'
%> false.to_string()
'false'
```

**Returns** `string`
