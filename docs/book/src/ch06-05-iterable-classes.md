# Making a Class Iterable

You can loop over an instance of any class with `for ... in`. All it takes
is two decorated methods. You don't need to declare an interface, return an
iterator object or keep any state between passes.

This section explains the protocol, the two forms it usually takes, what
happens when part of it is missing, and the rules that keep a custom
iterator well behaved.

## How It Works

There's nothing magic about `for value in thing`. Zuri evaluates `thing`
**once**, and then repeats these steps:

1. `key = thing.@key(key)`, starting with `key` set to `nil`
2. If `key` is `nil`, stop
3. `value = thing.@value(key)`, and run the body

So the two methods answer two different questions:

- **`@key(previous)`** answers "given the last position, what's the next
  one?" It receives `nil` on the first call, and it must return `nil` when
  there's nothing left.
- **`@value(key)`** answers "what's stored at this position?"

The loop doesn't keep track of its position itself. The key *is* the
position, which is why `@key` receives the previous key each time.

## A Counting Example

```zuri
class Countdown {

  @new(from) {
    self.from = from
  }

  @key(previous) {
    if previous == nil {
      return self.from
    }

    if previous <= 1 {
      return nil
    }

    return previous - 1
  }

  @value(key) {
    return key * 10
  }
}

for value in Countdown(3) {
  echo value
}
```

```console
30
20
10
```

If you trace through it once, the protocol becomes much less abstract.
`@key(nil)` returned `3`, so the first value is `@value(3)`, which is `30`.
Next, `@key(3)` returned `2`, then `@key(2)` returned `1`, and finally
`@key(1)` returned `nil`, which ended the loop.

Notice that the key and the value really are different here. The key counts
down from three, and the value is ten times the key. If you use two loop
variables, you get both:

```zuri
for key, value in Countdown(3) {
  echo '${key} -> ${value}'
}
```

```console
3 -> 30
2 -> 20
1 -> 10
```

## Wrapping a Collection

More often, the class wraps a list and the key is an index. The structure is
always the same, with three checks:

```zuri
class Stack {

  @new() {
    self.items = []
  }

  push(value) {
    self.items.append(value)
    return self
  }

  @key(previous) {
    if self.items.is_empty() {
      return nil
    }

    if previous == nil {
      return 0
    }

    if previous >= self.items.length() - 1 {
      return nil
    }

    return previous + 1
  }

  @value(key) {
    return self.items[key]
  }
}

var stack = Stack().push('a').push('b').push('c')

for value in stack {
  echo value
}

for index, value in stack {
  echo '${index}: ${value}'
}
```

```console
a
b
c
0: a
1: b
2: c
```

Each of the three checks in `@key` is needed:

**The check for an empty collection comes first.** Without it, `@key(nil)`
would return `0` for an empty stack, and `@value(0)` would read past the
end.

**`previous == nil` starts the iteration**, with `0` as the first index.

**`previous >= length() - 1` ends it.** Using `>=` instead of `==` means the
loop still ends if the collection gets shorter partway through.

An empty collection doesn't fail. The loop just runs zero times:

```zuri
for value in Stack() {
  echo 'never printed'
}

echo 'done'
```

```console
done
```

## What You Get

Once you've defined both methods, `is_iterable()` returns `true` for your
class, and you can use it anywhere `for` works:

```zuri
echo is_iterable(Stack().push('a'))
echo is_iterable(Countdown(1))
```

```console
true
true
```

## Both Methods Are Required

`for` calls `@key` and then `@value`. If you only define one of them, you
get an error that names the missing one:

```zuri
class OnlyKey {

  @key(previous) {
    return previous == nil ? 0 : nil
  }
}

catch {
  for value in OnlyKey() {
    echo value
  }
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
PropertyError: undefined property '@value' on instance of 'OnlyKey'
```

A class that defines neither fails on `@key`:

```zuri
class Plain {}

catch {
  for value in Plain() {
    echo value
  }
} as e {
  echo '${e.type}: ${e.message}'
}
```

```console
PropertyError: undefined property '@key' on instance of 'Plain'
```

## Rules to Keep in Mind

**A `nil` key always ends the loop.** So if `nil` could be a valid key in
your collection, you can't use those keys directly. Use indexes instead, and
look up the real key in `@value`.

**The instance is evaluated only once.** `for x in build_thing()` calls
`build_thing()` a single time, before the first pass, so `@key` and `@value`
are always called on the same object.

**Neither method should change anything.** They're called on every pass, in
a loop you don't control, and if `@key` has a side effect, the loop's
behavior depends on how many times something asked for the next key.

**The iteration order is whatever `@key` returns.** There's no rule that you
have to count upward or visit every element. `Countdown` goes backward, and
a class could just as easily skip, filter or repeat elements.
