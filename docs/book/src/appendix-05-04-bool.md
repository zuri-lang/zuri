# Boolean Methods

Every method of the built-in `bool` type, with its signature, what it
returns, and any behavior you might not expect.

| Method | Returns | Summary |
| --- | --- | --- |
| [`to_string()`](#to_string) | `string` | Returns the string representation of the boolean. |

## `to_string()`

```zuri,ignore
to_string() -> string
```

Returns the string representation of the boolean.

```zuri-repl
%> true.to_string()
'true'
%> false.to_string()
'false'
```

**Returns** `string`
