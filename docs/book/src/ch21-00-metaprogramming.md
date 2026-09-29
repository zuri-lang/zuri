# Metaprogramming and Reflection

Zuri's own compiler is available to Zuri programs. The `zuri` module hands
you the lexer, the parser and the bytecode compiler as ordinary functions,
alongside a reflection API over live functions, classes, modules and
instances.

That is an unusual amount of the language to expose, so it is worth being
clear about what it is for. This is the machinery behind documentation
generators, linters, plugin loaders, serialisers, debuggers and editor
tooling — programs whose subject is other programs. It is not machinery for
ordinary code, and the chapter ends with the one thing it deliberately
cannot do.

## Two Halves That Cannot Answer for Each Other

The module divides cleanly, and confusing the halves is the most common
mistake:

| | Reflection | The compiler API |
| --- | --- | --- |
| Subject | objects that exist **right now** | **source text** |
| Entry points | `zuri.reflect.*` | `tokenize()`, `parse()`, `compile()` |
| Can tell you | a function's arity, a class's fields, an instance's values | a doc block, a comment, a line number, what the compiler emitted |
| Cannot tell you | anything about the source it came from | anything about a running value |

A function's **arity** is a runtime fact: the function object carries it,
and reflection reads it. That same function's **doc block** is a source
fact: it exists only in the file, and only the parser can find it. There is
no call that crosses the gap, which is why a documentation generator parses
files rather than importing them.

## One Shape for Everything

`tokenize()`, `parse()` and `compile()` all return the same *shape* of
result: a flat list of small, uniformly tagged records.

| Function | Returns | Tag | Payload |
| --- | --- | --- | --- |
| `tokenize(source)` | `Token` list | `kind` | `value`, `text` |
| `parse(source)` | `Node` list | `kind` | `fields` |
| `compile(source)` | `Instr` list | `op` | `fields` |

There is one `Token` class, one `Node` class and one `Instr` class — not
one class per token kind, grammar rule or opcode. The grammar has around
fifty shapes and the instruction set around sixty opcodes; a class for each
would be a hundred-odd near-identical classes to keep in step with the
compiler forever.

The cost of that choice is real: nothing tells you which fields a given
`kind` carries. You cannot autocomplete your way to a `Binary` node's
`left`, `op` and `right`. So the module leans on a different habit — **run
it and look**:

```zuri
import zuri

echo zuri.parse('var total = a + 1')[0].dump()
```

```console
Stmt@1:5
  statement:
    Var@1:5
      name: 'total'
      value:
        Binary@1
          left:
            Identifier@1:13
              name: 'a'
          op: 'Plus'
          right:
            Integer
              value: 1
      type_hint: nil
      is_constant: false
```

Everything about that node is in front of you: it is a `Stmt` wrapping a
`Var`, the initialiser is a `Binary` whose `op` is the string `'Plus'`, and
the literal `1` is an `Integer` node rather than a generic literal. Nothing
had to be looked up.

`dump()` is the fastest way to learn any node's shape, and it is the first
thing to reach for whenever you are unsure. `zuri.dump_file(path)` does the
same for a whole file.

## Runtime Reflection

Reflection answers questions about a value you are holding. Every entry
point is under `zuri.reflect`.

### What Kind of Thing Is This?

```zuri
import zuri

def add(a, b) {
  return a + b
}

class Marker {}

echo zuri.reflect.kind(add)
echo zuri.reflect.kind(Marker)
echo zuri.reflect.kind(Marker())
echo zuri.reflect.kind(42)
echo zuri.reflect.kind(print)
```

```console
function
class
instance
number
function
```

`kind()` is `typeof()`'s more literal cousin. Note the last line: a
built-in native function reports `'function'`, as does a closure and a
bound method, because from the language's point of view all three are
interchangeably callable.

### Functions

```zuri
import zuri

def add(a, b) {
  return a + b
}

var info = zuri.reflect.function_info(add)

echo info.name
echo info.arity
echo info.variadic
echo info.is_method
echo info.owning_class_name
```

```console
add
2
false
false
nil
```

`function_info()` returns `{ name, arity, variadic, is_method,
owning_class_name, source_path }`. The last one is the only field that
reaches back toward the source, and it is a *path*, not content — to read
the function's doc block you still have to parse that file.

### Classes

```zuri
import zuri

class Shape {
  static var sides = 0

  @new(name) {
    self.name = name
  }

  area() {
    return 0
  }

  _secret() {
    return 'hidden'
  }

  @to_json() {
    return { name: self.name }
  }
}

var info = zuri.reflect.class_info(Shape)

echo info.name
echo info.superclass_name
echo info.fields
echo info.statics
echo info.methods.keys()
```

```console
Shape
nil
[name]
[sides]
[@new, area, @to_json, _secret]
```

Four things in that output are worth pausing on.

**`fields` lists `name`**, which was never declared with `var` — it was
created by `self.name = name` inside `@new`. Reflection reports the class's
real shape, not just what the `var` lines said.

**`statics` is separate from `fields`**, because a static belongs to the
class and a field belongs to each instance.

**Decorated methods appear under their `@` names.** `@new` and `@to_json`
are in `methods` alongside ordinary ones.

**`_secret` is listed.** Reflection sees private members; more on that
below.

Each entry in `methods` is a full `function_info`, so
`info.methods.area.arity` works without another call.

`superclass_name` is a **string or `nil`**, not a class:

```zuri
import zuri

class Base {}
class Derived < Base {}

echo zuri.reflect.class_info(Derived).superclass_name
echo zuri.reflect.class_info(Base).superclass_name
```

```console
Base
nil
```

### Instances

```zuri
import zuri

class Point {

  @new(x, y) {
    self.x = x
    self.y = y
  }

  distance() {
    return (self.x ** 2 + self.y ** 2).sqrt()
  }
}

var p = Point(3, 4)

echo zuri.reflect.get_props(p)
echo zuri.reflect.has_prop(p, 'x')
echo zuri.reflect.get_prop(p, 'x')

zuri.reflect.set_prop(p, 'x', 10)
echo p.x
```

```console
[x, y]
true
3
10
```

`get_props()` lists the field names an instance actually has.
`get_prop()`, `set_prop()`, `has_prop()` and `del_prop()` read and write
them by computed name — the reflective equivalents of the `getprop` family
of built-ins from [Chapter 6](ch06-04-encapsulation.md).

They cannot create a field. A class is sealed, so `set_prop()` on a name
the class never declared returns `false` and changes nothing.

### Methods by Name

Three calls turn a method name into something callable:

```zuri
import zuri

class Greeter {

  @new(name) {
    self.name = name
  }

  greet() {
    return 'hello ${self.name}'
  }

  @to_json() {
    return { name: self.name }
  }
}

var g = Greeter('ada')

echo zuri.reflect.has_method(g, 'greet')
echo zuri.reflect.has_decorator(g, 'to_json')
echo typeof(zuri.reflect.get_method(g, 'greet'))
echo zuri.reflect.bind_method(g, 'greet')()
```

```console
true
true
function
hello ada
```

The distinction between the last two matters. **`get_method()` returns the
unbound function**; calling it needs the instance supplied yourself.
**`bind_method()` returns it already attached to the instance**, so it can
be stored, passed around and called with nothing extra — which is what
makes a dispatch table of methods possible:

```zuri
import zuri

class Counter {

  @new() {
    self.n = 0
  }

  up() {
    self.n++
    return self.n
  }

  down() {
    self.n--
    return self.n
  }
}

var counter = Counter()

var actions = {
  up: zuri.reflect.bind_method(counter, 'up'),
  down: zuri.reflect.bind_method(counter, 'down'),
}

echo actions['up']()
echo actions['up']()
echo actions['down']()
echo counter.n
```

```console
1
2
1
1
```

`has_decorator()` and `get_decorator()` do the same for `@`-prefixed
methods, taking the name **without** the `@`.

### Reflection Sees Private Members

Everything above ignores the leading-underscore rule:

```zuri
import zuri

class Vault {
  var _combination = '1234'

  @new() {}
}

echo zuri.reflect.get_props(Vault())
echo zuri.reflect.get_prop(Vault(), '_combination')
```

```console
[_combination]
1234
```

This is deliberate, and it is the same decision `getprop()` makes. A
serialiser has to see every field or it writes an incomplete record; a
debugger has to see every field or it shows a lie. Hiding them would defeat
the only purpose these functions have.

The rule to take from it: **reflection is infrastructure, not a way around
encapsulation**. Code that reaches for `get_prop()` to read a private field
it could not otherwise read has not found a loophole; it has written
something the next reader will not expect.

### Modules

```zuri
import zuri
import math

var info = zuri.reflect.module_info(math)

echo info.name
echo info.members.contains('PI')
```

```console
math
true
```

`module_info()` reports a module's name, its path and the members it
exports. That is enough for a plugin loader: import a directory of modules,
ask each whether it has a known function, and call the ones that do.

### The Collector

The garbage collector runs on its own as a program allocates. `gc()` runs
a full collection on the spot:

```zuri
import zuri

var scratch = [1, 2, 3]
scratch = nil

zuri.reflect.gc()
echo 'collected'
```

```console
collected
```

Every object nothing reaches is freed before `gc()` returns, and anything
that releases a resource when collected releases it then: an `ffi` pointer
taken over with `own()` runs its destructor. No program needs `gc()` to
stay correct. It is for the moments timing matters, such as releasing
native resources at a known point, or a test checking what collection
does. A full collection visits every live object, so calling it in a loop
is slow.

## Tokens

`tokenize()` is the first stage: text in, a flat list of lexical tokens
out.

```zuri
import zuri

for token in zuri.tokenize('var x = 1 # note') {
  echo '${token.kind} at ${token.line}:${token.column} -> ${token.text}'
}
```

```console
Var at 1:1 -> var
Identifier at 1:5 -> x
Equal at 1:7 -> =
Integer at 1:9 -> 1
Comment at 1:11 -> # note
Eof at 1:17 -> 
```

**Nothing is filtered.** Comments, doc blocks and newlines are all real
tokens, and the list always ends with one `Eof`. The parser's grammar skips
trivia when it runs; `tokenize()` reports everything the lexer saw, which
is exactly what a formatter or a syntax highlighter needs.

Each `Token` carries:

| Field | What it is |
| --- | --- |
| `kind` | the lexer's own variant name — `'Identifier'`, `'Plus'`, `'Comment'` |
| `line`, `column` | 1-indexed position; `column` counts characters, not bytes |
| `start`, `end` | character offsets spanning the token's exact text |
| `text` | the exact source text, delimiters included |
| `value` | the payload, where there is one — a literal's value, an identifier's name, a comment's content |

`start` and `end` are what make edits possible: they let you recover or
replace a token's original text without re-lexing, which is how a rename
tool or an automatic formatter works.

`is_trivia()` is true for comments and doc blocks, so filtering them is one
call:

```zuri
import zuri

var tokens = zuri.tokenize('var x = 1 # note')

echo tokens.filter(@(t) => t.is_trivia()).map(@(t) => t.kind)
echo tokens.filter(@(t) => !t.is_trivia()).length()
```

```console
[Comment]
5
```

### Lexing Never Raises

A malformed input does not throw. It produces a token of kind `'Error'`
carrying what went wrong:

```zuri
import zuri

var tokens = zuri.tokenize("var s = 'unterminated")

echo tokens.map(@(t) => t.kind)
echo tokens.find(@(t) => t.kind == 'Error') != nil
```

```console
[Var, Identifier, Equal, Error, Eof]
true
```

`tokenize()` is therefore **total over any input at all**, which is what
makes it safe to point at a file you did not write, or at a half-typed
buffer in an editor. Neither `parse()` nor `compile()` has that property —
both raise on bad input, because neither can produce a meaningful result
from it.

## The Syntax Tree

`parse()` is the second stage: tokens become a tree.

```zuri
import zuri

var tree = zuri.parse('def double(n) { return n * 2 }')

echo tree[0].kind
echo tree[0].fields.keys()
echo tree[0].fields.name
echo tree[0].line
```

```console
Function
[name, parameters, body, is_variadic]
double
1
```

A `Node` has a `kind`, a `line`, a `column` and a `fields` dictionary. The
fields hold more nodes, plain lists, plain dictionaries or scalars — never
anything else — so walking the tree is uniform no matter which node you are
looking at.

### Walking It

`walk_nodes(tree, visitor)` visits every node, depth first:

```zuri
import zuri

var tree = zuri.parse('def double(n) { return n * 2 }')
var kinds = []

zuri.walk_nodes(tree, @(node) {
  kinds.append(node.kind)
})

echo kinds
```

```console
[Function, Argument, TypeHint, Any, Block, Return, Binary, Identifier, Integer]
```

That output repays a second look, because it shows how much the parser
makes explicit. The single unannotated parameter `n` still produced an
`Argument` wrapping a `TypeHint` wrapping an `Any` — the *absence* of an
annotation is represented in the tree, not omitted from it. A tool walking
this never has to special-case "no type was written".

The visitor may also be a **dictionary from kind to handler**, in which
case only matching nodes are called:

```zuri
import zuri

var tree = zuri.parse('def a() {}
def b() {}
var c = 1')
var names = []

zuri.walk_nodes(tree, {
  Function: @(node) {
    names.append(node.fields.name)
  },
})

echo names
```

```console
[a, b]
```

`find_nodes(tree, kind)` is the shortcut when you want one kind and nothing
else:

```zuri
import zuri

var tree = zuri.parse('def double(n) { return n * 2 }')

echo zuri.find_nodes(tree, 'Return').length()
echo zuri.find_nodes(tree, 'Binary')[0].fields.op
```

```console
1
Multiply
```

### Comments Survive

This is the property that makes documentation tooling possible. Comments
and doc blocks are kept in the tree, in their original position, as their
own nodes:

```zuri
import zuri

var source = "# a note
def add(a, b) {
  return a + b
}
"

echo zuri.parse(source).map(@(n) => n.kind)
```

```console
[Comment, Function]
```

A doc block sits as a sibling immediately before whatever it documents, so
pairing them is one pass with one variable of state — which is exactly what
the worked example below does.

Most parsers throw comments away. Keeping them is what separates a parser
you can build a formatter or a documentation generator on from one you can
only build an interpreter on.

## Bytecode

`compile()` is the third stage: the tree becomes VM instructions.

```zuri
import zuri

for instr in zuri.compile('var a = 1 + 2') {
  echo '${instr.op} from line ${instr.line}'
}
```

```console
LoadConst from line 0
AddImm from line 1
SetGlobal from line 1
LoadNil from line 1
Return from line 1
```

An `Instr` has an `op`, a `line` and a `fields` dictionary — the same shape
a `Node` has — and `walk_instrs()` and `find_instrs()` mirror the AST
walkers exactly.

Notice `AddImm`. The compiler folded the constant `2` into the add
instruction rather than loading it separately. That is the sort of question
only the bytecode can answer, and reading it is the most direct way to find
out what the compiler actually made of something:

```zuri
import zuri

echo zuri.compile('var a = 2 * 3 ** 2').map(@(i) => i.op)
```

```console
[LoadConst, LoadConst, LoadConst, Pow, Mul, SetGlobal, LoadNil, Return]
```

The power comes **before** the multiply, which is `**` binding tighter than
`*`. One line of bytecode settles an argument that reading the expression
does not.

### Resolved Operands

Several opcodes carry only a raw index into the compiler's constant pool,
which on its own tells you nothing. Rather than make every caller fetch and
correlate a constants table, each such field arrives with its resolved
value alongside the index:

```zuri
import zuri

var load = zuri.find_instrs(zuri.compile('var a = 42'), 'LoadConst')[0]

echo load.fields.keys()
echo load.fields.value
```

```console
[dst, const_idx, value]
42
```

`const_idx` is the raw index, `value` is what it points at, and `dst` is
the register the result lands in. The same
pairing appears on `GetGlobal`, `GetField`, `Invoke` and the rest.

A `Closure` instruction's resolved value is a nested function prototype
(`{ name, arity, variadic, instructions }`) whose own `instructions` are
wrapped the same way, recursively — so compiling a file with functions in
it exposes their bodies too, not only the code around them.

### Line Numbers Are Statement-Grained

Every instruction carries the source line it came from, but **never a
column**. The compiler's own tracking is line-only. A `Token` and a `Node`
both have real column information from the lexer; an `Instr` does not.

## Reading a File

Each of the three has a `_file` counterpart that reads the path first:

| Source string | File |
| --- | --- |
| `tokenize(source)` | `tokenize_file(path)` |
| `parse(source)` | `parse_file(path)` |
| `compile(source)` | `compile_file(path)` |

There is a fourth with no string equivalent: **`dump_file(path)`** reads,
parses and returns the `dump()` of every top-level node. It is usually the
fastest possible answer to "what is actually in this file".

## A Worked Example: A Documentation Extractor

Here is the pattern the book's own reference appendices are built on. It
takes source text and returns every documented function in it, pairing each
declaration with the doc block above it.

The key fact is that `zuri.parse()` keeps comments in the tree. A doc block
comes back as its own `DocBlock` node, sitting as a sibling immediately
before whatever it documents, so pairing them is a single pass with one
variable of state:

```zuri
import zuri

def documented_functions(text) {
  var found = []
  var pending = nil

  for node in zuri.parse(text) {
    if node.kind == 'DocBlock' {
      pending = node.fields.text
      continue
    }

    if node.kind == 'Function' and pending != nil {
      var params = node.fields.parameters.map(@(p) => p.fields.name)

      found.append({ name: node.fields.name, params, doc: pending })
    }

    # Anything else between a block and a declaration breaks the pairing.
    pending = nil
  }

  return found
}

var source = "/**\n * Adds two numbers.\n */\ndef add(a: number, b: number) {\n  return a + b\n}\n\ndef undocumented(x) {\n  return x\n}\n"

for fn in documented_functions(source) {
  echo '${fn.name}(${', '.join(fn.params)})'
  echo '  ' + fn.doc.split('\n')[1].trim().replace('* ', '', false)
}
```

```console
add(a, b)
  Adds two numbers.
```

`undocumented` is absent from the output, because nothing set `pending`
before it.

Three things make this approach worth preferring over scanning the text
yourself.

**The parser knows what a declaration is.** A function named `def_handler`,
a `def` inside a string literal, a doc block inside a block comment — all of
them fool a text scan and none of them fool the parser.

**Parameter names and types come from the real nodes.** A parameter's
`type_hint` node carries its types and whether it is nullable, so a
generated signature matches what the runtime will actually enforce rather
than what the source happened to look like.

**It cannot drift.** When the grammar gains something, the parser gains it
too, and a tool built this way keeps working.

What the parser does *not* do is interpret the doc block's contents. The
`@param` and `@returns` conventions are a convention, not grammar, so the
text inside a `DocBlock` node is yours to parse. That is the one piece you
write yourself — and the place to be careful, since a tag's text may wrap
onto the following line.

## A Second Example: A Linter

The other half of the module's use is checking rather than generating.
Here is a rule of the kind a team accumulates: flag every function whose
parameter list is longer than some limit.

```zuri
import zuri

def long_signatures(source, limit) {
  var offenders = []

  zuri.walk_nodes(zuri.parse(source), {
    Function: @(node) {
      var count = node.fields.parameters.length()

      if count > limit {
        offenders.append({ name: node.fields.name, line: node.line, count })
      }
    },
  })

  return offenders
}

var source = "def small(a, b) {}\n" +
  "def large(a, b, c, d, e) {}\n" +
  "def also_large(a, b, c, d) {}\n"

for problem in long_signatures(source, 3) {
  echo 'line ${problem.line}: ${problem.name}() takes ${problem.count}'
}
```

```console
line 2: large() takes 5
line 3: also_large() takes 4
```

Three properties make this worth doing with the parser rather than with a
regular expression over the text.

**It cannot be fooled by text that looks like code.** A function named
`def_handler`, the word `def` inside a string literal, a commented-out
declaration — none of them are `Function` nodes, so none of them are
counted.

**It reports the real line.** `node.line` comes from the lexer, so the
message points at the declaration whatever the formatting around it.

**It keeps working.** When the grammar gains something, the parser gains it
too, and a rule written this way does not quietly stop matching.

The dictionary form of the visitor is doing real work here: only `Function`
nodes reach the handler, so there is no `if node.kind == ...` and no chance
of matching a kind you did not mean to.

## Serialising Without a `@to_json`

Reflection covers the case where you would otherwise write the same method
on every class:

```zuri
import zuri
import json

class User {

  @new(name, age) {
    self.name = name
    self.age = age
  }
}

class Product {

  @new(title, price) {
    self.title = title
    self.price = price
  }
}

def to_record(instance) {
  var record = {}

  for field in zuri.reflect.get_props(instance) {
    record[field] = zuri.reflect.get_prop(instance, field)
  }

  return record
}

echo json.encode(to_record(User('ada', 36)))
echo json.encode(to_record(Product('desk', 120)))
```

```console
{"name":"ada","age":36}
{"title":"desk","price":120}
```

One function, every class, no per-class method to keep in step with the
fields. Note that this reads private fields too, which is right for a
debugging dump and wrong for an API response — for the latter, filter on
the leading underscore, or write a real `@to_json` and let the class decide
what it exposes.

## What Else This Is Good For

**Plugin systems.** Load modules from a directory, ask each whether it has
the function your host expects, and call the ones that do. `module_info()`
answers the question without importing blindly and hoping.

**Editor tooling.** `tokenize()` never raises, so it can run against a
half-typed buffer. Every token carries `start` and `end`, so a rename or a
reformat can rewrite exact spans.

**Understanding the compiler.** `zuri.compile()` on a snippet is faster
than reading the compiler's source, and it can never go out of date with
the compiler you are actually running.

**Answering questions about this book.** Appendices D and E are generated
from `libs/` with exactly the techniques above, and the audit that checks
those stubs is written the same way.

## What This Is Not Good For

There is no `eval()`. You can compile source to bytecode and inspect it;
you cannot execute a string as code.

Zuri leaves it out for security. `eval()` erases the line between data and
code, and every program that has one eventually runs a string it did not
mean to: a form field, a query parameter, a config value, a webhook
payload. The moment any of those reaches an `eval()`, whoever supplied it
is running code with your program's full permissions. It reads your files,
opens your sockets and sends your secrets anywhere it likes. This is the
single most damaging vulnerability class in dynamic languages, and it keeps
happening because the dangerous call looks harmless in review: one
function, three characters of input, and nothing in the language warns you.

So Zuri does not provide one, and every job `eval()` is usually reached for
has a better answer here:

| Instead of | Use |
| --- | --- |
| evaluating a user-supplied expression | a dictionary of handlers keyed by the input |
| calling a method whose name you computed | `zuri.reflect.bind_method()` |
| varying behaviour at runtime | pass a function in |
| turning text into data | `json.decode()` |

Each of those does the job with a fixed, auditable set of things that can
happen. That is the difference between a program that handles input and a
program that obeys it.

Classes are sealed, so reflection cannot add a method to one at runtime.
Patterns from other languages that rely on monkey-patching do not
translate, and the alternative is the one the language wants: express the
variation as a subclass, or as a function you pass in.
