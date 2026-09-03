# Wire

Wire is Zuri's built-in templating engine. Unlike most templating
languages, Wire does not invent its own syntax on top of your markup.
Every Wire feature is powered by attributes on ordinary HTML elements,
which means anything a designer already knows about HTML transfers
directly, and every valid HTML5 document is already a valid Wire
template.

Under the hood, a Wire template is compiled once — not re-interpreted
on every render — into a small instruction tree, using the exact same
[WHATWG-conformant parser](/docs/html.md) that backs the `html` module.
That has a consequence worth knowing up front: Wire understands your
markup as *structure*, not as text. It knows that one interpolation
sits inside a paragraph, another inside an `href`, and a third inside a
`<script>` tag, and it escapes each one correctly for where it actually
is. A value handed to a template can never turn into new markup by
accident — that has to be requested explicitly, and Wire makes you say
so out loud.

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

If you have used Laravel's Blade, PHP's own answer to templating, a lot
of Wire will feel familiar in spirit even though the syntax is
different. Both engines compile templates rather than interpret them
on every request, both let you extend a base layout and override named
sections of it, and both escape everything by default so that printing
a user's name can never become a way for that user to run script in
someone else's browser.

Where Wire departs from Blade is in how it expresses control flow.
Blade adds its own directive syntax on top of plain text
(`@if`, `@foreach`, `{{ }}`) that a template author has to learn as a
second language layered over HTML. Wire instead expresses everything
as attributes on the HTML you were already going to write:

```blade
{{-- Blade --}}
@if ($user->isAdmin())
    <p>Welcome back, administrator.</p>
@endif
```

```wire
{{-- Wire --}}
<p x-if="user.is_admin">Welcome back, administrator.</p>
```

The practical benefit is that a Wire template can be handed to a
designer who has never seen Zuri and they can still read it: it is
HTML, with some attributes they can look up. It also means every Wire
template validates as HTML5, can be opened directly in a browser to
check its structure, and can be run through the [`html`
module](/docs/html.md)'s own tools (`html.format()`, a linter, a
selector query) without anything special-casing Wire's own syntax.

### Your First Template

Here is the smallest possible Wire template, rendered from a string:

```zuri
import wire

echo wire.render_string('<p>Hello {{ name }}</p>', { name: 'Ada' })
# <p>Hello Ada</p>
```

`{{ name }}` is an interpolation: it evaluates the expression `name`
against the variables you supplied and writes the result into the
page, escaped for wherever it landed. Everything else in this guide is
built out of that one idea, plus a handful of `x-` prefixed attributes
that control *whether* and *how many times* an element is rendered.

## Rendering Templates

### Templates From Files

For anything beyond a one-off snippet, templates live in files. Build
a `Wire` instance, point it at a directory, and render by path:

```zuri
import wire

var view = wire.wire()
view.set_root('./views')

echo view.render('pages/home', { user, posts })
```

`render()`'s first argument is a path relative to the root. The
`.html` extension is added automatically when the path as written
names no file, so `'pages/home'` finds `pages/home.html`; a path that
already carries an extension (`'pages/home.wire'`) is tried exactly as
given first. `set_extension()` changes what gets tried when none is
given.

> **Note**
> A `Wire` instance is meant to be built once, configured, and reused
> for the life of your program — typically at startup, alongside
> however you already configure the rest of your application.
> Rendering does not mutate it, so it is safe to render from several
> places at once.

### Templates From Strings

`render_string()` renders a template given directly as a string,
without touching the filesystem. It behaves identically to `render()`
in every other respect — the same directives, the same escaping, the
same filters — and any `x-include` or `x-extend` inside the string
still resolves against the configured root.

```zuri
echo view.render_string('<p>{{ greeting }}</p>', { greeting: 'Hi there' })
```

The third argument names the source for error messages (it defaults
to `<source>`), which is worth passing when the string came from
somewhere with its own identity — a database row, a file you already
had open for another reason:

```zuri
view.render_string(row.body, { user }, 'cms:page:${row.id}')
```

`render_string()` is not cached, since there is no file path to key a
cache entry on. Reach for `render()` for anything rendered more than
once.

### The Template Root

Every path — the one passed to `render()`, and the one written in
every `x-include` and `x-extend` in every template — resolves *inside*
the configured root directory, and nowhere else. This is not a
convention; it is enforced by the loader on every single resolution,
and [it matters for security](#the-template-root-is-a-sandbox), not
just organization.

```zuri
view.set_root('./views')
view.root()
# '/home/you/project/views' — always the absolute path
```

The root does not have to exist yet. `create_root()` makes it, and
reports whether it had to:

```zuri
if view.create_root() {
  echo 'Created a fresh views/ directory.'
}
```

Wire never creates this directory on its own initiative — a typo in a
root path should read as "template not found," not silently produce
an empty folder somewhere unexpected.

## Displaying Data

You have already seen the basic form. `{{ expression }}` evaluates
whatever is between the braces and writes the result into the page:

```wire
<h1>{{ post.title }}</h1>
<p>By {{ post.author.name }}</p>
```

`expression` is not limited to a bare variable name — it is a full
expression, covered in its own section below — so all of the following
are valid:

```wire
<p>{{ post.views > 1000 ? 'Popular' : 'New' }}</p>
<p>{{ post.tags|join(', ') }}</p>
<p>{{ user.nickname ?? user.name }}</p>
```

### Escaping Data

Every interpolation is escaped for the specific place it lands, and
this cannot be turned off from inside a template. If `name` holds
`<b>Ada</b>`:

```wire
<p>Hello {{ name }}</p>
```

renders as:

```html
<p>Hello &lt;b&gt;Ada&lt;/b&gt;</p>
```

which is exactly what you want when `name` came from a form field, a
database column, or anywhere else outside your own control. Wire is
not guessing at this — because it compiles a real parsed document
rather than gluing strings together, it always knows precisely which
kind of place an interpolation sits in, and it applies the escaping
that place needs:

| Where the interpolation is                       | What happens                        |
| -------------------------------------------------- | ------------------------------------ |
| Text between tags                                  | `&`, `<`, `>` become entities        |
| An ordinary attribute (`title`, `alt`, `data-*`, …) | `&` and `"` become entities          |
| `href`, `src`, `action`, and other URL attributes  | the value's URL scheme is checked too |
| `<script>`, or an `on*` event handler attribute    | the value is encoded as JSON         |
| `<style>`                                          | anything outside a CSS-safe set is dropped |

That is a materially stronger guarantee than "the special characters
got replaced": a value dropped into an `href` cannot smuggle in a
`javascript:` scheme, and a value dropped into a `<script>` block
cannot break out of its string literal, close the tag early, or open a
comment — even though none of those things are `<`, `>`, or `&`.

### Rendering Raw Markup

Escaping everything by default is right, but sometimes you genuinely
have markup — the output of another render, a snippet you built and
trust — and you want it written out as markup. The `raw` filter is how
you say so:

```wire
<div class="article-body">{{ post.rendered_html|raw }}</div>
```

You can build the same value on the Zuri side and hand it to a
template already marked as safe, with `wire.safe()`:

```zuri
import wire

view.render_string('<div>{{ body }}</div>', {
  body: wire.safe('<em>already-trusted markup</em>'),
})
```

> **Warning**
> `raw` and `wire.safe()` are a promise that the value is safe to write
> unescaped at the place it lands. Applying either to anything a user
> submitted — a comment, a bio, a search query — reopens exactly the
> cross-site scripting hole the rest of Wire exists to close. Only
> reach for them on markup your own code produced.

If you need to render markup as text on purpose — showing someone a
literal `<script>` tag in a code sample, say — that is what plain
interpolation already does; there is nothing extra to opt into.

### Wire and JavaScript Frameworks

Wire uses `{{ }}` for interpolation, the same delimiter many
JavaScript templating libraries (Vue, Angular, Handlebars, Mustache)
use for their own. If a Wire template also contains inline script that
a browser-side framework is meant to interpret, escape the braces with
a leading `%` so Wire leaves them alone:

```wire
<div id="app">
  <p>{{ user.name }}</p>          {{-- rendered by Wire, server-side --}}
  <p>%{{ message }}</p>           {{-- left as literal {{ message }}, for Vue --}}
</div>
```

`%{{` renders as the literal text `{{`, and `%{!` does the same for
[template function calls](#calling-functions). If you find yourself
escaping braces constantly because most of a template belongs to a
client-side framework, it may be worth keeping that section in its own
file and serving it untouched rather than through Wire at all.

## The Expression Language

Everything between `{{` and `}}` — and everything given to `x-if`,
`x-for`, `x-attr`, and the rest of the directives below — is written in
Wire's own small expression language. It is deliberately not a full
embedded copy of Zuri: there is no assignment, no way to declare
anything, and no way to reach a global variable. A template describes
a page; the logic that decides what the page contains belongs in the
code that calls `render()`, not in the template itself.

### Literals

```wire
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

```wire
{{ user.name }}                 {{-- a dotted lookup --}}
{{ user.address.city }}         {{-- chains as deep as you like --}}
{{ items.0 }}                   {{-- a numeric key reads a list position --}}
{{ items[index] }}              {{-- a computed index --}}
{{ items[-1] }}                 {{-- negative counts from the end --}}
{{ items.length }}               {{-- a collection answers `length` by name --}}
```

Reading a variable that was never supplied gives `nil` rather than
raising, and reading a key off `nil` gives `nil` too. That is what
makes an optional value safe to reach through without a guard in front
of it:

```wire
{{ user.profile.avatar_url }}
```

renders as nothing at all if `user` has no `profile`, instead of
failing three levels down. Reach for [`x-if`](#conditionals) when the
difference between "empty" and "genuinely missing" matters to what you
show.

> **Note**
> A name starting with an underscore can never be read from a
> template, the same way Zuri treats `_field` as private. If you find
> yourself wanting to read one, expose a public accessor from Zuri
> instead.

### Operators

```wire
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

A string on either side of `+` concatenates rather than raising:

```wire
{{ 'Hello, ' + user.name }}
```

`??` and `or` look similar but answer different questions, and mixing
them up is the single most common Wire mistake:

- `??` asks "was this ever supplied?" and only falls back on `nil`.
- `or` asks "is this worth showing?" using [Wire's own
  truthiness](#truthiness), and falls back on anything falsy — `nil`,
  `false`, an empty string, an empty collection, or the number `0`.

```wire
{{ discount ?? 0 }}   {{-- a missing discount becomes 0 --}}
{{ discount or 0 }}   {{-- a discount that IS 0 also becomes 0, harmlessly here --}}

{{ stock_count ?? 'unknown' }}  {{-- 0 in stock still shows as 0 --}}
{{ stock_count or 'unknown' }}  {{-- 0 in stock is treated the same as never supplied --}}
```

Use `??` whenever zero is a legitimate value you want to keep, and
`or` whenever you only want to show something when there is genuinely
something to show.

### Truthiness

`x-if`, `x-not`, `and`/`or`, `!`/`not`, and `? :` all use Wire's own
notion of truthy and falsy, which is not quite the same as Zuri's own
rules and is deliberately friendlier for template authoring:

| Value                     | Wire        |
| ------------------------- | ----------- |
| `nil`, `false`            | falsy       |
| `0`                       | falsy       |
| any other number, **including negative ones** | truthy |
| an empty string           | falsy       |
| a non-empty string        | truthy      |
| an empty list or dict     | falsy       |
| a non-empty list or dict  | truthy      |
| anything else             | truthy      |

The one difference worth calling out explicitly: a negative number is
truthy in Wire. A temperature of `-5` or an account balance of `-1`
should still show up in the page; a template engine that treated any
negative number as "nothing to show" would be a constant source of
subtle bugs. An empty collection, on the other hand, is falsy — so
`x-if="results"` correctly hides a section for a search that came back
with nothing.

### Calling Functions

A value [registered as a global](#globals) can be called directly:

```wire
<a href="{{ route('user.profile', user.id) }}">{{ user.name }}</a>
```

There is also an older, standalone spelling for calling a function
that takes no arguments, kept from Wire's very first version because
it reads well on its own:

```wire
{! current_year !}
```

is exactly the same as writing:

```wire
{{ current_year() }}
```

Prefer `{{ fn() }}` for anything new; `{! !}` exists for templates that
already use it and for the handful of cases — a page's build stamp, a
feature flag — where a function taking nothing at all reads a little
cleaner without the parentheses.

## Filters

A filter transforms the value on the left of a `|`. It is the same
idea as a Unix pipe:

```wire
{{ name|upper }}
{{ price|round(2) }}
{{ post.body|truncate(150) }}
```

The value being filtered is always the filter's first argument;
anything written in parentheses follows it. `truncate(150)` above
calls the `truncate` filter as `truncate(post.body, 150)`.

### Chaining Filters

Filters read left to right, each one's output feeding the next:

```wire
{{ name|trim|title }}
{{ comment.body|strip_tags|truncate(200) }}
```

### The `=` Argument Shorthand

For a filter that takes exactly one argument, `name=value` is
shorthand for `name(value)`:

```wire
{{ status|is='active' }}
```

is the same as:

```wire
{{ status|is('active') }}
```

This spelling exists for symmetry with Wire's very first version and
reads naturally for a short comparison; the parenthesised form is
equally valid everywhere and is the only option once a filter needs
more than one argument.

### Available Filters

**Escaping**

| Filter | What it does |
| --- | --- |
| `raw` | [Marks the value as markup](#rendering-raw-markup), skipping escaping entirely. |
| `escape` / `e` | Escapes for a context other than the one the value is being written into. Takes `'text'` (the default), `'attribute'`, `'url'`, `'script'`, or `'style'`. |

**Text**

| Filter | What it does |
| --- | --- |
| `upper` | Converts to upper case. |
| `lower` | Converts to lower case. |
| `title` | Title Cases Every Word. |
| `capitalize` | Capitalizes only the first letter, leaving the rest alone. |
| `trim` | Removes leading and trailing whitespace of every kind (space, tab, newline). |
| `truncate(length, suffix?)` | Cuts to `length` characters, appending `suffix` (default `'…'`) only if anything was actually cut. |
| `replace(search, replacement?)` | Replaces every literal occurrence of `search`. Never a regular expression. `replacement` defaults to an empty string. |
| `lpad(width, fill?)` | Pads on the left to `width` characters, with `fill` (default a space). |
| `rpad(width, fill?)` | Pads on the right. |
| `repeat(count)` | Repeats the value `count` times. |
| `nl2br` | Turns line breaks into `<br>`, escaping the text first. Returns markup. |
| `line_breaks` | An alias for `nl2br`. |
| `strip_tags` | Removes every HTML tag, keeping only the text — a real parse, not a pattern match. |
| `slug` | Lower-cased, hyphen-separated, safe for a URL segment. |
| `url_encode` | Percent-encodes for use inside a URL. |
| `json` | Encodes as JSON text. |
| `json_script(id?)` | Wraps the value's JSON encoding in a `<script type="application/json">`, optionally with an `id`, ready to be read back by a script on the page. Returns markup. |

**Numbers**

| Filter | What it does |
| --- | --- |
| `abs` | Removes the sign. |
| `round(places?)` | Rounds to `places` decimal places (default `0`), halves rounding away from zero. |
| `floor` | Rounds down to the nearest whole number. |
| `ceil` | Rounds up. |
| `number_format(places?, point?, separator?)` | Groups thousands and fixes the decimal places, like `1,234,567.89`. Pass `point`/`separator` to use another convention, e.g. `1.234.567,89`. |
| `filesize(binary?)` | A byte count written the way a person reads it — `1.4 MB` by default, or `1.3 MiB` with `binary` set to `true`. |

**Collections**

| Filter | What it does |
| --- | --- |
| `length` | How many entries — works on a string, list, dict, or bytes. `nil` has length `0`. |
| `first` | The first entry, or `nil` if there is none. |
| `last` | The last entry. |
| `join(glue?)` | Joins entries into one string with `glue` (default `''`) between them. |
| `sort(key?)` | Sorted ascending; `key` sorts a list of dicts or instances by one field. |
| `reverse` | The entries backwards, or a string reversed. |
| `unique` | Duplicates removed, keeping the first of each. |
| `keys` | A dict's keys, in insertion order. |
| `values` | A dict's values, in insertion order. |
| `slice(start, end?)` | The entries from `start` up to but not including `end`. Negative positions count from the end. |
| `sum(key?)` | Adds the entries together; `key` sums one field of a list of dicts or instances. |
| `split(separator?)` | Splits a string into a list. `separator` defaults to any run of whitespace. |

**Choice**

| Filter | What it does |
| --- | --- |
| `default(fallback)` / `alt` | `fallback` when the value is [falsy](#truthiness), otherwise the value. |
| `empty` | Whether the value has nothing in it. Unlike falsiness, a number is never empty — not even `0`. |
| `is(expected)` | Whether the value equals `expected`. |
| `not(expected)` | Whether it differs. |

**Dates**

| Filter | What it does |
| --- | --- |
| `date(format?)` | Formats a `date.Date`, a Unix timestamp, or a parseable date string, using [the same format directives as `Date.format()`](/docs/date.md). Defaults to `'Y-m-d H:i:s'`. |

```wire
<time datetime="{{ post.published_at|date('Y-m-d') }}">
  {{ post.published_at|date('jS F Y') }}
</time>
```

### Writing Your Own Filter

See [Custom Filters](#custom-filters) below.

## Conditionals

### `x-if`, `x-elif`, and `x-else`

`x-if` renders an element, and everything inside it, only when its
expression is [truthy](#truthiness):

```wire
<p x-if="user.is_admin">You have administrator access.</p>
```

If `user.is_admin` is falsy, the whole `<p>` — tag and contents — is
left out of the page entirely. There is no empty element left behind.

Chain further conditions with `x-elif`, and close the chain with a
plain `x-else`:

```wire
<p x-if="user.role == 'admin'">Administrator</p>
<p x-elif="user.role == 'staff'">Staff member</p>
<p x-elif="user.role == 'contributor'">Contributor</p>
<p x-else>Member</p>
```

Exactly one of these renders. Only whitespace and HTML comments are
allowed between the elements in a chain — any real content in between
ends it, and an `x-elif` or `x-else` with no `x-if` in front of it is
rejected when the template compiles, not silently ignored:

```wire
{{-- this chain is broken by the text between the two elements --}}
<p x-if="a">A</p>
some text
<p x-else>not A</p> {{-- error: x-else has no x-if in front of it --}}
```

### `x-not`

`x-not` is the plain inverse of `x-if` — it renders when its expression
is falsy — and does not take part in a chain:

```wire
<div x-not="user.has_verified_email">
  <p>Please verify your email address.</p>
</div>
```

`x-if="!condition"` and `x-not="condition"` mean the same thing;
`x-not` exists because it often reads more naturally for a guard
clause.

## Loops

`x-for` repeats an element once per entry of whatever its expression
evaluates to — a list, a dict, a string, or a range:

```wire
<ul>
  <li x-for="posts" x-value="post">{{ post.title }}</li>
</ul>
```

Notice that the *element itself* repeats, not just its contents — the
example above produces one whole `<li>` per post, not one `<li>`
wrapping every post. `x-value` names the variable each iteration binds
its current entry to; leave it out entirely if you do not need to
refer to the entry by name.

An optional `x-key` binds the position (for a list) or the key (for a
dict):

```wire
<tr x-for="users" x-key="id" x-value="user">
  <td>{{ id }}</td>
  <td>{{ user.name }}</td>
</tr>
```

Every kind of collection iterates naturally:

```wire
<li x-for="tags" x-value="tag">{{ tag }}</li>              {{-- a list --}}
<li x-for="scores" x-key="name" x-value="score">           {{-- a dict --}}
  {{ name }}: {{ score }}
</li>
<span x-for="word" x-value="letter">{{ letter }}</span>    {{-- a string, by character --}}
<option x-for="1..5" x-value="n">{{ n }}</option>          {{-- a range --}}
```

A missing or empty collection simply renders nothing — there is no
need to guard a loop with an `x-if` first:

```wire
<li x-for="comments" x-value="comment">{{ comment.body }}</li>
{{-- renders nothing at all if `comments` is empty or was never supplied --}}
```

### The `loop` Variable

Every iteration publishes a `loop` variable with the following fields:

| Field | Value |
| --- | --- |
| `loop.index` | The position, counting from `1`. |
| `loop.index0` | The position, counting from `0`. |
| `loop.first` | `true` on the first pass. |
| `loop.last` | `true` on the last pass. |
| `loop.length` | How many entries there are in total. |
| `loop.even` | `true` on the 2nd, 4th, 6th, … pass — even-numbered by `loop.index`. |
| `loop.odd` | `true` on the 1st, 3rd, 5th, … pass. |
| `loop.key` | The current key or index, whether or not `x-key` binds it too. |
| `loop.value` | The current value, whether or not `x-value` binds it too. |
| `loop.parent` | The enclosing loop's own `loop`, for a nested `x-for`. |

```wire
<tr x-for="rows" x-value="row" x-attr="{ 'class': loop.odd ? 'zebra' : nil }">
  <td>{{ loop.index }}</td>
  <td>{{ row.name }}</td>
</tr>
```

### Nesting Loops

`x-loop` renames the metadata variable a specific `x-for` publishes,
which is what lets an inner loop's own `loop` and an outer loop's
`loop` both be reached at once:

```wire
<table x-for="rows" x-loop="row" x-value="cells">
  <tr>
    <td x-for="cells" x-value="cell">
      {{ row.index }}, {{ loop.index }}: {{ cell }}
    </td>
  </tr>
</table>
```

Without `x-loop`, the inner loop's own `loop` would simply shadow the
outer one for the scope of the inner loop — reach for `loop.parent`
instead if you would rather not rename anything:

```wire
<td x-for="cells" x-value="cell">
  outer position {{ loop.parent.index }}, inner position {{ loop.index }}
</td>
```

### Looping Without a Wrapper Element

Sometimes you want to repeat several elements together without a real
element wrapping them, or without introducing any element at all. Put
the directive on a `<template>` instead of on the element you want
repeated:

```wire
<select>
  <template x-for="countries" x-value="country">
    <option value="{{ country.code }}">{{ country.name }}</option>
  </template>
</select>
```

`<template>` is the one HTML element the parser lets through
completely untouched, wherever it appears — inside a `<head>`, inside
a `<table>`, inside a `<select>` — and Wire removes it from the output
entirely, leaving only what was inside it, once per pass. This is also
the way to apply [`x-if`](#conditionals) to a group of elements without
picking one of them to carry the attribute:

```wire
<template x-if="user.is_admin">
  <a href="/admin">Dashboard</a>
  <a href="/admin/users">Users</a>
</template>
```

## Content and Attributes

### `x-text`

`x-text` replaces an element's children with its expression, escaped
as plain text — useful when the element already has other attributes
and you would rather not write the value twice:

```wire
<p x-text="post.summary"></p>
```

is the same as:

```wire
<p>{{ post.summary }}</p>
```

Anything written inside the element in the source is discarded; it
exists only to describe what would go there without JavaScript.

### `x-html`

`x-html` is `x-text`'s unescaped counterpart — it replaces the
element's children with its expression's value, written as markup
rather than text:

```wire
<div x-html="post.rendered_body|raw"></div>
```

Note the `|raw`: `x-html` still expects a value marked safe, the same
as an ordinary interpolation would. `x-html` only changes *where* the
markup goes (replacing the whole element's contents, rather than
sitting inline in a text run) — it does not, on its own, turn escaping
off. The [same warning about untrusted input](#rendering-raw-markup)
applies here just as much as it does to the `raw` filter.

### `x-attr`

`x-attr` spreads a dictionary of names and values onto the element as
attributes:

```wire
<input x-attr="{ type: 'text', name: field.name, required: field.is_required }">
```

Within that dictionary:

- `true` produces a valueless attribute (`required`), exactly as
  writing `required` by hand would.
- `false` and `nil` leave the attribute off entirely, rather than
  writing it with an empty or literal `"false"` value.
- Anything else is written as the attribute's value, escaped for
  whatever that attribute means (a URL attribute is checked as a URL,
  same as always).

This is what turns a boolean into a real HTML boolean attribute
without a ternary in every place one is needed:

```wire
<button x-attr="{ disabled: !form.is_valid }">Submit</button>
```

A computed attribute takes over from one written directly on the same
element, so you can set a sensible default and only override it when
there is something to override:

```wire
<div class="card" x-attr="{ 'class': featured ? 'card card-featured' : nil }">
```

## Comments

HTML's own comment syntax is a server-side note in a Wire template. It
never reaches the rendered page, and nothing inside one is evaluated —
which is exactly what makes it safe to leave notes for other people
maintaining the template, without those notes shipping to a browser:

```wire
<!-- TODO: replace this hard-coded banner once marketing sends the real copy -->
<div class="banner">Coming soon</div>

<!-- this variable is not evaluated: {{ some.internal.detail }} -->
```

If you genuinely want a comment to reach the browser — a conditional
comment, a build stamp a deploy script checks for — turn comments back
on for that `Wire` instance with [`set_comments(true)`](#configuration).

## Including Templates

`<include path="..." />` renders another template in its place. It
also has a directive spelling, `<template x-include="...">`, which
means exactly the same thing — the pseudo-element forms in this guide
(`<include>`, `<extend>`, `<declare>`, `<define>`) are sugar, rewritten
into the directive form before the template is even parsed, which is
what lets them work correctly inside a `<head>` or a `<table>` where an
element the parser does not recognise would otherwise be moved or
dropped.

```wire
<include path="partials/nav.html" />
```

is the same as:

```wire
<template x-include="partials/nav.html"></template>
```

The path is resolved [inside the template root](#the-template-root),
the same as `render()`'s own path argument.

By default, an include sees every variable the page around it can
see — it is not a separate scope:

```wire
{{-- page.html --}}
<include path="partials/greeting.html" />
```

```wire
{{-- partials/greeting.html --}}
<p>Hi, {{ user.name }}!</p>
```

renders correctly without `user` ever being passed explicitly to the
partial.

### Passing Data to an Include

`x-with` adds variables for the included template, alongside whatever
it already inherits:

```wire
<include path="partials/badge.html" x-with="{ label: 'New', tone: 'green' }" />
```

`only` withholds the surrounding scope entirely, leaving the partial
with only what `x-with` gave it — turning a plain partial into
something closer to a real component with a defined interface:

```wire
<include path="components/price-tag.html" x-with="{ amount: item.price }" only />
```

### Includes as Components

Anything written *inside* an `<include>` tag is handed to the included
template as a named region called `content`, declared with
`<declare name="content">`:

```wire
{{-- components/card.html --}}
<section class="card">
  <h3>{{ title }}</h3>
  <declare name="content"></declare>
</section>
```

```wire
{{-- the page --}}
<include path="components/card.html" x-with="{ title: 'Recent Orders' }">
  <p>{{ orders|length }} orders this week</p>
</include>
```

renders as:

```html
<section class="card">
  <h3>Recent Orders</h3>
  <p>3 orders this week</p>
</section>
```

The content you write inside the `<include>` tag keeps the scope it
was written in — the page's, not the partial's — which is exactly what
lets `{{ orders|length }}` above read a variable from the page even
though `x-only` was not used and the card component never mentions
`orders` at all.

### Computed Include Paths

A path is read as literal text, with `{{ }}` interpolation allowed
inside it — not as an expression in its own right, so
`x-include="header"` names a file called `header`, rather than reading
a variable of that name:

```wire
<include path="themes/{{ current_theme }}/header.html" />
```

resolves a different file per render depending on `current_theme`,
while everything after `path="themes/"` up to the next `{{` is taken
literally.

## Template Inheritance

Includes are for small, reusable fragments — a navbar, a footer, a
badge. For whole-page structure — the parts of a layout every page on
your site shares — template inheritance is the better fit. Where an
include composes fragments together, inheritance lets a base layout
define the *shape* of a page once, and lets each page fill in only the
parts that differ.

### Defining a Layout

A base template marks the regions a page is allowed to fill in with
`<declare name="...">`:

```wire
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

Notice that `footer` has content already inside its `<declare>` tag.
That becomes [the default](#default-slot-content) — what renders when
a page does not define that region at all.

### Extending a Layout

A page declares which layout it extends with `<extend base="...">`,
and fills in regions with `<define name="...">`:

```wire
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

Rendering `pages/dashboard.html` produces the layout's full structure,
with `head` and `content` replaced by what the page defined, and
`footer` left exactly as the layout's own default. Both the layout and
the page are rendered against the same variables `render()` was given
— there is no separate scope to pass anything through.

Everything inside an `<extend>` must be inside a `<define>`. The base
template owns the page's structure; a page's job is only to fill in
the regions it was offered, not to add markup of its own outside them.
A template can extend exactly one base.

### Default Slot Content

A region a page does not define keeps whatever the layout put inside
its own `<declare>` tag:

```wire
{{-- pages/minimal.html --}}
<extend base="layouts/app.html">
  <define name="content">
    <p>Just this page's content — head and footer both use the layout's defaults.</p>
  </define>
</extend>
```

A `<declare>` tag with nothing inside it — like `head` in the layout
above — simply renders empty when nothing defines it.

### `x-super`

`<super />` — or its directive spelling, `<template x-super></template>`
— renders whatever the region it sits inside would have shown before
this definition replaced it. That lets a page *add to* a section
instead of fully restating it:

```wire
<extend base="layouts/app.html">
  <define name="footer">
    <super />
    <p><a href="/privacy">Privacy Policy</a></p>
  </define>
</extend>
```

renders the layout's own copyright line, followed by the extra privacy
link — without the page having to know or repeat what the layout's
default footer actually said.

### Multi-Level Inheritance

A template that extends a base can itself declare regions of its own,
letting a further template extend *it*. This is how a site with, say,
a general layout and several page-type-specific layouts (a blog post,
a product page) is usually structured:

```wire
{{-- layouts/app.html --}}
<html><body>
  <declare name="body"><p>default</p></declare>
</body></html>
```

```wire
{{-- layouts/article.html --}}
<extend base="layouts/app.html">
  <define name="body">
    <article>
      <declare name="article-content"></declare>
    </article>
  </define>
</extend>
```

```wire
{{-- pages/post.html --}}
<extend base="layouts/article.html">
  <define name="article-content">
    <h1>{{ post.title }}</h1>
    {{ post.body|raw }}
  </define>
</extend>
```

Rendering `pages/post.html` walks the whole chain: it extends
`layouts/article.html`, which extends `layouts/app.html`, and the
final page is assembled from all three.

### Overriding a Definition

Defining the same region twice in one template is almost always an
accident — two people editing the same file, a copy-paste that was
never cleaned up — so Wire refuses it at compile time unless you say
the replacement is deliberate with `override`:

```wire
<extend base="layouts/app.html">
  <define name="content"><p>First draft</p></define>
  <define name="content" override><p>Final version</p></define>
</extend>
```

Without `override` on the second one, this template fails to compile
with a clear message naming the region and both locations, rather than
silently keeping whichever definition happened to come last.

## Security

### Why Everything Is Escaped by Default

A templating engine's job includes keeping a value that came from
outside your program — a form submission, a query string, another
user's profile — from becoming markup, a script, or a link, unless you
explicitly say it is safe to. Wire treats this as the default rather
than an opt-in setting, for the same reason a seatbelt only works if
putting it on is the ordinary thing to do rather than the thing you
remember to do under pressure: the moment escaping is something you
have to *remember* to turn on, the templates that skip it are the ones
that get exploited.

Every interpolation is escaped for the exact place it lands — see the
[table earlier in this guide](#escaping-data) — and there is no
template-level setting to disable that. The only way past it is the
explicit, visible [`raw` filter or `x-html`
directive](#rendering-raw-markup), which is exactly the friction you
want between "displaying a value" and "trusting a value with
unescaped markup."

### URLs Are Checked, Not Just Escaped

An attribute a browser reads as a URL — `href`, `src`, `action`, and
several others — gets more than entity escaping. Its scheme is checked
against an allowlist, and a disallowed scheme is replaced with
`about:blank` rather than written through:

```wire
<a href="{{ profile_link }}">{{ user.name }}</a>
```

If `profile_link` were `javascript:alert(document.cookie)`, the
rendered `href` is `about:blank`, not the script URL. A URL with no
scheme at all — every relative link, every absolute path, every
protocol-relative URL — is always allowed, since none of those can
name a handler in the first place.

The default allowlist covers `http`, `https`, `mailto`, `tel`, and a
handful of others; it deliberately leaves out `javascript`,
`vbscript`, `data`, and `file`. [`set_url_schemes()`](#configuration)
replaces the list for an application that genuinely needs another
scheme — a custom `app:` handler, say.

`srcset` and `ping`, which each hold a *list* of URLs, have every entry
in the list checked the same way, with descriptors (`2x`, `640w`) and
separators preserved exactly as written.

### Scripts and Stylesheets

A value interpolated inside a `<script>` element, or inside an event
handler attribute like `onclick`, is encoded as JSON rather than
merely escaped — automatically, with no filter needed:

```wire
<script>
  var currentUser = {{ user }};
</script>
```

renders the whole `user` value as a JSON literal, with its own quotes
included. That is worth internalising, because it changes how you
write the surrounding script: **do not** wrap the interpolation in
your own quotes.

```wire
{{-- correct: renders   var name = "Ada";   --}}
<script>var name = {{ user.name }};</script>

{{-- wrong: renders   var name = '"Ada"';   which is not what you meant --}}
<script>var name = '{{ user.name }}';</script>
```

A value inside a `<style>` block has anything outside a conservative,
CSS-safe character set dropped rather than escaped — there is no
escape sequence that would stop a `<` from being read by the HTML
tokenizer looking for `</style>`, so the only sound answer is for the
character not to be there at all.

### The Template Root Is a Sandbox

Every path — whether it came from `render()`'s own argument, or from
an `x-include`/`x-extend` written inside a template, even one built
from an interpolated value — is resolved *inside* the configured root
and refused if it resolves anywhere else, `..` segments and symbolic
links both included:

```wire
<include path="{{ theme }}/header.html" />
```

If `theme` ever came from something a request controls, an unbounded
loader would turn this into a way to read arbitrary files the process
can reach. Wire's loader treats the root as a hard boundary instead:
the worst a hostile value can do here is fail to find a template.

## Extending Wire

### Custom Filters

`register_filter()` adds a filter, or replaces one that already
exists by that name. The value being filtered is always the first
argument; anything the template passes after it follows:

```zuri
view.register_filter('excerpt', @(value, words) {
  var count = words ?? 25
  return ' '.join(value.split('/\\s+/').take(count)) + '…'
})
```

```wire
<p>{{ post.body|excerpt(40) }}</p>
```

An argument the template leaves out arrives as `nil`, which is why
`words ?? 25` above supplies a sensible default rather than the
filter insisting the template spell it out every time.

A filter returning a plain value has that value escaped like any
other — the same as an ordinary interpolation. A filter that genuinely
produces markup returns [`wire.safe()`](#rendering-raw-markup), and
from that point on is responsible for what is inside it, exactly like
`raw` and `x-html` are.

### Globals

`register_global()` makes a value readable from every template
without it being passed to `render()` explicitly:

```zuri
view.register_global('site_name', 'Example Inc.')
view.register_global('route', @(name) {
  return '/' + name
})
```

```wire
<title>{{ page_title }} — {{ site_name }}</title>
<a href="{{ route('user.profile', user.id) }}">{{ user.name }}</a>
```

A variable of the same name passed to `render()` wins over a global —
a global is a default, not a hard override, so a page can still shadow
`site_name` for itself if it ever needs to.

### Custom Elements

For the rare case the directives genuinely cannot express,
`register_element()` claims an HTML tag name outright and hands every
element of that name to a Zuri function instead of writing it out:

```zuri
view.register_element('icon', @(view, element) {
  var name = element.attributes.get('name', 'dot')
  return wire.safe('<svg class="icon"><use href="#${name}"></use></svg>')
})
```

```wire
<icon name="star" />
```

The function receives the `Wire` instance and a dictionary describing
the element — its tag name, its attributes (already rendered and
escaped), its already-rendered children, and the scope it was written
in. Return `wire.safe(markup)` to write markup, any other value to
write it as escaped text, or `nil` to write nothing at all. Directives
still work on a claimed element exactly as they read
(`<icon x-if="..." />` behaves the way it looks), and the whole
mechanism exists for genuinely dynamic, code-driven markup that a
partial and `x-include` cannot express — reach for an include first;
it is easier for the next person to read and does not require them to
go find the Zuri function behind it.

## Configuration

Every setting can be passed to the `Wire` constructor at once, or set
individually with its own method afterward:

```zuri
var view = wire.wire({
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
| `root` / `set_root(path)` | `./templates` | The directory every template path resolves inside. |
| `extension` / `set_extension(ext)` | `.html` | Tried when a path names no file as written. Must start with `.`. |
| `compact` / `set_compact(bool)` | `false` | Drops whitespace-only text between tags. |
| `comments` / `set_comments(bool)` | `false` | Whether HTML comments reach the rendered output. |
| `auto_reload` / `set_auto_reload(bool)` | `true` | Whether a cached template is checked against its file, and re-read if it changed, before each render. |
| `url_schemes` / `set_url_schemes(list)` | see [above](#urls-are-checked-not-just-escaped) | Which URL schemes an `href`/`src`/etc. is allowed to use. |

`root()` reports the currently configured root as an absolute path,
and `create_root()` makes the directory if it does not exist yet
(returning whether it had to).

## Compiling and Caching

`render()` compiles a template into its instruction tree the first
time it is used, and keeps the compiled result for next time — parsing
and directive resolution happen once per template, not once per
request. Every subsequent render just walks that tree.

With `auto_reload` on (the default), each render checks the file's
modification time and size against what was cached, and recompiles if
either changed. That is one filesystem `stat` per template per
render — cheap, and exactly what you want while actively editing
templates. Turn it off with `set_auto_reload(false)` once you are
running under load and are not editing templates live; `clear_cache()`
is then how a long-running process picks up a new deployment.

`compile(path)` compiles a template without rendering it, and returns
the compiled result — or raises, if the template has a mistake in it.
That makes it a good fit for a startup-time check across a whole
directory of templates, so a broken one is caught before the first
request that would have hit it:

```zuri
for name in os.read_dir('./views', true) {
  view.compile(name)
}
```

## Error Handling

Everything Wire raises is a `wire.WireError`, and every one of them
carries the template path and the line/column of the tag responsible,
so catching the base class is enough to handle any template failure
the same way — turning it into a 500 page, logging it with context,
whatever your application needs:

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

Three more specific errors, each a `WireError`, tell you *what kind*
of thing went wrong:

| Error | Raised when |
| --- | --- |
| `wire.TemplateSyntaxError` | A template does not compile: an unknown directive, a malformed expression, a broken inheritance chain. Always caught the first time a template is used, not on a later render. |
| `wire.TemplateNotFoundError` | A path names no file, or resolves outside the template root. |
| `wire.RenderError` | Something that depends on the values a template was given: iterating something that cannot be iterated, a filter rejecting its input, an include chain that never bottoms out. |

A variable that was simply never supplied is *not* one of these —
that renders as empty and tests as falsy, by design, so an optional
section can be written without a guard around every single field it
touches.

## Full Directive Reference

Every directive Wire understands. An `x-` prefixed attribute outside
this list fails to compile — Wire owns that whole prefix, so a typo
like `x-fi` is caught the moment the template is compiled rather than
silently doing nothing.

| Directive | Takes | Meaning |
| --- | --- | --- |
| `x-if` | expression | Renders only if truthy. Starts a chain. |
| `x-elif` | expression | Continues an `x-if` chain. |
| `x-else` | *(none)* | Closes an `x-if` chain. |
| `x-not` | expression | Renders only if falsy. Does not chain. |
| `x-for` | expression | Repeats the element once per entry. |
| `x-value` | name | Binds each iteration's value. |
| `x-key` | name | Binds each iteration's key/index. |
| `x-loop` | name | Renames the loop metadata variable. |
| `x-text` | expression | Replaces children with escaped text. |
| `x-html` | expression | Replaces children with unescaped markup. |
| `x-attr` | expression (dict) | Spreads attributes onto the element. |
| `x-include` | path | Renders another template in this element's place. |
| `x-with` | expression (dict) | Adds variables for an include. |
| `x-only` | *(none)* | Withholds the surrounding scope from an include. |
| `x-extend` | path | This template extends the named base. |
| `x-slot` | name | Declares a region an extending template may replace. |
| `x-define` | name | Replaces a base template's region. |
| `x-override` | *(none)* | Permits redefining a region already defined once. |
| `x-super` | *(none)* | Renders the definition this one replaces. |

Five pseudo-elements are sugar for the directives above, rewritten
before the template is parsed:

| Pseudo-element | Equivalent to |
| --- | --- |
| `<include path="p" />` | `<template x-include="p"></template>` |
| `<extend base="p">…</extend>` | `<template x-extend="p">…</template>` |
| `<declare name="n">…</declare>` | `<template x-slot="n">…</template>` |
| `<define name="n">…</define>` | `<template x-define="n">…</template>` |
| `<super />` | `<template x-super></template>` |

## Cheat Sheet

```wire
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

Further reading: the [`html` module](/docs/html.md) that Wire's parser
and serializer are built on, and the [`date` module](/docs/date.md)
for the format directives the `date` filter accepts.
