# Common Programming Concepts

Every language gives you a way to name values, a set of types for those
values, operators for combining them, and statements that control what runs
next. This chapter covers how Zuri handles each of these.

If Zuri is your first language, read the sections in order, since each one
only relies on the ones before it. If you've programmed before, pay close
attention to [Operators](ch03-03-operators.md) and [Control
Flow](ch03-05-control-flow.md). That's where syntax you already know behaves
differently in Zuri.

## Reserved Words

First, the words you can't use as names. Zuri reserves 31 keywords:

```text
and       as        assert    break     catch     class     const
continue  def       default   do        echo      else      false
for       if        import    in        iter      nil       or
parent    raise     return    self      static    true      using
var       when      while
```

They're all lowercase, and each one is explained somewhere in this book.
[Appendix A](appendix-01-keywords.md) lists them all, with a one-line
summary and a link for each.

A few words are *not* on the list, and it helps to know that now:

- **`print` isn't a keyword.** It's an ordinary built-in function, so you
  could even shadow it with a variable of your own. `echo`, on the other
  hand, *is* a keyword.
- **There are no `function`, `int`, `string` or `bool` keywords.** Type
  names are ordinary identifiers, which is why you can use them in
  annotations without them being reserved.
- **There's no `try`, `finally`, `switch`, `case`, `new`, `this`, `public`
  or `private`.** Zuri uses `catch`, `using`, `when`, a direct call to the
  class, `self`, and a leading underscore instead.
