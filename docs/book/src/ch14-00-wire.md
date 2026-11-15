# Wire

Wire is Zuri's built-in template engine. Unlike most template languages,
Wire doesn't add its own syntax on top of your markup. Every Wire feature
works through attributes on ordinary HTML elements, so anything a designer
already knows about HTML still applies, and every valid HTML5 document is
already a valid Wire template.

A Wire template is compiled once into a small tree of instructions, instead
of being interpreted again on every render. It's parsed with the same
[WHATWG-conformant parser](ch13-00-stdlib-tour.md#html) that the `html`
module uses. One consequence is worth knowing from the start: Wire
understands your markup as *structure*, not as text. It knows that one
interpolation is inside a paragraph, another is inside an `href` and a third
is inside a `<script>` tag, and it escapes each one correctly for where it
is. A value passed to a template can never become new markup by accident. If
you want that, you have to ask for it explicitly.

- [Introduction](#introduction)
  - [Wire and Blade](#wire-and-blade)
  - [Your First Template](#your-first-template)
- [Rendering Templates](#rendering-templates)
  - [Templates From Files](#templates-from-files)
  - [Templates From Strings](#templates-from-strings)
  - [The Template Root](#the-template-root)
- [Displaying Data](#displaying-data)
  - [Escaping Data](#escaping-data)
  - [Rendering Raw Markup](#rendering-raw-markup)
  - [Wire and JavaScript Frameworks](#wire-and-javascript-frameworks)
- [The Expression Language](#the-expression-language)
  - [Literals](#literals)
  - [Looking Up Values](#looking-up-values)
  - [Operators](#operators)
  - [Truthiness](#truthiness)
  - [Calling Functions](#calling-functions)
- [Filters](#filters)
  - [Chaining Filters](#chaining-filters)
  - [The `=` Argument Shorthand](#the--argument-shorthand)
  - [Available Filters](#available-filters)
  - [Writing Your Own Filter](#writing-your-own-filter)
- [Conditionals](#conditionals)
  - [`x-if`, `x-elif`, and `x-else`](#x-if-x-elif-and-x-else)
  - [`x-not`](#x-not)
- [Loops](#loops)
  - [The `loop` Variable](#the-loop-variable)
  - [Nesting Loops](#nesting-loops)
  - [Looping Without a Wrapper Element](#looping-without-a-wrapper-element)
- [Content and Attributes](#content-and-attributes)
  - [`x-text`](#x-text)
  - [`x-html`](#x-html)
  - [`x-attr`](#x-attr)
- [Comments](#comments)
- [Including Templates](#including-templates)
  - [Passing Data to an Include](#passing-data-to-an-include)
  - [Includes as Components](#includes-as-components)
  - [Computed Include Paths](#computed-include-paths)
- [Template Inheritance](#template-inheritance)
  - [Defining a Layout](#defining-a-layout)
  - [Extending a Layout](#extending-a-layout)
  - [Default Slot Content](#default-slot-content)
  - [`x-super`](#x-super)
  - [Multi-Level Inheritance](#multi-level-inheritance)
  - [Overriding a Definition](#overriding-a-definition)
- [Security](#security)
  - [Why Everything Is Escaped by Default](#why-everything-is-escaped-by-default)
  - [URLs Are Checked, Not Just Escaped](#urls-are-checked-not-just-escaped)
  - [Scripts and Stylesheets](#scripts-and-stylesheets)
  - [The Template Root Is a Sandbox](#the-template-root-is-a-sandbox)
- [Extending Wire](#extending-wire)
  - [Custom Filters](#custom-filters)
  - [Globals](#globals)
  - [Custom Elements](#custom-elements)
- [Configuration](#configuration)
- [Compiling and Caching](#compiling-and-caching)
- [Error Handling](#error-handling)
- [Full Directive Reference](#full-directive-reference)
- [Cheat Sheet](#cheat-sheet)

## Introduction

### Wire and Blade

If you've used Laravel's Blade, PHP's best-known template engine, a lot of
Wire will feel familiar, even though the syntax is different. Both engines
compile templates instead of interpreting them on every request, both let
you extend a base layout and override named sections of it, and both escape
everything by default, so displaying a user's name can never let that user
run a script in someone else's browser.

Where Wire differs from Blade is in how it handles control flow. Blade adds
its own directive syntax on top of plain text (`@if`, `@foreach`, `{{ }}`),
which a template author has to learn as a second language on top of HTML.
Wire expresses everything as attributes on the HTML you'd be writing anyway:

```html
{{-- Blade --}}
@if ($user->isAdmin())
    <p>Welcome back, administrator.</p>
@endif
```

```html
{{-- Wire --}}
<p x-if="user.is_admin">Welcome back, administrator.</p>
```

In practice, this means you can give a Wire template to a designer who has
never seen Zuri, and they can still read it. It's just HTML with some
attributes they can look up. It also means every Wire template is valid
HTML5, can be opened directly in a browser to check its structure, and can
be processed with the [`html` module](ch13-00-stdlib-tour.md#html)'s tools
(`html.format()`, a linter, a selector query) without any special handling
for Wire.

### Your First Template

Here's the smallest possible Wire template, rendered from a string:

```zuri
import wire

echo wire.render_string('<p>Hello {{ name }}</p>', { name: 'Ada' })
# <p>Hello Ada</p>
```

`{{ name }}` is an interpolation. It evaluates the expression `name` using
the variables you passed in and writes the result into the page, escaped
correctly for wherever it appears. Everything else in this chapter builds on
that idea, along with a few attributes starting with `x-` that control
*whether* an element is rendered and *how many times*.

## Rendering Templates

### Templates From Files

For anything more than a quick snippet, you'll want to keep templates in
files. Create a `Wire` instance, point it at a directory, and render
templates by their path:

```zuri,ignore
import wire

var view = wire()
view.set_root('./views')

echo view.render('pages/home', { user, posts })
```

The first argument to `render()` is a path relative to the root. If the path
doesn't name an existing file, `.html` is added automatically, so
`'pages/home'` finds `pages/home.html`. A path that already has an
extension, such as `'pages/home.wire'`, is tried exactly as given first.
`set_extension()` changes the extension that's tried when none is given.

> **Note**
> A `Wire` instance is meant to be created once, configured, and reused for
> the lifetime of your program, usually at startup, in the same place you
> configure the rest of your application. Rendering doesn't change the
> instance, so it's safe to render from several places at once.

### Templates From Strings

`render_string()` renders a template passed directly as a string, without
touching the file system. Apart from that, it behaves exactly like
`render()`, with the same directives, escaping and filters, and any
`x-include` or `x-extend` in the string is still resolved relative to the
configured root.

```zuri,ignore
echo view.render_string('<p>{{ greeting }}</p>', { greeting: 'Hi there' })
```

The third argument names the source in error messages, and defaults to
`<source>`. It's worth passing when the string came from somewhere with its
own identity, such as a database row or a file you'd already opened for
another reason:

```zuri,ignore
view.render_string(row.body, { user }, 'cms:page:${row.id}')
```

`render_string()` isn't cached, since there's no file path to use as a cache
key. Use `render()` for anything you render more than once.

### The Template Root

Every path, whether it's passed to `render()` or written in an `x-include`
or `x-extend` in a template, is resolved *inside* the configured root
directory and nowhere else. This isn't just a convention. The loader
enforces it every time it resolves a path, and [it matters for
security](#the-template-root-is-a-sandbox), not only for keeping things
organized.

```zuri,ignore
view.set_root('./views')
view.root()
# '/home/you/project/views' (always an absolute path)
```

The root directory doesn't have to exist yet. `create_root()` creates it and
tells you whether it needed to:

```zuri,ignore
if view.create_root() {
  echo 'Created a fresh views/ directory.'
}
```

Wire never creates this directory by itself. If you make a typo in the root
path, you should get a "template not found" error, not an empty folder
created somewhere unexpected.

## Displaying Data

You've already seen the basic form. `{{ expression }}` evaluates whatever is
between the braces and writes the result into the page:

```html
<h1>{{ post.title }}</h1>
<p>By {{ post.author.name }}</p>
```

`expression` doesn't have to be a variable name. It can be a full
expression, which is covered in its own section below, so all of these are
valid:

```html
<p>{{ post.views > 1000 ? 'Popular' : 'New' }}</p>
<p>{{ post.tags|join(', ') }}</p>
<p>{{ user.nickname ?? user.name }}</p>
```

### Escaping Data

Every interpolation is escaped for the place where it appears, and you can't
turn this off from inside a template. If `name` contains `<b>Ada</b>`:

```html
<p>Hello {{ name }}</p>
```

the result is:

```html
<p>Hello &lt;b&gt;Ada&lt;/b&gt;</p>
```

That's exactly what you want when `name` came from a form field, a database
column or anywhere else outside your control. Wire isn't guessing here.
Because it compiles a parsed document instead of joining strings together,
it always knows exactly what kind of place an interpolation is in, and
applies the escaping that place needs:

| Where the interpolation is | What happens |
| --- | --- |
| Text between tags | `&`, `<` and `>` are replaced with entities |
| An ordinary attribute (`title`, `alt`, `data-*` and so on) | `&` and `"` are replaced with entities |
| `href`, `src`, `action` and other URL attributes | The URL scheme is checked as well |
| `<script>`, or an `on*` event handler attribute | The value is encoded as JSON |
| `<style>` | Any character outside a set that's safe in CSS is removed |

That's a much stronger guarantee than simply replacing special characters. A
value placed in an `href` can't sneak in a `javascript:` URL, and a value
placed in a `<script>` block can't break out of its string literal, close
the tag early or start a comment, even though none of those attacks need
`<`, `>` or `&`.

### Rendering Raw Markup

Escaping everything by default is the right choice, but sometimes you really
do have markup, such as the output of another render or a snippet you built
yourself and trust, and you want to write it out as markup. The `raw` filter
lets you do that:

```html
<div class="article-body">{{ post.rendered_html|raw }}</div>
```

You can also mark a value as safe in your Zuri code before passing it to the
template, with `wire.safe()`:

```zuri,ignore
import wire

view.render_string('<div>{{ body }}</div>', {
  body: wire.safe('<em>already-trusted markup</em>'),
})
```

> **Warning**
> `raw` and `wire.safe()` tell Wire that the value is safe to write without
> escaping where it appears. If you use either one on anything a user
> submitted, such as a comment, a profile or a search query, you reopen the
> cross-site scripting hole that the rest of Wire is designed to close. Only
> use them on markup that your own code produced.

If you want to display markup as text, for example to show a literal
`<script>` tag in a code sample, a plain interpolation already does that.
You don't need to do anything extra.

### Wire and JavaScript Frameworks

Wire uses `{{ }}` for interpolation, which is the same delimiter that many
JavaScript template libraries use, including Vue, Angular, Handlebars and
Mustache. If a Wire template contains inline script that a browser-side
framework is supposed to process, put a `%` in front of the braces so that
Wire leaves them alone:

```html
<div id="app">
  <p>{{ user.name }}</p>          {{-- rendered by Wire, server-side --}}
  <p>%{{ message }}</p>           {{-- left as literal {{ message }}, for Vue --}}
</div>
```

`%{{` produces the literal text `{{`, and `%{!` does the same for [template
function calls](#calling-functions). If you find you're escaping braces all
the time because most of a template belongs to a client-side framework,
consider moving that part into its own file and serving it as is, without
Wire.

## The Expression Language

Everything between `{{` and `}}`, as well as everything you pass to `x-if`,
`x-for`, `x-attr` and the other directives below, is written in Wire's own
small expression language. It's intentionally not a full copy of Zuri:
there's no assignment, no way to declare anything and no way to access
global variables. A template describes a page, and the logic that decides
what goes on the page belongs in the code that calls `render()`, not in the
template.

### Literals

```html
{{ 42 }}            {{-- a number --}}
{{ 3.14 }}           {{-- a decimal --}}
{{ 'a string' }}     {{-- single quotes --}}
{{ "a string" }}     {{-- or double, interchangeably --}}
{{ true }}           {{-- true / false / nil --}}
{{ [1, 2, 3] }}      {{-- a list literal --}}
{{ { id: 1, name } }} {{-- a dict literal; { name } is short for { name: name } --}}
{{ 0..pages }}       {{-- a range, exactly as Zuri writes one --}}
```

### Looking Up Values

```html
{{ user.name }}                 {{-- a dotted lookup --}}
{{ user.address.city }}         {{-- chains as deep as you like --}}
{{ items.0 }}                   {{-- a numeric key reads a list position --}}
{{ items[index] }}              {{-- a computed index --}}
{{ items[-1] }}                 {{-- negative counts from the end --}}
{{ items.length }}               {{-- a collection answers `length` by name --}}
```

Reading a variable that wasn't passed in gives you `nil` instead of an
error, and reading a key from `nil` also gives you `nil`. That lets you
safely reach into an optional value without checking it first:

```html
{{ user.profile.avatar_url }}
```

If `user` has no `profile`, this renders nothing instead of failing three
levels down. Use [`x-if`](#conditionals) when the difference between "empty"
and "missing" affects what you display.

> **Note**
> A template can never read a name that starts with an underscore, in the
> same way that Zuri treats `_field` as private. If you need a value like
> that in a template, expose it through a public accessor in your Zuri code.

### Operators

```html
{{ price * quantity }}
{{ subtotal + tax }}
{{ stock - reserved }}
{{ total / count }}
{{ index % 2 }}

{{ a == b }}   {{ a != b }}
{{ a < b }}    {{ a <= b }}
{{ a > b }}    {{ a >= b }}

{{ 'admin' in user.roles }}
{{ 'admin' not in user.roles }}

{{ is_admin and is_active }}
{{ is_admin && is_active }}      {{-- && is the same as `and` --}}
{{ is_guest or is_banned }}
{{ is_guest || is_banned }}       {{-- || is the same as `or` --}}
{{ !is_active }}
{{ not is_active }}               {{-- ! is the same as `not` --}}

{{ stock > 0 ? 'In stock' : 'Sold out' }}
{{ nickname ?? name }}
```

When either side of `+` is a string, it concatenates instead of raising an
error:

```html
{{ 'Hello, ' + user.name }}
```

`??` and `or` look similar but do different things, and mixing them up is
the most common mistake people make with Wire:

- `??` checks whether the value was passed in at all, and only uses the
  fallback for `nil`.
- `or` checks whether the value is worth displaying, using [Wire's own rules
  for truthiness](#truthiness), and uses the fallback for anything falsy:
  `nil`, `false`, an empty string, an empty collection or the number `0`.

```html
{{ discount ?? 0 }}   {{-- a missing discount becomes 0 --}}
{{ discount or 0 }}   {{-- a discount that IS 0 also becomes 0, harmlessly here --}}

{{ stock_count ?? 'unknown' }}  {{-- 0 in stock still shows as 0 --}}
{{ stock_count or 'unknown' }}  {{-- 0 in stock is treated the same as never supplied --}}
```

Use `??` when zero is a valid value that you want to display, and `or` when
you only want to show something if there's actually something to show.

### Truthiness

`x-if`, `x-not`, `and`/`or`, `!`/`not` and `? :` all follow Wire's own rules
for what's truthy and falsy. These rules differ from Zuri's in one place, on
purpose, to make templates easier to write:

| Value | In Wire |
| --- | --- |
| `nil`, `false` | Falsy |
| `0`, `NaN` | Falsy |
| Any other number, including negative numbers | Truthy |
| An empty string | Falsy |
| A non-empty string | Truthy |
| An empty list or dictionary | Falsy |
| A non-empty list or dictionary | Truthy |
| Anything else | Truthy |

The difference is empty collections. Zuri treats `[]` and `{}` as truthy,
but Wire treats them as falsy, so `x-if="results"` hides a section when a
search returns nothing.

### Calling Functions

You can call a value that's been [registered as a global](#globals)
directly:

```html
<a href="{{ route('user.profile', user.id) }}">{{ user.name }}</a>
```

There's also an older syntax for calling a function that takes no arguments.
It's been kept from Wire's first version because it reads well on its own:

```html
{! current_year !}
```

This is exactly the same as writing:

```html
{{ current_year() }}
```

Use `{{ fn() }}` in new templates. `{! !}` is there for templates that
already use it, and for the few cases, such as a build stamp or a feature
flag, where a function with no arguments reads a little better without the
parentheses.

## Filters

A filter transforms the value to the left of a `|`, much like a Unix pipe:

```html
{{ name|upper }}
{{ price|round(2) }}
{{ post.body|truncate(150) }}
```

The value being filtered is always the filter's first argument, and anything
in parentheses comes after it. `truncate(150)` above calls the `truncate`
filter as `truncate(post.body, 150)`.

### Chaining Filters

Filters are applied from left to right, and the output of each one is passed
to the next:

```html
{{ name|trim|title }}
{{ comment.body|strip_tags|truncate(200) }}
```

### The `=` Argument Shorthand

For a filter that takes exactly one argument, `name=value` is a shorter way
of writing `name(value)`:

```html
{{ status|is='active' }}
```

This is the same as:

```html
{{ status|is('active') }}
```

This syntax was kept for consistency with Wire's first version, and it reads
well for a short comparison. The form with parentheses works everywhere, and
it's the only option once a filter needs more than one argument.

### Available Filters

**Escaping**

| Filter | What it does |
| --- | --- |
| `raw` | [Marks the value as markup](#rendering-raw-markup), so it isn't escaped at all. |
| `escape` / `e` | Escapes the value for a different context from the one it's being written into. Takes `'text'` (the default), `'attribute'`, `'url'`, `'script'` or `'style'`. |

**Text**

| Filter | What it does |
| --- | --- |
| `upper` | Converts the value to uppercase. |
| `lower` | Converts the value to lowercase. |
| `title` | Capitalizes the first letter of every word. |
| `capitalize` | Capitalizes only the first letter and leaves the rest unchanged. |
| `trim` | Removes all leading and trailing whitespace, including spaces, tabs and newlines. |
| `truncate(length, suffix?)` | Shortens the value to `length` characters, adding `suffix` (`'…'` by default) only if something was removed. |
| `replace(search, replacement?)` | Replaces every occurrence of the literal string `search`. It never uses a regular expression. `replacement` defaults to an empty string. |
| `lpad(width, fill?)` | Pads the value on the left to `width` characters with `fill`, which is a space by default. |
| `rpad(width, fill?)` | Pads the value on the right. |
| `repeat(count)` | Repeats the value `count` times. |
| `nl2br` | Escapes the text and then replaces line breaks with `<br>`. Returns markup. |
| `line_breaks` | Another name for `nl2br`. |
| `strip_tags` | Removes all HTML tags and keeps only the text. It parses the HTML properly rather than using pattern matching. |
| `slug` | Converts the value to lowercase words separated by hyphens, safe to use in a URL. |
| `url_encode(reserved?)` | Percent-encodes the value for use in a URL, as `url.encode()` does. Reserved characters such as `/`, `&` and `#` are kept unless `reserved` is `false`, which you need for a single value in a query string: `?q={{ query\|url_encode(false) }}`. |
| `json` | Encodes the value as JSON. |
| `json_script(id?)` | Wraps the value's JSON encoding in a `<script type="application/json">` tag, with an optional `id`, so that a script on the page can read it. Returns markup. |

**Numbers**

| Filter | What it does |
| --- | --- |
| `abs` | Returns the absolute value. |
| `round(places?)` | Rounds to `places` decimal places (`0` by default), with halves rounded away from zero. |
| `floor` | Rounds down to the nearest whole number. |
| `ceil` | Rounds up to the nearest whole number. |
| `number_format(places?, point?, separator?)` | Adds thousands separators and a fixed number of decimal places, as in `1,234,567.89`. Pass `point` and `separator` to use another convention, such as `1.234.567,89`. |
| `filesize(binary?)` | Formats a number of bytes in a readable way, such as `1.4 MB` by default, or `1.3 MiB` when `binary` is `true`. |

**Collections**

| Filter | What it does |
| --- | --- |
| `length` | The number of items. Works on strings, lists, dictionaries and bytes. The length of `nil` is `0`. |
| `first` | The first item, or `nil` if there isn't one. |
| `last` | The last item. |
| `join(glue?)` | Joins the items into a single string with `glue` (an empty string by default) between them. |
| `sort(key?)` | Sorts in ascending order. `key` sorts a list of dictionaries or instances by one of their fields. |
| `reverse` | The items in reverse order, or a reversed string. |
| `unique` | Removes duplicates, keeping the first occurrence of each. |
| `keys` | A dictionary's keys, in insertion order. |
| `values` | A dictionary's values, in insertion order. |
| `slice(start, end?)` | The items from `start` up to but not including `end`. Negative positions count from the end. |
| `sum(key?)` | Adds up the items. `key` adds up one field of a list of dictionaries or instances. |
| `split(separator?)` | Splits a string into a list. By default, it splits on any run of whitespace. |

**Choice**

| Filter | What it does |
| --- | --- |
| `default(fallback)` / `alt` | Returns `fallback` when the value is [falsy](#truthiness), and the value otherwise. |
| `empty` | Whether the value is empty. Unlike falsiness, a number is never empty, not even `0`. |
| `is(expected)` | Whether the value equals `expected`. |
| `not(expected)` | Whether the value differs from `expected`. |

**Dates**

| Filter | What it does |
| --- | --- |
| `date(format?)` | Formats a `date.Date`, a Unix timestamp or a date string that can be parsed, using [the same format codes as `Date.format()`](ch13-00-stdlib-tour.md#date). The default format is `'Y-m-d H:i:s'`. |

```html
<time datetime="{{ post.published_at|date('Y-m-d') }}">
  {{ post.published_at|date('jS F Y') }}
</time>
```

### Writing Your Own Filter

See [Custom Filters](#custom-filters) below.

## Conditionals

### `x-if`, `x-elif`, and `x-else`

`x-if` renders an element, along with everything inside it, only when its
expression is [truthy](#truthiness):

```html
<p x-if="user.is_admin">You have administrator access.</p>
```

If `user.is_admin` is falsy, the whole `<p>`, tag and contents, is left out
of the page completely. No empty element is left behind.

Add more conditions with `x-elif`, and end the chain with a plain `x-else`:

```html
<p x-if="user.role == 'admin'">Administrator</p>
<p x-elif="user.role == 'staff'">Staff member</p>
<p x-elif="user.role == 'contributor'">Contributor</p>
<p x-else>Member</p>
```

Exactly one of these elements is rendered. Only whitespace and HTML comments
are allowed between the elements in a chain, so any real content in between
ends the chain. An `x-elif` or `x-else` without an `x-if` before it is
rejected when the template compiles, rather than silently ignored:

```html
{{-- this chain is broken by the text between the two elements --}}
<p x-if="a">A</p>
some text
<p x-else>not A</p> {{-- error: x-else has no x-if in front of it --}}
```

### `x-not`

`x-not` is simply the opposite of `x-if`. It renders the element when its
expression is falsy, and it can't be part of a chain:

```html
<div x-not="user.has_verified_email">
  <p>Please verify your email address.</p>
</div>
```

`x-if="!condition"` and `x-not="condition"` mean the same thing. `x-not`
exists because it often reads better as a guard clause.

## Loops

`x-for` repeats an element once for each item in whatever its expression
evaluates to, which can be a list, a dictionary, a string or a range:

```html
<ul>
  <li x-for="posts" x-value="post">{{ post.title }}</li>
</ul>
```

Notice that the *element itself* is repeated, not just its contents. The
example above produces a separate `<li>` for each post, not one `<li>`
containing all of them. `x-value` names the variable that holds the current
item on each pass. If you don't need to refer to the item by name, leave it
out.

An optional `x-key` gives you the position (for a list) or the key (for a
dictionary):

```html
<tr x-for="users" x-key="id" x-value="user">
  <td>{{ id }}</td>
  <td>{{ user.name }}</td>
</tr>
```

Every kind of collection can be iterated over:

```html
<li x-for="tags" x-value="tag">{{ tag }}</li>              {{-- a list --}}
<li x-for="scores" x-key="name" x-value="score">           {{-- a dict --}}
  {{ name }}: {{ score }}
</li>
<span x-for="word" x-value="letter">{{ letter }}</span>    {{-- a string, by character --}}
<option x-for="1..5" x-value="n">{{ n }}</option>          {{-- a range --}}
```

A missing or empty collection simply renders nothing, so you don't need to
wrap a loop in an `x-if`:

```html
<li x-for="comments" x-value="comment">{{ comment.body }}</li>
{{-- renders nothing at all if `comments` is empty or was never supplied --}}
```

### The `loop` Variable

On each pass, Wire makes a `loop` variable available, with these fields:

| Field | Value |
| --- | --- |
| `loop.index` | The position, counting from `1` |
| `loop.index0` | The position, counting from `0` |
| `loop.first` | `true` on the first pass |
| `loop.last` | `true` on the last pass |
| `loop.length` | The total number of items |
| `loop.even` | `true` on the 2nd, 4th, 6th and later even-numbered passes, counted by `loop.index` |
| `loop.odd` | `true` on the 1st, 3rd, 5th and later odd-numbered passes |
| `loop.key` | The current key or index, whether or not `x-key` is also used |
| `loop.value` | The current value, whether or not `x-value` is also used |
| `loop.parent` | The `loop` variable of the enclosing loop, in a nested `x-for` |

```html
<tr x-for="rows" x-value="row" x-attr="{ 'class': loop.odd ? 'zebra' : nil }">
  <td>{{ loop.index }}</td>
  <td>{{ row.name }}</td>
</tr>
```

### Nesting Loops

`x-loop` renames the metadata variable for a particular `x-for`, so you can
refer to both the inner loop's `loop` and the outer loop's `loop` at the
same time:

```html
<table x-for="rows" x-loop="row" x-value="cells">
  <tr>
    <td x-for="cells" x-value="cell">
      {{ row.index }}, {{ loop.index }}: {{ cell }}
    </td>
  </tr>
</table>
```

Without `x-loop`, the inner loop's `loop` hides the outer one inside the
inner loop. If you'd rather not rename anything, use `loop.parent` instead:

```html
<td x-for="cells" x-value="cell">
  outer position {{ loop.parent.index }}, inner position {{ loop.index }}
</td>
```

### Looping Without a Wrapper Element

Sometimes you want to repeat several elements together without wrapping them
in a real element, or without adding any element at all. Put the directive
on a `<template>` element instead of on the element you want to repeat:

```html
<select>
  <template x-for="countries" x-value="country">
    <option value="{{ country.code }}">{{ country.name }}</option>
  </template>
</select>
```

`<template>` is the one HTML element the parser accepts unchanged wherever
it appears, including inside a `<head>`, a `<table>` or a `<select>`. Wire
removes the `<template>` itself from the output and keeps only its contents,
once for each pass. You can also use it to apply [`x-if`](#conditionals) to
a group of elements without choosing one of them to carry the attribute:

```html
<template x-if="user.is_admin">
  <a href="/admin">Dashboard</a>
  <a href="/admin/users">Users</a>
</template>
```

## Content and Attributes

### `x-text`

`x-text` replaces an element's children with the value of its expression,
escaped as plain text. It's useful when the element already has other
attributes and you don't want to write the value twice:

```html
<p x-text="post.summary"></p>
```

This is the same as:

```html
<p>{{ post.summary }}</p>
```

Anything written inside the element in the source is thrown away. It's only
there to show what would appear on the page without JavaScript.

### `x-html`

`x-html` is the unescaped version of `x-text`. It replaces the element's
children with the value of its expression, written as markup instead of
text:

```html
<div x-html="post.rendered_body|raw"></div>
```

Note the `|raw`. `x-html` still expects a value that's been marked as safe,
just like an ordinary interpolation does. `x-html` only changes *where* the
markup goes, replacing the element's contents instead of appearing inline in
the text. It doesn't turn off escaping by itself. The [warning about
untrusted input](#rendering-raw-markup) applies here just as it does to the
`raw` filter.

### `x-attr`

`x-attr` takes a dictionary of names and values and adds them to the element
as attributes:

```html
<input x-attr="{ type: 'text', name: field.name, required: field.is_required }">
```

In that dictionary:

- `true` produces an attribute without a value, such as `required`, just as
  if you'd written `required` yourself.
- `false` and `nil` leave the attribute out completely, instead of writing
  it with an empty value or the text `"false"`.
- Any other value is written as the attribute's value, escaped according to
  what the attribute means. As always, a URL attribute is checked as a URL.

This lets you turn a boolean into a real HTML boolean attribute without
writing a conditional expression everywhere you need one:

```html
<button x-attr="{ disabled: !form.is_valid }">Submit</button>
```

An attribute set by `x-attr` overrides one written directly on the same
element, so you can write a sensible default and only override it when you
need to:

```html
<div class="card" x-attr="{ 'class': featured ? 'card card-featured' : nil }">
```

## Comments

In a Wire template, an ordinary HTML comment is a note for the server. It
never appears in the rendered page, and nothing inside it is evaluated. That
means you can safely leave notes for other people who maintain the template
without sending them to the browser:

```html
<!-- TODO: replace this hard-coded banner once marketing sends the real copy -->
<div class="banner">Coming soon</div>

<!-- this variable is not evaluated: {{ some.internal.detail }} -->
```

If you really do want a comment to reach the browser, such as a conditional
comment or a build stamp that a deployment script checks for, turn comments
back on for that `Wire` instance with
[`set_comments(true)`](#configuration).

## Including Templates

`<include path="..." />` renders another template in its place. You can also
write it as a directive, `<template x-include="...">`, which means exactly
the same thing. The element forms in this chapter (`<include>`, `<extend>`,
`<declare>` and `<define>`) are shorthand. They're rewritten into the
directive form before the template is parsed, which is why they work
correctly inside a `<head>` or a `<table>`, where the parser would otherwise
move or drop an element it doesn't recognize.

```html
<include path="partials/nav.html" />
```

This is the same as:

```html
<template x-include="partials/nav.html"></template>
```

The path is resolved [inside the template root](#the-template-root), just
like the path passed to `render()`.

By default, an included template can see every variable that the surrounding
page can see. It doesn't have a separate scope:

```html
{{-- page.html --}}
<include path="partials/greeting.html" />
```

```html
{{-- partials/greeting.html --}}
<p>Hi, {{ user.name }}!</p>
```

This renders correctly even though `user` was never passed to the partial
explicitly.

### Passing Data to an Include

`x-with` adds variables for the included template, in addition to the ones
it already inherits:

```html
<include path="partials/badge.html" x-with="{ label: 'New', tone: 'green' }" />
```

`only` hides the surrounding scope completely, so the partial only sees what
`x-with` gives it. This turns a plain partial into something more like a
component, with a defined interface:

```html
<include path="components/price-tag.html" x-with="{ amount: item.price }" only />
```

### Includes as Components

Anything you write *inside* an `<include>` tag is passed to the included
template as a named region called `content`, which the template declares
with `<declare name="content">`:

```html
{{-- components/card.html --}}
<section class="card">
  <h3>{{ title }}</h3>
  <declare name="content"></declare>
</section>
```

```html
{{-- the page --}}
<include path="components/card.html" x-with="{ title: 'Recent Orders' }">
  <p>{{ orders|length }} orders this week</p>
</include>
```

The result is:

```html
<section class="card">
  <h3>Recent Orders</h3>
  <p>3 orders this week</p>
</section>
```

The content inside the `<include>` tag keeps the scope it was written in,
which is the page's scope, not the partial's. That's why
`{{ orders|length }}` above can read a variable from the page, even though
`x-only` wasn't used and the card component never refers to `orders`.

### Computed Include Paths

A path is read as literal text, with `{{ }}` interpolation allowed inside
it. It isn't an expression, so `x-include="header"` refers to a file called
`header`, not to a variable with that name:

```html
<include path="themes/{{ current_theme }}/header.html" />
```

This resolves to a different file on each render, depending on
`current_theme`. Everything after `path="themes/"` up to the next `{{` is
taken literally.

## Template Inheritance

Includes are for small, reusable pieces such as a navigation bar, a footer
or a badge. For the overall structure of a page, meaning the parts of the
layout that every page on your site shares, template inheritance works
better. Includes combine pieces, while inheritance lets a base layout define
the *shape* of a page once, so each page only fills in the parts that are
different.

### Defining a Layout

A base template marks the regions that a page can fill in with
`<declare name="...">`:

```html
{{-- layouts/app.html --}}
<!DOCTYPE html>
<html>
  <head>
    <title>{{ title }}</title>
    <declare name="head"></declare>
  </head>
  <body>
    <nav><!-- shared navigation --></nav>
    <main>
      <declare name="content"></declare>
    </main>
    <footer>
      <declare name="footer">
        <p>&copy; {{ year }} My Company</p>
      </declare>
    </footer>
  </body>
</html>
```

Notice that `footer` already has some content inside its `<declare>` tag.
That's [the default](#default-slot-content), which is rendered when a page
doesn't define that region.

### Extending a Layout

A page states which layout it extends with `<extend base="...">`, and fills
in regions with `<define name="...">`:

```html
{{-- pages/dashboard.html --}}
<extend base="layouts/app.html">
  <define name="head">
    <meta name="description" content="Your account dashboard.">
  </define>
  <define name="content">
    <h1>Welcome back, {{ user.name }}</h1>
    <p>You have {{ notifications|length }} new notifications.</p>
  </define>
</extend>
```

Rendering `pages/dashboard.html` produces the complete layout, with `head`
and `content` replaced by what the page defined, and `footer` left as the
layout's own default. The layout and the page are both rendered with the
same variables that were passed to `render()`, so there's no separate scope
that you need to pass values through.

Everything inside an `<extend>` has to be inside a `<define>`. The base
template owns the structure of the page, and a page's job is only to fill in
the regions the layout offers, not to add markup of its own outside them. A
template can only extend one base template.

### Default Slot Content

A region that a page doesn't define keeps whatever the layout put inside its
`<declare>` tag:

```html
{{-- pages/minimal.html --}}
<extend base="layouts/app.html">
  <define name="content">
    <p>Just this page's content — head and footer both use the layout's defaults.</p>
  </define>
</extend>
```

A `<declare>` tag with nothing inside it, like `head` in the layout above,
simply renders nothing when no page defines it.

### `x-super`

`<super />`, or its directive form `<template x-super></template>`, renders
whatever the region it's in would have shown if this definition hadn't
replaced it. That lets a page *add to* a region instead of repeating all of
it:

```html
<extend base="layouts/app.html">
  <define name="footer">
    <super />
    <p><a href="/privacy">Privacy Policy</a></p>
  </define>
</extend>
```

This renders the layout's own copyright line, followed by the extra privacy
link, without the page needing to know or repeat what the layout's default
footer said.

### Multi-Level Inheritance

A template that extends a base can declare regions of its own, which lets
another template extend it in turn. This is how you'd usually structure a
site with a general layout and several layouts for specific kinds of page,
such as a blog post or a product page:

```html
{{-- layouts/app.html --}}
<html><body>
  <declare name="body"><p>default</p></declare>
</body></html>
```

```html
{{-- layouts/article.html --}}
<extend base="layouts/app.html">
  <define name="body">
    <article>
      <declare name="article-content"></declare>
    </article>
  </define>
</extend>
```

```html
{{-- pages/post.html --}}
<extend base="layouts/article.html">
  <define name="article-content">
    <h1>{{ post.title }}</h1>
    {{ post.body|raw }}
  </define>
</extend>
```

Rendering `pages/post.html` follows the whole chain. It extends
`layouts/article.html`, which extends `layouts/app.html`, and the final page
is assembled from all three.

### Overriding a Definition

Defining the same region twice in one template is nearly always a mistake,
for example two people editing the same file, or a copy and paste that was
never cleaned up. So Wire rejects it at compile time, unless you mark the
second definition with `override` to show that it's intentional:

```html
<extend base="layouts/app.html">
  <define name="content"><p>First draft</p></define>
  <define name="content" override><p>Final version</p></define>
</extend>
```

Without `override` on the second definition, this template fails to compile
with a clear message that names the region and both locations, instead of
silently keeping whichever definition came last.

## Security

### Why Everything Is Escaped by Default

Part of a template engine's job is to stop values that came from outside
your program, such as a form submission, a query string or another user's
profile, from turning into markup, a script or a link, unless you explicitly
say they're safe. Wire does this by default rather than as an option you
have to enable. As soon as escaping is something you have to *remember* to
turn on, the templates where someone forgot are the ones that get exploited.

Every interpolation is escaped for the place where it appears (see the
[table earlier in this chapter](#escaping-data)), and there's no template
setting that turns this off. The only way around it is the explicit and
easy-to-spot [`raw` filter or `x-html` directive](#rendering-raw-markup).
That's the right amount of friction between displaying a value and trusting
it as unescaped markup.

### URLs Are Checked, Not Just Escaped

Attributes that a browser treats as URLs, such as `href`, `src`, `action`
and several others, get more than entity escaping. Their scheme is checked
against an allowlist, and a scheme that isn't allowed is replaced with
`about:blank` instead of being written out:

```html
<a href="{{ profile_link }}">{{ user.name }}</a>
```

If `profile_link` were `javascript:alert(document.cookie)`, the rendered
`href` would be `about:blank`, not the script URL. A URL without a scheme,
which includes every relative link, absolute path and protocol-relative URL,
is always allowed, because none of them can refer to a handler.

The default allowlist includes `http`, `https`, `mailto`, `tel` and a few
others, and it deliberately leaves out `javascript`, `vbscript`, `data` and
`file`. If your application needs another scheme, such as a custom `app:`
handler, use [`set_url_schemes()`](#configuration) to replace the list.

`srcset` and `ping` each contain a *list* of URLs, and every URL in the list
is checked in the same way. Descriptors such as `2x` and `640w`, and the
separators between entries, are kept exactly as written.

### Scripts and Stylesheets

A value interpolated inside a `<script>` element, or inside an event handler
attribute such as `onclick`, is encoded as JSON instead of just being
escaped. This happens automatically, with no filter needed:

```html
<script>
  var currentUser = {{ user }};
</script>
```

This renders the whole `user` value as a JSON literal, quotes included. Keep
this in mind, because it changes how you write the script around it:
**don't** put your own quotes around the interpolation.

```html
{{-- correct: renders   var name = "Ada";   --}}
<script>var name = {{ user.name }};</script>

{{-- wrong: renders   var name = '"Ada"';   which is not what you meant --}}
<script>var name = '{{ user.name }}';</script>
```

In a `<style>` block, any character outside a conservative set that's safe
in CSS is removed rather than escaped. No escape sequence would stop the
HTML tokenizer from reading a `<` while it looks for `</style>`, so the only
safe option is to remove the character.

### The Template Root Is a Sandbox

Every path, whether it was passed to `render()` or written in an `x-include`
or `x-extend` inside a template, even one built from an interpolated value,
is resolved *inside* the configured root. If it resolves anywhere else,
including through `..` segments or symbolic links, it's rejected:

```html
<include path="{{ theme }}/header.html" />
```

If `theme` ever came from something a request controls, a loader without
this check would let an attacker read any file the process can access.
Wire's loader treats the root as a strict boundary instead, so the worst a
malicious value can do here is fail to find a template.

## Extending Wire

### Custom Filters

`register_filter()` adds a new filter, or replaces an existing one with the
same name. The value being filtered is always the first argument, followed
by any arguments the template passes:

```zuri,ignore
view.register_filter('excerpt', @(value, words) {
  var count = words == nil ? 25 : words
  return ' '.join(value.split('/\\s+/').take(count)) + '…'
})
```

```html
<p>{{ post.body|excerpt(40) }}</p>
```

An argument that the template leaves out is `nil`, which is why
`words == nil ? 25 : words` above provides a default instead of requiring
the template to pass a value every time.

When a filter returns a plain value, that value is escaped like any other
interpolation. A filter that produces markup should return
[`wire.safe()`](#rendering-raw-markup), and from then on it's responsible
for what's inside, just as `raw` and `x-html` are.

### Globals

`register_global()` makes a value available to every template, without
passing it to `render()` each time:

```zuri,ignore
view.register_global('site_name', 'Example Inc.')
view.register_global('route', @(name) {
  return '/' + name
})
```

```html
<title>{{ page_title }} — {{ site_name }}</title>
<a href="{{ route('user.profile', user.id) }}">{{ user.name }}</a>
```

A variable with the same name passed to `render()` takes priority over a
global. A global is a default, not a fixed value, so a page can still
override `site_name` if it needs to.

### Custom Elements

For the rare cases that the directives can't handle, `register_element()`
takes over an HTML tag name and passes every element with that name to a
Zuri function instead of writing it out:

```zuri,ignore
view.register_element('icon', @(view, element) {
  var name = element.attributes.get('name', 'dot')
  return wire.safe('<svg class="icon"><use href="#${name}"></use></svg>')
})
```

```html
<icon name="star" />
```

The function receives the `Wire` instance and a dictionary that describes
the element: its tag name, its attributes (already rendered and escaped),
its rendered children and the scope it was written in. Return
`wire.safe(markup)` to write markup, any other value to write it as escaped
text, or `nil` to write nothing. Directives still work on a custom element
as you'd expect, so `<icon x-if="..." />` behaves the way it looks. This
feature is meant for markup that really does have to be generated by code,
where a partial and `x-include` aren't enough. Try an include first. It's
easier for the next person to read, and they won't have to go looking for
the Zuri function behind it.

## Configuration

You can pass every setting to the `Wire` constructor at once, or set each
one later with its own method:

```zuri
var view = wire({
  root: './views',
  extension: '.html',
  compact: true,
  comments: false,
  auto_reload: true,
  url_schemes: ['https', 'mailto'],
})
```

| Option / method | Default | What it controls |
| --- | --- | --- |
| `root` / `set_root(path)` | `./templates` | The directory that every template path is resolved inside. |
| `extension` / `set_extension(ext)` | `.html` | The extension tried when a path doesn't name an existing file. Must start with `.`. |
| `compact` / `set_compact(bool)` | `false` | Removes text between tags that's only whitespace. |
| `comments` / `set_comments(bool)` | `false` | Whether HTML comments appear in the rendered output. |
| `auto_reload` / `set_auto_reload(bool)` | `true` | Whether a cached template is checked against its file before each render, and read again if the file has changed. |
| `url_schemes` / `set_url_schemes(list)` | See [above](#urls-are-checked-not-just-escaped) | Which URL schemes `href`, `src` and similar attributes can use. |

`root()` returns the configured root as an absolute path, and
`create_root()` creates the directory if it doesn't exist yet, returning
whether it had to.

## Compiling and Caching

The first time a template is used, `render()` compiles it into a tree of
instructions and keeps the result for later. Parsing and resolving
directives happen once per template, not once per request, and every later
render just walks the tree.

When `auto_reload` is on, which is the default, each render compares the
file's modification time and size with the cached values and recompiles the
template if either has changed. That costs one file system `stat` per
template per render, which is cheap and exactly what you want while you're
editing templates. Once you're running under load and no longer editing
templates, turn it off with `set_auto_reload(false)`. A long-running process
can then call `clear_cache()` to pick up a new deployment.

Under `zuri run --watch`, a template change does more than reload the
template. Changing any template the program has rendered restarts the
program, just as changing its code does, because Wire tells watch mode about
every template file it reads.

`compile(path)` compiles a template without rendering it and returns the
compiled result, or raises an error if the template has a mistake in it.
That makes it useful for checking a whole directory of templates at startup,
so that a broken template is caught before the first request that would use
it:

```zuri,ignore
for name in os.read_dir('./views', true) {
  view.compile(name)
}
```

## Error Handling

Every error that Wire raises is a `wire.WireError`, and each one includes
the template path and the line and column of the tag that caused it. That
means catching the base class is enough to handle any template failure in
one place, whether you turn it into a 500 page, log it with context or do
something else your application needs:

```zuri
catch {
  echo view.render('pages/dashboard', { user })
} as e {
  if instance_of(e, wire.WireError) {
    log.error('template failed: ${e.reason} at ${e.location()}')
    echo view.render('errors/500')
  }
}
```

Three more specific error classes, all subclasses of `WireError`, tell you
*what kind* of problem occurred:

| Error | Raised when |
| --- | --- |
| `wire.TemplateSyntaxError` | A template doesn't compile, for example because of an unknown directive, an invalid expression or a broken inheritance chain. It's always raised the first time the template is used, not on a later render. |
| `wire.TemplateNotFoundError` | A path doesn't name a file, or resolves to somewhere outside the template root. |
| `wire.RenderError` | Something went wrong because of the values a template received, for example iterating over something that can't be iterated, a filter rejecting its input, or a chain of includes that never ends. |

A variable that was never passed in is *not* an error. It renders as nothing
and counts as falsy, by design, so you can write an optional section without
guarding every field it uses.

## Full Directive Reference

These are all the directives Wire understands. Any other attribute starting
with `x-` fails to compile, because Wire reserves the whole prefix. That
way, a typo such as `x-fi` is caught as soon as the template compiles,
rather than silently doing nothing.

| Directive | Takes | Meaning |
| --- | --- | --- |
| `x-if` | Expression | Renders the element only if the expression is truthy. Starts a chain. |
| `x-elif` | Expression | Continues an `x-if` chain. |
| `x-else` | *(nothing)* | Ends an `x-if` chain. |
| `x-not` | Expression | Renders the element only if the expression is falsy. Can't be part of a chain. |
| `x-for` | Expression | Repeats the element once for each item. |
| `x-value` | Name | Names the value on each pass. |
| `x-key` | Name | Names the key or index on each pass. |
| `x-loop` | Name | Renames the loop metadata variable. |
| `x-text` | Expression | Replaces the children with escaped text. |
| `x-html` | Expression | Replaces the children with unescaped markup. |
| `x-attr` | Expression (dictionary) | Adds attributes to the element. |
| `x-include` | Path | Renders another template in place of this element. |
| `x-with` | Expression (dictionary) | Adds variables for an include. |
| `x-only` | *(nothing)* | Hides the surrounding scope from an include. |
| `x-extend` | Path | Makes this template extend the named base template. |
| `x-slot` | Name | Declares a region that an extending template can replace. |
| `x-define` | Name | Replaces a region of the base template. |
| `x-override` | *(nothing)* | Allows a region that's already been defined to be defined again. |
| `x-super` | *(nothing)* | Renders the definition that this one replaces. |

Five elements are shorthand for the directives above, and they're rewritten
before the template is parsed:

| Element | Equivalent to |
| --- | --- |
| `<include path="p" />` | `<template x-include="p"></template>` |
| `<extend base="p">…</extend>` | `<template x-extend="p">…</template>` |
| `<declare name="n">…</declare>` | `<template x-slot="n">…</template>` |
| `<define name="n">…</define>` | `<template x-define="n">…</template>` |
| `<super />` | `<template x-super></template>` |

## Cheat Sheet

```html
{{-- Variables --}}
{{ name }}
{{ user.address.city }}
{{ items.0 }}
{{ items[index] }}

{{-- Filters --}}
{{ name|upper }}
{{ price|round(2) }}
{{ post.body|truncate(150) }}
{{ status|is='active' }}

{{-- Raw markup --}}
{{ trusted_html|raw }}

{{-- Conditionals --}}
<p x-if="condition">…</p>
<p x-elif="other">…</p>
<p x-else>…</p>
<p x-not="condition">…</p>

{{-- Loops --}}
<li x-for="items" x-value="item" x-key="i">{{ i }}: {{ item }}</li>
{{ loop.index }} {{ loop.first }} {{ loop.last }} {{ loop.length }}

{{-- Wrapper-free groups --}}
<template x-for="items" x-value="item">…</template>
<template x-if="condition">…</template>

{{-- Content and attributes --}}
<p x-text="value"></p>
<div x-html="trusted_html|raw"></div>
<input x-attr="{ type: 'text', disabled: !enabled }">

{{-- Comments (never rendered) --}}
<!-- a note for the next person -->

{{-- Includes --}}
<include path="partials/nav.html" />
<include path="components/card.html" x-with="{ title: 'X' }" only>
  content for the component's declared region
</include>

{{-- Inheritance --}}
<extend base="layouts/app.html">
  <define name="content">…</define>
  <define name="footer"><super />…</define>
</extend>
```

Further reading: the [`html` module](ch13-00-stdlib-tour.md#html) that Wire's parser
and serializer are built on, and the [`date` module](ch13-00-stdlib-tour.md#date)
for the format directives the `date` filter accepts.
