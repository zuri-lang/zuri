# Making a Class Iterable

Any class can be walked with `for ... in`. It takes two decorated methods
and no other ceremony: no interface to declare, no iterator object to
return, and no state kept between passes.

This section covers the protocol, the two shapes it usually takes, what
happens when a piece is missing, and the rules that keep a custom iterator
well behaved.

## The Protocol

`for value in thing` is not magic. Zuri evaluates `thing` **once**, then
repeats three steps:

1. `key = thing.@key(key)`, starting from `nil`
2. stop if `key` is `nil`
3. `value = thing.@value(key)`, and run the body

So the two methods answer two separate questions:

- **`@key(previous)`** — "given the last position, what is the next one?"
  It receives `nil` on the first call and must return `nil` when there is
  nothing left.
- **`@value(key)`** — "what is stored at this position?"

The loop never stores a cursor of its own. The key *is* the cursor, which
is why `@key` gets the previous one back each time.

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

Trace it once and the protocol stops being abstract. `@key(nil)` returned
`3`, so the first value is `@value(3)`, which is `30`. `@key(3)` returned
`2`, then `@key(2)` returned `1`, then `@key(1)` returned `nil` and the
loop ended.

Note that the key and the value are genuinely different things here: the
key counts down from three, the value is ten times it. Two variables in the
loop give you both:

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

The more common case is a class holding a list, where the key is an index.
The shape is always the same three checks:

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

Each of the three checks in `@key` earns its place:

**The empty check comes first.** Without it, `@key(nil)` would return `0`
for an empty stack and `@value(0)` would index past the end.

**`previous == nil` starts the walk**, and `0` is the first index.

**`previous >= length() - 1` ends it.** Using `>=` rather than `==` means a
collection that shrank mid-loop still terminates.

An empty collection iterates zero times rather than failing:

```zuri
for value in Stack() {
  echo 'never printed'
}

echo 'done'
```

```console
done
```

## What You Get for Free

Defining both methods makes `is_iterable()` answer `true`, and makes the
class usable everywhere `for` is:

```zuri
echo is_iterable(Stack().push('a'))
echo is_iterable(Countdown(1))
```

```console
true
true
```

## Both Are Required

`for` calls `@key` and then `@value`. Defining only one produces an error
naming the one that is missing:

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

A class with neither fails on `@key` instead:

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

## Rules Worth Knowing

**A `nil` key ends the loop, always.** That means a collection whose keys
could legitimately *be* `nil` cannot use them as keys. Use indices and look
the real key up in `@value`.

**The instance is evaluated once.** `for x in build_thing()` calls
`build_thing()` a single time, before the first pass, so `@key` and
`@value` are always called on the same object.

**Neither method should mutate.** They are called once per pass, in a loop
you do not control, and a `@key` with a side effect is a loop whose
behaviour depends on how many times something asked for the next key.

**Iteration order is whatever `@key` says.** There is no requirement to
count upwards, or to visit everything; `Countdown` walks backwards, and a
class could just as well skip, filter or repeat.
