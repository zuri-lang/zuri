# Metaprogramming and Reflection

Zuri programs have access to Zuri's own compiler. The `zuri` module provides
the lexer, the parser and the bytecode compiler as ordinary functions, along
with a reflection API for working with live functions, classes, modules and
instances.

That's an unusual amount of the language to expose, so it's worth being
clear about what it's for. These are the tools behind documentation
generators, linters, plugin loaders, serializers, debuggers and editor
tooling, which are all programs that work with other programs. They aren't
meant for everyday code, and the chapter ends with the one thing they
intentionally can't do.

## Reflection and the Compiler API

The module is divided into two clear halves, and confusing them is the most
common mistake:

| | Reflection | The compiler API |
| --- | --- | --- |
| Works with | Objects that exist **right now** | **Source code** |
| Entry points | `zuri.reflect.*` | `tokenize()`, `parse()`, `compile()` |
| Can tell you | A function's arity, a class's fields, an instance's values | A doc block, a comment, a line number, what the compiler produced |
| Can't tell you | Anything about the source code it came from | Anything about a running value |

A function's **arity** is a run-time fact. The function object stores it,
and reflection reads it. The same function's **doc block** is a fact about
the source code. It only exists in the file, and only the parser can find
it. No function bridges the two, which is why a documentation generator
parses files instead of importing them.

## A Common Structure

`tokenize()`, `parse()` and `compile()` all return results with the same
*structure*: a flat list of small records, each tagged in the same way.

| Function | Returns | Tag | Data |
| --- | --- | --- | --- |
| `tokenize(source)` | A list of `Token` | `kind` | `value`, `text` |
| `parse(source)` | A list of `Node` | `kind` | `fields` |
| `compile(source)` | A list of `Instr` | `op` | `fields` |

There's a single `Token` class, a single `Node` class and a single `Instr`
class, rather than one class for each kind of token, grammar rule or opcode.
The grammar has about fifty node types and the instruction set has about
sixty opcodes, so a class for each would mean more than a hundred nearly
identical classes that would have to be kept in sync with the compiler
forever.

The downside of this choice is real: nothing tells you which fields a
particular `kind` has. Your editor can't autocomplete a `Binary` node's
`left`, `op` and `right`. Instead, the module relies on a different habit,
which is to **run it and look**:

```zuri
import zuri

echo zuri.parse('var total = a + 1')[0].dump()
```

```console
Stmt@1:1
  statement:
    Var@1:1
      name: 'total'
      name_span: 1:5-1:10
      value:
        Binary@1:13
          left:
            Identifier@1:13
              name: 'a'
          op: 'Plus'
          right:
            Integer@1:17
              value: 1
      type_hint: nil
      is_constant: false
```

Everything about that node is right there. It's a `Stmt` that wraps a `Var`,
the initializer is a `Binary` node whose `op` is the string `'Plus'`, and
the literal `1` is an `Integer` node rather than a generic literal. Each
header shows where the node starts, and `name_span` shows exactly where the
name `total` is. You didn't have to look anything up.

`dump()` is the quickest way to learn the structure of any node, and it's
the first thing to try whenever you're unsure. `zuri.dump_file(path)` does
the same for a whole file.

## Runtime Reflection

Reflection answers questions about a value you have. Every reflection
function is in `zuri.reflect`.

### What Kind of Value Is This?

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

`kind()` is a more literal version of `typeof()`. Note the last line: a
built-in native function reports `'function'`, as do closures and bound
methods, because as far as the language is concerned, all three can be
called in exactly the same way.

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

`function_info()` returns
`{ name, arity, variadic, is_method, owning_class_name, source_path }`. The
last field is the only one that refers back to the source code, and it's a
*path*, not the content. To read the function's doc block, you still have to
parse that file.

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

Four things in that output are worth a closer look.

**`fields` includes `name`**, which was never declared with `var`. It was
created by `self.name = name` inside `@new`. Reflection reports the class's
actual structure, not just what the `var` lines said.

**`statics` is separate from `fields`**, because a static field belongs to
the class, while a field belongs to each instance.

**Decorated methods are listed under their `@` names.** `@new` and
`@to_json` appear in `methods` alongside the ordinary methods.

**`_secret` is listed.** Reflection can see private members, as explained
below.

Each entry in `methods` is a complete `function_info`, so
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

`get_props()` lists the names of the fields an instance actually has.
`get_prop()`, `set_prop()`, `has_prop()` and `del_prop()` read and write
fields by a computed name. They're the reflection equivalents of the
`getprop` family of built-in functions from [Chapter
6](ch06-04-encapsulation.md).

They can't create a field. Classes are sealed, so `set_prop()` with a name
that the class never declared returns `false` and doesn't change anything.

`class_of()` works in the other direction. Given an instance, it returns the
class it was created from, as the class itself:

```zuri
import zuri

class Point {}
class Labelled < Point {}

var p = Labelled()

echo zuri.reflect.class_of(p) == Labelled
echo zuri.reflect.class_of(p) == Point
echo zuri.reflect.class_info(zuri.reflect.class_of(p)).name
echo zuri.reflect.class_of(42)
```

```console
true
false
Labelled
nil
```

`typeof()` returns the class's name, and two classes with the same name
declared in different modules return the same name. Only the class itself
can tell them apart, and only the class can be called, passed to
`class_info()` or used as a dictionary key. That's how a library can store
something for each class. For example, `sql.model` stores each model's
definition this way.

### Looking Up Methods by Name

Three functions turn a method name into something you can call:

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

The difference between the last two is important. **`get_method()` returns
the unbound function**, so you have to pass the instance yourself when you
call it. **`bind_method()` returns the method already attached to the
instance**, so it can be stored, passed around and called without anything
extra. That's what makes a dispatch table of methods possible:

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

`has_decorator()` and `get_decorator()` do the same for methods whose names
start with `@`. Pass the name **without** the `@`.

### Reflection Can See Private Members

All of the functions above ignore the leading underscore rule:

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

This is intentional, and it's the same choice `getprop()` makes. A
serializer has to see every field, or it writes an incomplete record, and a
debugger has to see every field, or it shows misleading information. Hiding
private fields would defeat the only purpose these functions have.

The lesson here is that **reflection is for building tools, not for getting
around encapsulation**. If code uses `get_prop()` to read a private field it
couldn't otherwise read, it hasn't found a clever loophole. It has written
something the next person to read it won't expect.

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

`module_info()` returns a module's name, its path and the members it
exports.

`import` takes its path exactly as written, so it can't import a file whose
name the program only learns while it's running. `zuri.import_file()` can.
It imports the module at a given path, just as an `import` naming that file
would, and returns it:

```zuri
import os
import zuri

var plugins = os.create_temp_dir('plugins-')

file(
  os.join_paths(plugins, 'greeter.zu'),
  'w',
).write("def greet(name) {\n  return 'hello ' + name\n}\n")

for name in os.read_dir(plugins).filter(@(entry) => entry.ends_with('.zu')) {
  var plugin = zuri.import_file(os.join_paths(plugins, name))

  if zuri.reflect.module_info(plugin).members.get('greet', nil) == 'function' {
    echo plugin.greet('Ada')
  }
}
```

```console
hello Ada
```

That's how you build a plugin loader: import a directory of modules, check
whether each one has a known function, and call the ones that do. A relative
path is resolved from the working directory. The module comes from the same
cache that `import` uses, so its top level only runs once, however many
times and in whatever way it's imported, and two imports of the same file
return the same module, which are equal (`==`) to each other. `sql.migrate`
loads its migrations this way.

### The Garbage Collector

The garbage collector runs automatically as a program allocates memory.
`gc()` runs a full collection immediately:

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

Every unreachable object is freed before `gc()` returns, and anything that
releases a resource when it's collected does so at that point. For example,
an `ffi` pointer taken over with `own()` runs its destructor. The freed
memory is also returned to the operating system before `gc()` returns. No
program needs to call `gc()` to work correctly. It's for situations where
timing matters, such as releasing native resources at a known point, or a
test that checks what garbage collection does. A full collection visits
every live object, so calling it in a loop is slow.

## Tokens

`tokenize()` is the first stage. It takes source text and returns a flat
list of tokens.

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

**Nothing is filtered out.** Comments, doc blocks and newlines are all
returned as tokens, and the list always ends with a single `Eof` token. The
parser skips comments and whitespace when it runs, but `tokenize()` reports
everything the lexer saw, which is exactly what a formatter or a syntax
highlighter needs.

Each `Token` has these fields:

| Field | Description |
| --- | --- |
| `kind` | The lexer's own name for the token type, such as `'Identifier'`, `'Plus'` or `'Comment'` |
| `line`, `column` | The position, starting from 1. `column` counts characters, not bytes. |
| `start`, `end` | The character offsets of the token's exact text |
| `text` | The exact source text, including any delimiters |
| `value` | The token's data, where it has any, such as a literal's value, an identifier's name or a comment's content |

`start` and `end` make it possible to edit source code. They let you recover
or replace a token's original text without running the lexer again, which is
how a rename tool or a code formatter works.

`is_trivia()` returns true for comments and doc blocks, so you can filter
them out with a single call:

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

### The Lexer Never Raises an Error

Invalid input doesn't raise an error. Instead, it produces a token of kind
`'Error'` that describes what went wrong:

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

That means `tokenize()` **accepts any input at all**, which makes it safe to
use on a file you didn't write, or on a half-written buffer in an editor.
Neither `parse()` nor `compile()` works like that. Both raise an error for
invalid input, because neither can produce a meaningful result from it.

## The Syntax Tree

`parse()` is the second stage. It turns tokens into a tree.

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
[name, name_span, parameters, body, is_variadic]
double
1
```

A `Node` has a `kind`, a position and a `fields` dictionary. The fields
contain other nodes, plain lists, plain dictionaries or simple values and
nothing else, so you can walk through the tree in the same way, whatever
node you're looking at.

### Node Positions

A node's position covers all of the source it was parsed from. `start` and
`end` are character offsets, of the first character and of the character
just after the last one, so slicing the source with them gives you the
node's own text. `line` and `col` give the start of the node, and `end_line`
and `end_col` give its end, all counted from 1:

```zuri
import zuri

var source = 'def double(n) {\n  return n * 2\n}'
var function = zuri.parse(source)[0]
var product = function.fields.body.fields.statements[0].fields.value

echo source[product.start, product.end]
echo '${function.line}:${function.col} to ${function.end_line}:${function.end_col}'
echo source[function.fields.name_span.start, function.fields.name_span.end]
```

```console
n * 2
1:1 to 3:2
double
```

A declaration starts at its keyword, so a function starts at `def`, and its
name has its own `name_span`. That's everything a tool needs to underline a
mistake, jump to a definition or select a whole statement.

### Walking the Tree

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

That output is worth a second look, because it shows how much the parser
makes explicit. The single parameter `n`, which has no annotation, still
produced an `Argument` that wraps a `TypeHint` that wraps an `Any`. The
*absence* of an annotation is represented in the tree rather than left out,
so a tool that walks the tree never has to handle "no type was written" as a
special case.

The visitor can also be a **dictionary that maps node kinds to handlers**,
in which case only the matching nodes are passed to it:

```zuri
import zuri

var tree = zuri.parse(
  'def a() {}
def b() {}
var c = 1',
)
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

`find_nodes(tree, kind)` is a shortcut for when you only want nodes of one
kind:

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

### Comments Are Kept

This is what makes documentation tools possible. Comments and doc blocks are
kept in the tree, in their original positions, as nodes of their own:

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

A doc block is a sibling placed immediately before whatever it documents, so
pairing them up takes a single pass with one variable of state. That's
exactly what the example below does.

Most parsers throw comments away. Keeping them is what lets you build a
formatter or a documentation generator on top of a parser, rather than only
an interpreter.

## Bytecode

`compile()` is the third stage. It turns the tree into VM instructions.

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

An `Instr` has an `op`, a `line` and a `fields` dictionary, which is the
same structure a `Node` has, and `walk_instrs()` and `find_instrs()` work
exactly like the tree walkers.

Notice the `AddImm` instruction. The compiler included the constant `2` in
the add instruction instead of loading it separately. Questions like this
can only be answered by the bytecode, and reading it is the most direct way
to find out what the compiler actually did with your code:

```zuri
import zuri

echo zuri.compile('var a = 2 * 3 ** 2').map(@(i) => i.op)
```

```console
[LoadConst, LoadConst, LoadConst, Pow, Mul, SetGlobal, LoadNil, Return]
```

The power comes **before** the multiplication, because `**` binds more
tightly than `*`. One line of bytecode settles a question that reading the
expression might not.

### Resolved Operands

Several opcodes only contain an index into the compiler's constant pool,
which doesn't tell you much by itself. Instead of making every caller fetch
and match up a table of constants, each of these fields comes with the
resolved value next to the index:

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

`const_idx` is the index, `value` is the constant it refers to, and `dst` is
the register the result is stored in. The same pairing appears on
`GetGlobal`, `GetField`, `Invoke` and the other opcodes like them.

The resolved value of a `Closure` instruction is a nested function prototype
(`{ name, arity, variadic, instructions }`), whose own `instructions` are
wrapped in the same way, all the way down. So when you compile a file that
contains functions, you can see their bodies too, not only the code around
them.

### Line Numbers Only, Not Columns

Every instruction records the source line it came from, but **never a
column**, because the compiler only tracks lines. Both `Token` and `Node`
have accurate column information from the lexer, but `Instr` doesn't.

## Checking Source Without Running It

`parse()` and `compile()` raise an error at the first sign of invalid
source, which is the right behavior for a tool that needs the complete
program. A tool that works on source while it's being written, half a
statement at a time, needs the opposite: everything that can be read, and a
precise description of what's wrong.

`parse_partial()` reads as much as it can. When a statement fails, the
parser reports it, skips ahead to the start of the next statement and
continues, so each mistake only affects one statement and is reported once:

```zuri
import zuri

var result = zuri.parse_partial(
  'var a = 1
var = 2
var c = 3',
)

echo result.is_clean()
echo result.errors
echo result.nodes.map(@(n) => n.fields.statement.fields.name)
```

```console
false
[2:5: Variable name expected.]
[a, , c]
```

Each error is a `Diagnostic`, which has a `message` and the same six-part
position that a node has, so you can underline it precisely. The statement
that failed is still in the tree, with an empty string where its missing
name should be.

`check()` goes one step further. Source that parses successfully is passed
on to the compiler, which catches problems that the grammar can't see, such
as a `break` outside a loop or a name declared twice in the same scope:

```zuri
import zuri

var source = 'def total(items) {
  var sum = 0
  for item in items {
    sum += item
  }
  break
}'

for problem in zuri.check(source) {
  echo problem
  echo source[problem.start, problem.end]
}
```

```console
6:3: 'break' used outside of a loop
break
```

If `check()` returns an empty list, the source compiles. Neither function
runs any code or raises an error for invalid source, so you can use both of
them on whatever an editor's buffer contains at any moment.

## Reading Files

Each of these functions has a `_file` version that reads a file first:

| Source string | File |
| --- | --- |
| `tokenize(source)` | `tokenize_file(path)` |
| `parse(source)` | `parse_file(path)` |
| `compile(source)` | `compile_file(path)` |
| `parse_partial(source)` | `parse_partial_file(path)` |
| `check(source)` | `check_file(path)` |

There's also one with no string version: **`dump_file(path)`** reads and
parses a file, and returns the `dump()` of every top-level node. It's
usually the quickest way to find out what's actually in a file.

## Example: Extracting Documentation

Here's the technique that this book's own reference appendices are built
with. It takes source code and returns every documented function in it,
pairing each declaration with the doc block above it.

The key is that `zuri.parse()` keeps comments in the tree. A doc block is
returned as a `DocBlock` node of its own, placed as a sibling immediately
before whatever it documents. `zuri.doc.attach()` walks through a list of
siblings and pairs each one with the doc block directly above it, and
`zuri.doc` parses the contents of each block into its text and its `@` tags:

```zuri
import zuri

def documented_functions(text) {
  var found = []

  for attached in zuri.doc.attach(zuri.parse(text)) {
    var node = attached.node

    if node.kind == 'Function' and attached.documented {
      var params = node.fields.parameters.map(@(p) => p.fields.name)

      found.append({ name: node.fields.name, params, doc: attached.block })
    }
  }

  return found
}

var source = "/**\n * Adds two numbers.\n *\n * @param number a\n * @param number b\n */\n" +
    "def add(a: number, b: number) {\n  return a + b\n}\n\n" +
    "def undocumented(x) {\n  return x\n}\n"

for fn in documented_functions(source) {
  var takes = fn.doc.all('param').map(@(tag) => '${tag.name}: ${tag.type}')

  echo '${fn.name}(${', '.join(fn.params)})'
  echo '  ' + fn.doc.summary()
  echo '  takes ' + ', '.join(takes)
}
```

```console
add(a, b)
  Adds two numbers.
  takes a: number, b: number
```

`undocumented` doesn't appear in the output. `attach()` returns it with
`documented` set to false, because there's no doc block above it.

There are several reasons to prefer this approach over scanning the text
yourself.

**The parser knows what a declaration is.** A function named `def_handler`,
a `def` inside a string literal and a doc block inside a block comment would
all confuse a text scan, but none of them confuse the parser.

**Parameter names and types come from the actual nodes.** A parameter's
`type_hint` node contains its types and whether it accepts `nil`, so a
generated signature matches what the runtime will actually enforce, rather
than what the source code seemed to say.

**It stays up to date.** When the grammar gains a new feature, so does the
parser, and a tool built this way keeps working.

**The tags are parsed for you.** `zuri.doc` joins a tag's wrapped lines,
reads its type and the name it documents, and treats `@return` as
`@returns`. It's the same parsing that this book's own reference is
generated with, so a block is read the same way here as it is there.

## Example: A Linter

The other common use for this module is checking code rather than generating
something from it. Here's the kind of rule a team builds up over time:
report every function that has more than a certain number of parameters.

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

There are three reasons to do this with the parser rather than with a
regular expression over the text.

**It can't be fooled by text that looks like code.** A function named
`def_handler`, the word `def` inside a string literal and a commented-out
declaration aren't `Function` nodes, so none of them are counted.

**It reports the correct line.** `node.line` comes from the lexer, so the
message points to the declaration, however the code around it is formatted.

**It keeps working.** When the grammar gains a new feature, so does the
parser, and a rule written this way doesn't silently stop matching.

The dictionary form of the visitor is important here. Only `Function` nodes
are passed to the handler, so you don't need an `if node.kind == ...` check,
and you can't accidentally match a kind you didn't mean to.

## Serializing Without `@to_json`

Reflection is useful when you'd otherwise have to write the same method in
every class:

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

A single function works for every class, and there's no method in each class
to keep in sync with its fields. Note that this also reads private fields,
which is right for a debugging dump but wrong for an API response. For an
API response, either filter out names that start with an underscore, or
write a real `@to_json` and let each class decide what to expose.

## Telling Watch Mode Which Files a Program Reads

`zuri run --watch` restarts a program when a file it depends on changes. The
runtime automatically reports every Zuri source file the program reads. A
library that reads other files that affect the program's behavior declares
each one with `zuri.watch.depends_on()`, which is how Wire gets its
templates watched:

```zuri
import zuri.watch

def load_rules(path) {
  watch.depends_on(path)
  return file(path).read()
}

file('rules.txt', 'w').write('no shouting')

echo load_rules('rules.txt')
echo watch.active()

file('rules.txt').delete()
```

```console
no shouting
false
```

Outside watch mode, `depends_on()` does nothing and `active()` returns
`false`, so a library can always call it. `zuri.watch` is a separate module
from the rest of `zuri`, so importing it doesn't load the parser and the
compiler.

## Other Uses

**Plugin systems.** Load modules from a directory, check whether each one
has the function your program expects, and call the ones that do.
`module_info()` lets you check without having to import modules blindly and
hope for the best.

**Editor tooling.** `tokenize()`, `parse_partial()` and `check()` never
raise errors, so you can run them on a half-written buffer. Every token and
node has `start` and `end` offsets, so a rename or reformat can rewrite
exact ranges of text, and you can underline every problem `check()` finds
exactly where it is.

**Understanding the compiler.** Calling `zuri.compile()` on a snippet is
quicker than reading the compiler's source code, and it always matches the
version of the compiler you're actually running.

**Generating parts of this book.** Appendices D and E are generated from
`libs/` using exactly the techniques described above, and the audit that
checks those stubs works the same way.

## What You Can't Do

There's no `eval()`. You can compile source code to bytecode and inspect it,
but you can't run a string as code.

Zuri leaves it out for security reasons. `eval()` blurs the line between
data and code, and sooner or later, every program that uses it runs a string
it didn't intend to, such as a form field, a query parameter, a
configuration value or a webhook payload. As soon as one of those reaches
`eval()`, whoever provided it can run code with your program's full
permissions. That code can read your files, open network connections and
send your secrets wherever it likes. This is the most damaging class of
vulnerability in dynamic languages, and it keeps happening because the
dangerous call looks harmless in a code review: a single function, a few
characters of input, and nothing in the language warns you.

So Zuri doesn't provide `eval()`, and every task people usually use it for
has a better solution:

| Instead of | Use |
| --- | --- |
| Evaluating an expression entered by a user | A dictionary of handlers keyed by the input |
| Calling a method whose name you computed | `zuri.reflect.bind_method()` |
| Changing behavior at run time | Pass in a function |
| Turning text into data | `json.decode()` |

Each of these does the job with a fixed set of possible outcomes that you
can review. That's the difference between a program that handles its input
and a program that does whatever its input tells it to.

Classes are sealed, so reflection can't add a method to a class at run time
either. Techniques from other languages that depend on monkey patching don't
carry over to Zuri. Instead, express the variation as a subclass, or as a
function that you pass in.
