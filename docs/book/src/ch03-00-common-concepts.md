# Common Programming Concepts

Every language gives you a way to name a value, a set of types those values
come in, operators to combine them, and statements to decide what runs
next. This chapter is those four things in the form Zuri gives them.

If this is your first language, read the sections in order; each one uses
only what came before it. If you already program, read
[Operators](ch03-03-operators.md) and
[Control Flow](ch03-05-control-flow.md) attentively — those are where the
same syntax you already know does something different here.

## The Reserved Words

Before anything else, the words you cannot use as names. Zuri reserves 31
keywords:

```text
and       as        assert    break     catch     class     const
continue  def       default   do        echo      else      false
for       if        import    in        iter      nil       or
parent    raise     return    self      static    true      using
var       when      while
```

Every one is lowercase, and every one is explained somewhere in this book.
[Appendix A](appendix-01-keywords.md) is the index, with a one-line summary
and a pointer for each.

Three things are *not* in that list, and are worth noticing now:

- **`print` is not a keyword.** It is an ordinary built-in function, and
  you could shadow it with a variable of your own if you wanted to.
  `echo` *is* a keyword.
- **There is no `function`, `int`, `string` or `bool` keyword.** Type names
  are ordinary identifiers, which is why they can appear as annotations
  without being reserved.
- **There is no `try`, `finally`, `switch`, `case`, `new`, `this`,
  `public` or `private`.** The equivalents are `catch`, `using`, `when`,
  calling the class directly, `self`, and a leading underscore.
