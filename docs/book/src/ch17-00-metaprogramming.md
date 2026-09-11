# Metaprogramming and Reflection

Zuri's own compiler is available to Zuri programs. The `zuri` module
exposes the lexer, the parser and the bytecode compiler, plus a runtime
reflection API over live functions, classes, modules and instances.

There are two halves, and the distinction matters. **Reflection** asks
questions about objects that exist right now. **The compiler API** asks
questions about source text. A function's arity is a runtime fact; its doc
block is a source fact. Neither half can answer the other's question.

## Runtime Reflection

```zuri
import zuri

def add(a, b) {
  return a + b
}

echo zuri.reflect.kind(add)
echo zuri.reflect.function_info(add)
```

```console
function
{name: add, arity: 2, variadic: false, is_method: false, owning_class_name: nil, source_path: /path/to/main.zu}
```

`kind()` gives a runtime type tag. A closure, a native function and a bound
method all report `'function'`, because from Zuri's point of view they are
interchangeably callable.

### Classes

```zuri
class Point {
  var x = 0
  var y = 0

  @new(x, y) {
    self.x = x
    self.y = y
  }

  distance() {
    return (self.x ** 2 + self.y ** 2).sqrt()
  }
}

echo zuri.reflect.class_info(Point).fields
echo zuri.reflect.class_info(Point).methods.keys()
```

```console
[x, y]
[@new, distance]
```

`class_info()` returns `{ name, superclass_name, methods, fields, statics
}`, with each method's own `function_info` under `methods`.

### Instances

```zuri
var p = Point(3, 4)

echo zuri.reflect.get_props(p)
echo zuri.reflect.get_prop(p, 'x')

zuri.reflect.set_prop(p, 'x', 10)
echo p.x

echo zuri.reflect.has_method(p, 'distance')

var method = zuri.reflect.bind_method(p, 'distance')
echo method()
```

```console
[x, y]
3
10
true
10.770329614269007
```

`bind_method()` gives you a callable already bound to the instance, which
is how you dispatch on a name you computed. `has_decorator()` and
`get_decorator()` do the same for `@`-prefixed methods.

These see private members, for the same reason `getprop` does: serialisers
and debuggers are the use case, and hiding the data from them would defeat
the purpose. Reflection is a tool for infrastructure, not for ordinary
code.

### Modules

`module_info(m)` reports a module's name, path and the members it exports,
which is enough to build a plugin loader: import a directory of modules,
ask each for a known function, and call the ones that have it.

## Tokens

```zuri
import zuri

var tokens = zuri.tokenize('var x = 1 + 2')

echo tokens.length()
echo tokens.map(@(t) => t.kind)
echo tokens[1].text
```

```console
7
[Var, Identifier, Equal, Integer, Plus, Integer, Eof]
x
```

Nothing is filtered. Comments, doc blocks and newlines are all real tokens,
and the list always ends with `Eof`.

Each `Token` carries `kind`, `line`, `column`, `start`, `end`, `text` and
`value`. `start` and `end` are character offsets into the source, so you can
recover a token's exact original text without re-lexing. `is_trivia()` is
true for comments and doc blocks.

Lexing never raises. A malformed input produces a token of kind `'Error'`,
so `tokenize()` is total over any input at all. That is what makes it safe
to point at a file you did not write.

## The Syntax Tree

```zuri
var tree = zuri.parse('def double(n) { return n * 2 }')

echo tree[0].kind
echo tree[0].fields.keys()
```

```console
Function
[name, parameters, body, is_variadic]
```

A `Node` has a `kind` and a `fields` dictionary. The fields hold more
nodes, plain lists, plain dictionaries or scalars, so walking the tree is
uniform regardless of node type.

`walk_nodes(tree, visitor)` visits every node:

```zuri
var names = []

zuri.walk_nodes(tree, @(node) {
  if node.kind == 'Function' {
    names.append(node.fields.name)
  }
})

echo names
```

```console
[double]
```

The visitor can also be a dictionary from kind to handler, in which case
only matching nodes are called.

`find_nodes(tree, kind)` is the shortcut for one kind:

```zuri
echo zuri.find_nodes(tree, 'Return').length()
```

```console
1
```

`parse_file(path)` parses a file, and `dump_file(path)` prints a readable
tree.

Comments and doc blocks survive parsing as real nodes in their original
position, which is what makes a documentation generator possible: find a
`Function` node, look at the node immediately before it, and if it is a
`DocBlock`, that is its documentation.

## Bytecode

```zuri
var chunk = zuri.compile('var a = 1 + 2')

echo chunk.map(@(i) => i.op).take(5)
```

```console
[LoadConst, AddImm, SetGlobal, LoadNil, Return]
```

Every instruction is an `Instr` with an `op` and a `fields` dictionary, the
same shape a `Node` has, and `walk_instrs()` and `find_instrs()` mirror the
AST walkers.

Notice `AddImm` in that output. The compiler folded the constant `2` into
the add instruction rather than loading it separately. Reading the bytecode
is the most direct way to answer "did the compiler do what I hoped?", and
it is exactly what the JIT chapter asks you to do.

`compile_file(path)` does the same for a file.

## What This Is Good For

**Documentation generators.** Parse a file, pair each declaration with the
doc block above it, emit markdown.

**Linters.** Walk the tree and complain about the patterns your team has
decided against.

**Serialisers.** Reflect over an instance's fields instead of writing a
`@to_json` by hand for every class.

**Plugin systems.** Load modules from a directory, ask each one whether it
has the function your host expects.

**Understanding the compiler.** `zuri.compile()` on a snippet is faster
than reading the compiler source, and it never goes out of date.

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
