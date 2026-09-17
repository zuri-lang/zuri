# Mail

The `mail` module is Zuri's mail stack: the message format, the three
protocols that move messages around, and the servers at the far end of
two of them. It is written in Zuri from the socket up, and it speaks to
real mail servers.

Mail is older than almost everything it runs on, and it shows. A
message is text with a header block, defined in 1982 and extended ever
since to carry anything that is not ASCII, which by now is most of what
people send. Sending one is SMTP. Reading one where it is kept is IMAP.
Taking one away is POP3. Three protocols, one format, and a great deal
of accumulated history, almost all of which a program should not have
to know about.

That is the design. A message is described by what is in it and the
module works out the rest: which parts it needs, how they nest, which
encoding each header and each body wants, and what has to be escaped so
that a line of a single dot in the body does not end the message early.
A program says who a message is from and what it says; it does not say
`Content-Transfer-Encoding`.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Building a Message](#building-a-message)
  - [Addresses](#addresses)
  - [Text, HTML, or Both](#text-html-or-both)
  - [Attachments](#attachments)
  - [Images Inside the HTML](#images-inside-the-html)
  - [Headers of Your Own](#headers-of-your-own)
  - [Replies and Threads](#replies-and-threads)
- [Reading a Message](#reading-a-message)
  - [The Tree](#the-tree)
  - [Finding the Body](#finding-the-body)
  - [Attachments Coming In](#attachments-coming-in)
  - [Character Sets](#character-sets)
- [Sending](#sending)
  - [A Client of Your Own](#a-client-of-your-own)
  - [The Envelope](#the-envelope)
  - [TLS](#tls)
  - [Authenticating](#authenticating)
  - [What the Server Can Do](#what-the-server-can-do)
  - [When Sending Fails](#when-sending-fails)
- [Reading Mail with IMAP](#reading-mail-with-imap)
  - [Opening a Mailbox](#opening-a-mailbox)
  - [Searching](#searching)
  - [Fetching Less Than Everything](#fetching-less-than-everything)
  - [Flags](#flags)
  - [Moving, Copying and Removing](#moving-copying-and-removing)
  - [Putting a Message Back](#putting-a-message-back)
  - [Waiting for New Mail](#waiting-for-new-mail)
- [Collecting Mail with POP3](#collecting-mail-with-pop3)
- [Running an SMTP Server](#running-an-smtp-server)
  - [The Handlers](#the-handlers)
  - [Refusing Properly](#refusing-properly)
  - [TLS and Authentication](#tls-and-authentication)
  - [Limits](#limits)
- [Running an IMAP Server](#running-an-imap-server)
  - [Where the Mail Lives](#where-the-mail-lives)
  - [Maildir](#maildir)
  - [A Store of Your Own](#a-store-of-your-own)
- [Serving More Than One Connection](#serving-more-than-one-connection)
- [Proving Where Mail Came From](#proving-where-mail-came-from)
  - [Signing](#signing)
  - [Checking](#checking)
  - [What a Signature Does Not Say](#what-a-signature-does-not-say)
- [Authentication Mechanisms](#authentication-mechanisms)
- [Errors](#errors)
- [What the Module Refuses](#what-the-module-refuses)
- [Module Reference](#module-reference)

## Following Along

Most examples below build on one message. This is it:

```zuri
import mail

var note = mail.message()
  .set_from('Ada Lovelace <ada@example.com>')
  .add_to('Grace Hopper <grace@example.com>')
  .set_subject('On Engines')
  .set_text('The analytical engine weaves algebraic patterns.')

echo note.subject()
echo note.sender().name
echo note.to()[0].address
```

```console
On Engines
Ada Lovelace
grace@example.com
```

Nothing there touches a network. Building and reading messages is
entirely separate from moving them, and a program that only needs to
produce or parse one never opens a socket.

## Introduction

There are three protocols and they do genuinely different things.

**SMTP** moves a message from wherever it was written to a server that
will take responsibility for it. That is all it does. It has no notion
of a folder, a read message, or a search; a message goes in and either
is accepted or is not.

**IMAP** is for reading mail where it is kept. The mail stays on the
server, the client works with it in place, and two devices looking at
the same mailbox see the same thing. Almost everything a mail client
does is IMAP.

**POP3** is for taking mail away. There is a numbered list and there is
removing things from it. It is the wrong protocol for reading mail on
more than one device and the right one for a program whose job is to
drain a mailbox into somewhere else.

The module covers the client end of all three and the server end of the
two that have one worth having. There is no POP3 server here and there
is not meant to be: a POP3 server is an IMAP server with almost
everything taken away, and `mail.imap` is the one to run.

## Building a Message

`mail.message()` starts one. What goes in it is said one call at a
time, and each call hands the message back, so they read as one:

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_subject('On Engines')
  .set_text('The analytical engine weaves algebraic patterns.')

echo note.content_type().mime_type()
```

```console
text/plain
```

A message starts with a `Date`, because the time it was written is the
time it was written, and with `MIME-Version`. It gets its `Message-ID`
when it is sent, because that is when the domain it belongs to is
settled.

### Addresses

Every address setter takes text, an `Address`, or a list of either:

```zuri
import mail

var note = mail.message()
  .set_from('Ada Lovelace <ada@example.com>')
  .add_to(['grace@example.com', 'Charles Babbage <charles@example.com>'])
  .add_cc('archive@example.com')

echo note.to().map(@(person) => person.address)
echo note.to()[1].name
echo note.cc().length()
```

```console
[grace@example.com, charles@example.com]
Charles Babbage
1
```

`add_to()` keeps whoever is already there, so it can be called in a
loop. `set_to()` replaces them. The same pair exists for `Cc` and
`Bcc`.

`Bcc` is removed from the message before it is handed to a server,
after the recipients have been taken out of it. That is the whole point
of a blind copy, and it is easy to get wrong by hand.

A display name with characters a header cannot carry is encoded, and
one containing a comma is quoted, without being asked:

```zuri
import mail

echo mail.Address('gruss@example.com', 'Grüße').to_string()
echo mail.Address('ada@example.com', 'Lovelace, Ada').to_string()
```

```console
=?utf-8?B?R3LDvMOfZQ==?= <gruss@example.com>
"Lovelace, Ada" <ada@example.com>
```

Reading them back gives the name, not the encoding:

```zuri
import mail

echo mail.parse_address('=?utf-8?B?R3LDvMOfZQ==?= <gruss@example.com>').name
echo mail.parse_address('"Lovelace, Ada" <ada@example.com>').name
```

```console
Grüße
Lovelace, Ada
```

`mail.parse_address_list()` reads a whole header, including the
comments and groups that RFC 5322 allows and nobody remembers:

```zuri
import mail

var people = mail.parse_address_list(
  'Ada <ada@example.com> (the author), Engineers: grace@example.com, charles@example.com;'
)

echo people.map(@(person) => person.address)
```

```console
[ada@example.com, grace@example.com, charles@example.com]
```

The group's members come back alongside the rest, since a program
sending mail cares who the recipients are and not how they were
gathered. `mail.parse_address_groups()` keeps the grouping when that
matters.

### Text, HTML, or Both

Text alone is a plain message. HTML alone is an HTML message. Both
together become a `multipart/alternative`, with the plain text first,
because a reader that understands both is meant to take the last one it
can display:

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_subject('On Engines')
  .set_text('The engine weaves algebraic patterns.')
  .set_html('<p>The engine weaves <em>algebraic patterns</em>.</p>')

echo note.content_type().mime_type()
echo note.parts().map(@(part) => part.content_type().mime_type())
```

```console
multipart/alternative
[text/plain, text/html]
```

Setting the text again replaces it wherever in the tree it is, rather
than adding a second copy. The shape is a consequence of the content,
so it stays correct as the content changes.

Outgoing text is written as UTF-8, or as US-ASCII when that is all it
needs. Naming any other character set raises, because the module cannot
encode into one and a header claiming otherwise would be a lie.

### Attachments

An attachment is a thing in its own right, built the way a message is
and handed to `attach()`:

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_subject('The figures')
  .set_text('Attached.')
  .attach(mail.attachment('name,total\nengines,41\n')
    .set_filename('figures.csv'))

echo note.content_type().mime_type()
echo note.attachments().map(@(part) => part.filename())
echo note.attachments()[0].content_type().mime_type()
```

```console
multipart/mixed
[figures.csv]
text/csv
```

The media type came from the filename. The message became a
`multipart/mixed` with whatever it held before as the first part; that
happens once however many files are attached.

| | |
| --- | --- |
| `set_filename(name)` | the name to offer the file under |
| `set_content_type(type)` | the media type, when the filename is not enough |
| `set_disposition(kind)` | `attachment` by default, or `inline` |
| `set_cid(id)` | the identifier HTML refers to it by |
| `set_encoding(name)` | the transfer encoding, chosen from the data when absent |
| `set_description(text)` | what some clients show beside the file |

A file on disk is one call:

```zuri,ignore
note.attach(mail.Attachment.from_file('reports/q3.pdf'))
```

The name and the media type both come from the path unless something
says otherwise. A filename with characters a header cannot carry is
encoded the way RFC 2231 says, split across continuations if it is
long, and read back whole at the far end.

### Images Inside the HTML

An image the message shows rather than offers is attached like any
other file and referred to by an identifier:

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_subject('The logo')

var source = note.embed(
  mail.attachment(bytes([137, 80, 78, 71])).set_filename('logo.png')
)

note.set_html('<p>Our mark: <img src="${source}"></p>')

echo source.starts_with('cid:')
echo note.content_type().mime_type()
echo note.parts().map(@(part) => part.content_type().mime_type())
```

```console
true
multipart/related
[text/html, image/png]
```

`embed()` returns the reference to point an `<img>` at and rearranges
the message into the `multipart/related` that tells a reader the two
belong together. The HTML ends up as the first part, which is what
marks it as the one to display.

### Headers of Your Own

Anything the setters do not cover goes through `set_header()`:

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_header('X-Priority', '1')
  .set_header('List-Unsubscribe', '<https://example.com/unsubscribe>')

echo note.headers.get('X-Priority', nil)
```

```console
1
```

The value is written exactly as given. A header that needs encoding
needs it applied first; the setters that cover addresses and the
subject do that for you, because those are the headers where getting it
wrong is common.

`add_header()` keeps any header already there rather than replacing it,
which is what `Received` needs.

### Replies and Threads

A reply is an ordinary message with two more headers, and getting them
right is what puts it in the same conversation in a mail client:

```zuri
import mail

var original = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_subject('On Engines')
  .set_message_id('<first@example.com>')

var reply = mail.message()
  .set_from('grace@example.com')
  .add_to(original.sender())
  .set_subject('Re: ${original.subject()}')
  .set_in_reply_to(original.message_id())
  .set_references(original.references() + [original.message_id()])
  .set_text('Quite so.')

echo reply.headers.get('In-Reply-To', nil)
echo reply.references()
```

```console
<first@example.com>
[<first@example.com>]
```

`references()` on the original gives whatever chain it already belonged
to, so appending its own identifier extends the thread rather than
starting a new one.

## Reading a Message

`mail.parse()` takes the bytes and gives back the tree:

```zuri
import mail

var note = mail.parse(
  'From: Ada Lovelace <ada@example.com>\r\n'
  + 'To: grace@example.com\r\n'
  + 'Subject: =?utf-8?Q?On_Engines_and_Gr=C3=BC=C3=9Fe?=\r\n'
  + 'Content-Type: text/plain; charset=utf-8\r\n'
  + '\r\n'
  + 'The engine weaves algebraic patterns.'
)

echo note.sender().name
echo note.subject()
echo note.text()
```

```console
Ada Lovelace
On Engines and Grüße
The engine weaves algebraic patterns.
```

Nothing is decoded until something asks for it. Reading the subject of
a message with a twenty megabyte attachment in it costs no more than
reading the headers, which is what makes it reasonable to parse
everything in a mailbox and look at only some of it.

### The Tree

A part is a message too: it has headers and a body, and its body may be
more parts. The same class covers both, so walking a message and
reading a standalone one are the same code.

```zuri
import mail

var note = mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_text('plain')
  .set_html('<p>html</p>')
  .attach(mail.attachment('some,data\n').set_filename('figures.csv'))

for part in note.walk() {
  echo part.content_type().mime_type()
}
```

```console
multipart/mixed
multipart/alternative
text/plain
text/html
text/csv
```

`walk()` is the whole tree, outermost first. `parts()` is one level.
`find_part()` is the first part of a given type anywhere in it.

### Finding the Body

Most programs want the text, wherever it happens to be:

```zuri
import mail

var note = mail.parse(mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_text('the plain version')
  .set_html('<p>the html version</p>')
  .to_bytes())

echo note.text_body()
echo note.html_body()
```

```console
the plain version
<p>the html version</p>
```

Both return `nil` when the message has no such part, which is not the
same as an empty one. An attachment that happens to be `text/plain` is
not mistaken for the body: a part that says it is an attachment, or
that carries a filename without saying either way, is left out.

### Attachments Coming In

```zuri
import mail

var note = mail.parse(mail.message()
  .set_from('ada@example.com')
  .add_to('grace@example.com')
  .set_text('Attached.')
  .attach(mail.attachment('name,total\nengines,41')
    .set_filename('figures.csv'))
  .to_bytes())

for part in note.attachments() {
  echo '${part.filename()} (${part.content_type().mime_type()})'
  echo part.body_bytes().to_string()
}
```

```console
figures.csv (text/csv)
name,total
engines,41
```

`body_bytes()` decodes the transfer encoding, so base64 and
quoted-printable both come back as the bytes that went in. `text()`
goes one step further and applies the character set.

### Character Sets

A header carrying anything outside ASCII carries it encoded, and there
are two encodings it might have used. Reading a header through the
module decodes both:

```zuri
import mail.encoding

echo encoding.decode_words('=?utf-8?B?SGVsbG8=?= =?utf-8?B?IHdvcmxk?=')
echo encoding.decode_words('=?iso-8859-1?Q?caf=E9?=')
echo encoding.decode_words('this is not =?encoded? at all')
```

```console
Hello world
café
this is not =?encoded? at all
```

Whitespace between two adjacent encoded words is dropped, which is what
lets a long subject be split across several of them without a space
appearing where none was written. Anything that is not a well-formed
encoded word is left exactly as it is, including text that merely
begins with `=?`.

Bodies say their character set in the `Content-Type`. The sets a
message is likely to name are understood: UTF-8, US-ASCII, the ISO 8859
Latin sets 1 and 15, windows-1252, and UTF-16 in either byte order. One
this does not know is read as UTF-8, which leaves whatever ASCII is in
it intact rather than discarding the part that would have been
readable.

```zuri
import mail.encoding

echo encoding.decode_text(bytes([0x63, 0x61, 0x66, 0xe9]), 'iso-8859-1')
echo encoding.decode_text(bytes([0x93, 0x41, 0x94]), 'windows-1252')
```

```console
café
“A”
```

## Sending

`mail.send()` opens a connection, sends one message and closes it
again:

```zuri,ignore
import mail

mail.send('smtp://mail.example.com', mail.message()
  .set_from('reports@example.com')
  .add_to('ada@example.com')
  .set_subject('Quarterly report')
  .set_text('The numbers are in.'),
  { username: 'reports', password: secret })
```

That is the whole of sending mail for a program that sends one at a
time.

### A Client of Your Own

A program sending many wants a connection kept open across all of them,
because opening one costs a handshake and an authentication exchange:

```zuri,ignore
import mail.smtp { SmtpClient }

var server = SmtpClient.connect('smtp://mail.example.com', {
  username: 'reports',
  password: secret,
})

for note in queue {
  server.send(note, nil)
}

server.quit()
```

`quit()` says goodbye and closes. `close()` just closes, which is what
to do when something has gone wrong and the conversation is no longer
in a state the server would recognise.

| scheme | port | what it means |
| --- | --- | --- |
| `smtp://` | 587 | submission, with TLS negotiated over it |
| `smtps://` | 465 | TLS from the first byte |

Port 25 is for one server relaying to another, not for a program
submitting mail, and it is not a default here. Give it explicitly when
that is genuinely what you are doing.

### The Envelope

What the server is told and what the message says are two different
things. The envelope comes from the message unless the options say
otherwise: the sender from `From`, the recipients from `To`, `Cc` and
`Bcc` together, with duplicates removed.

```zuri,ignore
server.send(note, { from: 'bounces@example.com' })
server.send(note, { to: ['someone@example.com'] })
```

The first is what a mailing list does: the message says who wrote it,
the envelope says where a bounce should go. The second delivers to
somewhere the headers do not mention at all, which is how a blind copy
actually works underneath.

An empty sender is the null path, which is what a bounce is sent from
so that it cannot be bounced in turn:

```zuri,ignore
server.send(bounce, { from: '' })
```

### TLS

A client connects, reads what the server can do, negotiates TLS, and
only then sends anything worth protecting. That is the default:

| `tls` | what happens |
| --- | --- |
| `require` | negotiate TLS, and refuse to go on without it. The default. |
| `prefer` | negotiate it when the server offers it |
| `disable` | do not ask |

`smtps://`, `imaps://` and `pop3s://` handshake before the first byte
instead, and then `tls` has nothing left to decide.

Whatever `tls` says, a mechanism that puts the password on the wire is
never used on a connection that is not encrypted. A client offered
nothing else raises rather than sending it. That is not configurable,
and it is the one place the module refuses to do what it is told.

To trust a certificate the system does not, hand it a configuration:

```zuri,ignore
import net.tls

var config = tls.TlsConfig()

config.add_ca_pem(file('internal-ca.pem').read())

var server = SmtpClient.connect('smtp://mail.internal', {
  username: 'reports',
  password: secret,
  tls_config: config,
})
```

### Authenticating

Credentials go in the options, and the client picks the strongest
mechanism both ends know:

```zuri,ignore
SmtpClient.connect('smtp://mail.example.com', {
  username: 'reports',
  password: secret,
})

SmtpClient.connect('smtp://smtp.gmail.com', {
  username: 'reports@example.com',
  token: access_token,
})
```

A token picks a token mechanism. To force one rather than choosing,
pass `mechanisms`; the choice is described under [Authentication
Mechanisms](#authentication-mechanisms).

A username and password in the connection string work too, which is
convenient for a string that came from configuration:

```zuri,ignore
mail.send('smtp://reports:secret@mail.example.com', note, nil)
```

### What the Server Can Do

```zuri,ignore
echo server.capabilities().keys()
echo server.max_size()
```

A message larger than the server said it would take is refused before
it is sent rather than after uploading it, which matters when the
message is the reason the connection is slow.

Where the server offers `CHUNKING`, `send()` can hand the message over
in pieces with nothing escaped:

```zuri,ignore
server.send(note, { chunking: true })
```

Where it offers `SIZE`, `8BITMIME` or `DSN`, those are used without
being asked for. Where it does not, the message still goes.

### When Sending Fails

A refusal partway through a transaction leaves the server holding half
of one. `send()` abandons it before raising, so the connection is still
usable for the next message rather than answering `503` to everything
afterwards. That is worth knowing because doing it by hand is easy to
forget:

```zuri,ignore
for note in queue {
  catch {
    server.send(note, nil)
  } as error {
    failures.append([note, error])
  }
}

server.quit()
```

Every message after a failure still goes.

## Reading Mail with IMAP

IMAP leaves the mail on the server. A client opens a mailbox, searches
it, and fetches what it needs:

```zuri,ignore
import mail.imap { ImapClient }

var inbox = ImapClient.connect('imaps://mail.example.com', {
  username: 'ada',
  password: secret,
})

inbox.select('INBOX')

for uid in inbox.search('UNSEEN', true) {
  var note = inbox.fetch_message(uid, true, false)

  echo '${note.sender().address}: ${note.subject()}'
}

inbox.logout()
```

### Opening a Mailbox

```zuri,ignore
var box = inbox.select('INBOX')

echo box.exists
echo box.uidvalidity
echo box.permanent_flags
```

`select()` opens a mailbox for reading and writing. `examine()` opens
it read-only, which also means that reading a message does not mark it
read. `close_mailbox()` closes it and removes anything marked deleted
on the way out; `unselect()` closes it without doing that.

`uidvalidity` is how a server says its numbering has been reset. A
client that remembers identifiers between sessions has to check it: if
it has changed, every identifier it remembers means something else now.

To see what is there:

```zuri,ignore
for box in inbox.list('', '*') {
  echo '${box.name} ${box.is_selectable() ? '' : '(container only)'}'
}
```

`*` matches anything including the separator between levels; `%`
matches anything except it, which is what lists one level. `status()`
asks what is in a mailbox without opening it, which is how a client
shows unread counts for a dozen folders without selecting each one:

```zuri,ignore
echo inbox.status('Archive', ['MESSAGES', 'UNSEEN'])
```

### Searching

The criteria are IMAP's own, and they read close to English:

```zuri,ignore
inbox.search('UNSEEN', true)
inbox.search('FROM ada@example.com SINCE 1-Jan-2026', true)
inbox.search('SUBJECT engines LARGER 10000', true)
inbox.search('FLAGGED UNDELETED', true)
```

The second argument asks for unique identifiers rather than positions.
A position is only good until something is removed from the mailbox and
everything after it renumbers; an identifier outlives the connection
and is what a program that runs twice should remember. Prefer `true`
unless the numbers are being used immediately.

### Fetching Less Than Everything

Fetching every message to show a list of them is the mistake IMAP
exists to prevent:

```zuri,ignore
for info in inbox.fetch('1:50', 'ENVELOPE FLAGS RFC822.SIZE', false) {
  echo '${info.envelope.subject} (${info.size} bytes)'
}
```

The envelope is the addresses and the date, parsed by the server. No
body crossed the network at all.

| what to ask for | what comes back |
| --- | --- |
| `ENVELOPE` | the addresses, subject and date |
| `FLAGS` | what has been done to the message |
| `RFC822.SIZE` | how large it is |
| `INTERNALDATE` | when the server took it |
| `BODYSTRUCTURE` | the shape of the message, part by part |
| `BODY.PEEK[HEADER]` | the header block |
| `BODY.PEEK[]` | the whole message |
| `BODY.PEEK[2]` | one part of it |

`BODYSTRUCTURE` is the one worth knowing about. It reports what the
message is made of without sending any of it, so a client can decide to
fetch the text and leave a large attachment on the server:

```zuri,ignore
var structure = inbox.fetch(uid, 'BODYSTRUCTURE', true)[0].structure

for part in structure.walk() {
  echo '${part.section} ${part.mime_type()} ${part.size}'
}
```

`section` is the number to ask for that part on its own.

`fetch_headers()` is the common case of asking for the header block,
and `fetch_message()` the common case of asking for all of it:

```zuri,ignore
inbox.fetch_message(uid, true, false)   # leaves it unread
inbox.fetch_message(uid, true, true)    # marks it read
```

Reading a message does not mark it read unless you say so. A program
going through a mailbox should not change what a person sees when they
next open it.

### Flags

A flag is what has been done to a message:

```zuri,ignore
inbox.add_flags(uid, ['\\Seen'], true)
inbox.remove_flags(uid, ['\\Flagged'], true)
inbox.mark_seen(uid, true)
inbox.mark_deleted(uid, true)
```

`\Seen`, `\Answered`, `\Flagged`, `\Deleted` and `\Draft` are the ones
with a defined meaning. A server may allow others, and says which in
the mailbox's `permanent_flags`.

`mark_deleted()` only marks. Nothing goes until `expunge()`:

```zuri,ignore
inbox.mark_deleted(uid, true)
echo inbox.expunge()
```

`expunge()` returns the positions that went, highest first, because
each removal renumbers everything after it and that is the order they
have to be applied in.

### Moving, Copying and Removing

```zuri,ignore
inbox.copy(uid, 'Archive', true)
inbox.move(uid, 'Archive', true)
```

`move()` uses the server's own `MOVE` where there is one, and otherwise
does what `MOVE` was invented to replace: copy, mark deleted, expunge.
Either way the message ends up in one place.

`create()`, `delete()`, `rename()`, `subscribe()` and `unsubscribe()`
do what they say.

### Putting a Message Back

`append()` adds a message to a mailbox without sending it anywhere,
which is how a sent message gets into the Sent folder:

```zuri,ignore
inbox.append('Sent', note, ['\\Seen'], nil)
```

The flags are the ones to file it under, and `\Seen` is the usual one
for something the account itself wrote. The last argument is the
internal date; the time of arrival is used when it is not given.

### Waiting for New Mail

`idle()` waits for the server to say something rather than asking over
and over:

```zuri,ignore
inbox.on_event(@(response) {
  echo 'the server said ${response.name()}'
})

while true {
  inbox.idle(1500000)
}
```

A server may drop a connection that idles for too long, which is why
the wait is bounded and the loop comes back around. Twenty-five minutes
is the default and is what RFC 2177 recommends.

`noop()` does the same thing without waiting: it gives the server a
chance to report anything that has changed, and keeps the connection
from going idle at all.

## Collecting Mail with POP3

POP3 hands the mail over and forgets it. There is no searching and no
folders; there is a numbered list, and there is taking things off it:

```zuri,ignore
import mail.pop3 { Pop3Client }

var mailbox = Pop3Client.connect('pop3s://mail.example.com', {
  username: 'ada',
  password: secret,
})

for entry in mailbox.list() {
  archive(mailbox.retrieve(entry.number))
  mailbox.delete(entry.number)
}

mailbox.quit()
```

`list()` gives every message with its size and, where the server offers
one, an identifier that outlives the session. The number does not: it
is only good until something is removed and the rest renumber.

Nothing is actually removed until `quit()`. `delete()` only marks and
`reset()` unmarks everything. A connection that drops halfway leaves
the mailbox exactly as it was, which is the protocol protecting you
from a program that fails in the middle. It also means that closing
without `quit()` is how to abandon a run.

`top()` fetches the headers and the first few lines of the body, which
is enough to decide whether the rest is worth fetching:

```zuri,ignore
var preview = mailbox.top(entry.number, 5)
```

Where the server's greeting offers it, `APOP` is used in preference to
sending the password, and where it offers SASL those mechanisms are
preferred again. `USER`/`PASS` is the last resort and is only used over
TLS.

## Running an SMTP Server

`SmtpServer` knows the protocol and nothing about policy. What to
accept is decided by handlers:

```zuri,ignore
import mail.smtp { SmtpServer }

var server = SmtpServer({ port: 2525, hostname: 'mail.example.com' })

server.on_rcpt(@(session, recipient) {
  if !accounts.contains(recipient.local) {
    return { code: 550, message: 'no such user here', enhanced: '5.1.1' }
  }
})

server.on_data(@(session, raw) {
  for recipient in session.recipients {
    store.append(recipient.local, 'INBOX', raw, nil, nil)
  }
})

server.listen()
```

### The Handlers

| handler | called with | when |
| --- | --- | --- |
| `on_connect` | the session | a connection opens, before the greeting |
| `on_auth` | the session and the credentials | a client authenticates |
| `on_mail` | the session and the sender | a transaction starts |
| `on_rcpt` | the session and a recipient | for each recipient |
| `on_data` | the session and the message | the whole message has arrived |
| `on_close` | the session | the connection closes, however it closed |
| `on_error` | the error and the session | something inside the server failed |

The session carries what is known so far: who connected, whether the
connection is encrypted, who authenticated, the sender, and the
recipients accepted up to now. `session.state` is an empty dictionary
your handlers can put anything in, and it lives as long as the
connection.

### Refusing Properly

A handler that returns nothing accepts. One that returns a code and a
message refuses with those:

```zuri,ignore
server.on_mail(@(session, sender) {
  if blocklist.contains(sender.domain) {
    return { code: 550, message: 'not accepted from there', enhanced: '5.7.1' }
  }
})
```

A handler that raises is a failure on the server's side, not a bad
message, and the sender is told `451` and to try again later. That
distinction matters: a database that is down should not turn into a
bounce.

Refusing at `on_rcpt` is the useful one. It tells the sender
immediately which address is wrong, while it is still connected and can
do something about it, rather than accepting the message and generating
a bounce to an address that may not exist either.

The null sender, which is what a bounce comes from, arrives at
`on_mail` as an empty string rather than an address. Refusing to accept
mail from it is how a server ends up unable to receive bounces, so
handle it deliberately.

### TLS and Authentication

```zuri,ignore
server.use_tls(file('cert.pem').read(), file('key.pem').read())
```

That is what lets the server offer `STARTTLS`. Everything a client said
before the handshake is discarded afterwards, including who it claimed
to be, because none of it was protected.

Authentication needs one handler and, for the mechanisms that prove a
password without sending it, a second:

```zuri,ignore
server.on_auth(@(session, credentials) {
  if !accounts.verify(credentials.username, credentials.password) {
    return { code: 535, message: 'no' }
  }
})

server.on_password(@(username) {
  return accounts.password_of(username)
})
```

`on_password` is what makes `CRAM-MD5` possible: checking that proof
means working out the same one, which means knowing the password. A
server that cannot produce one does not advertise the mechanism.
Without `on_auth` the server advertises no authentication at all.

`PLAIN` and `LOGIN` are advertised only once the connection is
encrypted. `require_auth` refuses mail from a client that has not
authenticated, and `require_tls` refuses it on a connection that is not
encrypted.

### Limits

| option | default | what it does |
| --- | --- | --- |
| `max_size` | 35 MB | the largest message to take |
| `max_recipients` | 100 | recipients one message may have |
| `timeout` | 300000 | milliseconds a client may go quiet |

A client that declares a size past the limit is refused at `MAIL`,
before it uploads anything. One that does not declare it is refused
when it goes past. Ten malformed commands in a row and the connection
is dropped.

## Running an IMAP Server

`ImapServer` runs the session state machine and answers every command
out of a `MailStore`. What mail there is and where it lives is the
store's business:

```zuri,ignore
import mail.imap { ImapServer, MaildirStore }

var store = MaildirStore('/var/mail')

store.add_account('ada', secret)

ImapServer({ port: 143 }, store).listen()
```

The server implements IMAP4rev1 along with `UNSELECT`, `MOVE`, `IDLE`,
`LITERAL+`, `SASL-IR` and `ID`. It advertises exactly what it
implements, so a client that reads the capability list and trusts it
will not go wrong.

### Where the Mail Lives

Two stores ship. `MemoryStore` keeps everything in the process and
forgets it on exit, which is what a test wants and what a server
embedded in something larger wants when the mail it holds is not the
point:

```zuri
import mail
import mail.imap { MemoryStore }

var store = MemoryStore(['INBOX', 'Archive'])

store.add_account('ada', 'secret')
store.append('ada', 'INBOX', mail.message()
  .set_from('grace@example.com')
  .add_to('ada@example.com')
  .set_subject('On Bugs')
  .set_text('Found one.')
  .to_bytes(), nil, nil)

echo store.mailboxes('ada')
echo store.messages('ada', 'INBOX').length()
echo store.messages('ada', 'INBOX')[0].uid
```

```console
[Archive, INBOX]
1
1
```

`MaildirStore` is the same contract on disk.

### Maildir

Each account is a directory, holding its INBOX directly and every other
mailbox beside it under a leading dot, which is the Maildir++ arrangement
every other mail tool understands. A mailbox written by this server can be
read by anything else, and mail delivered by anything else turns up here.

Every mailbox is the three directories Maildir defines. A message is
written into `tmp`, where nothing reads from, and only moved into place
once it is whole, so a reader never sees half of one. `new` is where a
delivery agent leaves mail nobody has looked at yet, and `cur` is where a
message lives once a client has seen the mailbox, with its flags recorded
in the filename after `:2,`. So an account on disk looks like this:

```text
ada/
  cur/   new/   tmp/   zuri-uidlist
  .Archive/
    cur/   new/   tmp/   zuri-uidlist
```

That interoperability is the reason to choose it. A delivery agent can
drop a message in and the server finds it on the next look, with no
shared database and no protocol between them.

IMAP needs a message number that survives a rename and Maildir has no
such thing, so each mailbox keeps a small index file beside its
directories recording which file is which number. A message whose file
has gone keeps its number retired rather than reused, so a client that
remembered one is told the message is missing rather than handed a
different one.

Mail is on disk; accounts are not:

```zuri,ignore
store.add_account('ada', secret)

store.set_authenticator(@(username, password) {
  return accounts.verify(username, password)
})
```

Where an application keeps its passwords is the application's business
and not a mail library's. A store with an authenticator can no longer
produce a password, so the server stops advertising the mechanisms that
need one.

### A Store of Your Own

A store only has to answer the same calls. Nothing in the contract
requires a file:

| | |
| --- | --- |
| `authenticate(username, password)` | are these the right credentials |
| `password_of(username)` | the password, for the mechanisms that need it |
| `has_passwords()` | whether it can produce one at all |
| `mailboxes(username)` | what mailboxes there are |
| `exists`, `create`, `remove`, `rename` | the mailboxes themselves |
| `messages(username, mailbox)` | everything in one, oldest first |
| `append(username, mailbox, raw, flags, received)` | add a message |
| `set_flags(username, mailbox, uid, flags)` | change one's flags |
| `expunge(username, mailbox)` | remove what is marked deleted |
| `counters(username, mailbox)` | `uidnext` and `uidvalidity` |

Subclass `MailStore` and implement them, and an `ImapServer` will serve
whatever is behind it.

## Serving More Than One Connection

Both servers serve one connection to the end before taking the next.
That is the right shape for the protocols and the wrong shape for more
than one client at a time. `mail.pool` puts a pool of isolates behind
one listening socket:

```zuri,ignore
import mail.pool
import .my_server

pool.serve(my_server.build, { host: '0.0.0.0', port: 143, workers: 8 })
```

`build` is a function in a module of its own rather than a closure,
because each worker resolves it by name on its own side and builds its
own server there:

```zuri,ignore
# my_server.zu
import mail.imap { ImapServer, MaildirStore }

def build() {
  var store = MaildirStore('/var/mail')

  store.set_authenticator(accounts.verify)

  return ImapServer({}, store)
}
```

An IMAP connection can be open for hours, so the pool is a ceiling on
how many clients can be served at once rather than on how fast they are
served. Size it for the number of clients, not the rate of requests.

`pool.start()` does everything except run the accept loop, which is
what to use when the address has to be known before the first
connection, as it does in a test.

## Proving Where Mail Came From

A receiving server has no reason to believe a `From` header. DKIM is
the domain signing the message on the way out, and the receiver
checking the signature against a key published in that domain's own
DNS.

### Signing

```zuri,ignore
import mail.dkim { Signer }

var signer = Signer('example.com', 'default', private_key)

signer.sign(note)
```

The signature covers the body and the headers worth covering, and the
header it adds goes at the top. It goes on last, once everything else
about the message is settled: changing a signed header afterwards
breaks it, and `mail.send()` sets the `Message-ID` and `Date` if they
are absent, so let it or set them yourself before signing.

| option | default | what it does |
| --- | --- | --- |
| `algorithm` | `rsa-sha256` | or `ed25519-sha256` |
| `canonicalisation` | `relaxed/relaxed` | headers then body |
| `headers` | a sensible set | which headers to cover |
| `expires_in` | none | seconds until the signature stops counting |

`relaxed` forgives the whitespace and folding changes a mail server may
make in passing. `simple` covers the bytes exactly, which means any
change at all on the way breaks the signature. Both are implemented;
`relaxed/relaxed` is what almost everything uses and is the default for
that reason.

The public half goes in DNS under the selector:

```console
default._domainkey.example.com  TXT  "v=DKIM1; k=rsa; p=MIIBIjANBg..."
```

### Checking

```zuri,ignore
import mail.dkim

for result in dkim.verify(incoming) {
  if result.valid {
    echo '${result.domain} takes responsibility for this'
  } else {
    echo '${result.domain}: ${result.reason}'
  }
}
```

One result per signature, in the order they appear. A message with no
signatures gives an empty list, which is not a failure: it is a message
nobody signed.

The key is looked up through `net.resolver` unless `verify()` is handed
something else to look it up with, which is what a program with its own
resolver or its own cache wants:

```zuri,ignore
dkim.verify(incoming, @(name) {
  return my_dns.text_records(name)
})
```

A message that was parsed is checked against the bytes it was parsed
from, which is the only thing a signature can be checked against.
Changing a parsed message and checking it again reports on the message
that arrived, not the one now in hand.

### What a Signature Does Not Say

A valid signature says the domain vouches for the message. It does not
say the message is wanted, that the `From` header matches the signing
domain, or that the sender is who they claim to be to a human reader.
Deciding what a signature from a given domain is worth is a separate
question with a separate answer, and that answer is usually DMARC.

DNSSEC is not validated. Signatures are carried through when a server
sends them and the `dnssec` option asks for them, but nothing here
checks one. A program that needs a validated answer wants a validating
resolver and a trusted path to it, which is what `net.resolver`'s `tls`
option gives.

## Authentication Mechanisms

All three protocols carry the same mechanisms, and `mail.sasl` holds
them once rather than three times. A client is given credentials and
picks the strongest thing both ends know:

| | |
| --- | --- |
| `SCRAM-SHA-256`, `SCRAM-SHA-1` | proves the password without sending it, and proves the server knew it too |
| `CRAM-MD5` | proves it without sending it, and proves nothing about the server |
| `XOAUTH2`, `OAUTHBEARER` | a bearer token, which is what the large providers want |
| `PLAIN`, `LOGIN` | the password itself, so never without TLS |
| `EXTERNAL` | nothing: the client certificate already said who this is |

The order is the order of that table. SCRAM is the one to use where a
server offers it: the server stores something derived from the password
rather than the password, the client never sends it, and the exchange
ends with the server proving it knew it too, which is what stops a
server that simply says yes to everything.

Channel binding, the `-PLUS` form of SCRAM, is not offered. It needs a
value out of the TLS session that `net.tls` does not expose.

To force a mechanism rather than choosing:

```zuri,ignore
SmtpClient.connect(url, {
  username: 'reports',
  password: secret,
  mechanisms: ['SCRAM-SHA-256'],
})
```

## Errors

Every error is a `MailError`. Catching that catches everything the
module raises on its own.

| | |
| --- | --- |
| `MessageError` | the bytes are not a message, or are one that contradicts itself |
| `ProtocolError` | the server said something the protocol does not allow |
| `ConnectionClosed` | the connection went away mid-conversation |
| `AuthenticationError` | the credentials were refused, or nothing usable was offered |
| `StateError` | a command that makes no sense where it was issued |
| `MailboxError` | a mailbox that does not exist, or cannot be created |
| `SmtpError` | the SMTP server refused, with a code |
| `ImapError` | the IMAP server answered `NO` or `BAD` |
| `Pop3Error` | the POP3 server answered `-ERR` |

The one distinction worth building a mail program around is SMTP's:

```zuri,ignore
catch {
  mail.send(url, note, credentials)
} as error {
  if instance_of(error, mail.SmtpTransientError) {
    queue.retry(note)
  } else if instance_of(error, mail.SmtpPermanentError) {
    queue.bounce(note, error.code)
  } else {
    raise error
  }
}
```

A 4xx reply means the server could not take the message now and the
sender should try again later. A 5xx means it will not take the
message and trying again changes nothing. Queue the first and bounce
the second; treating them alike is how a mail queue either loses mail
or sends it forty times.

`SmtpError` carries `code`, the three digit reply, and `enhanced`, the
finer grained code from RFC 3463 when the server sends one. Neither is
meant to be matched on beyond its first digit.

`ImapError` carries `status`, which is `NO` when the server understood
the command and refused it, and `BAD` when it did not understand it at
all. `BAD` points at the client rather than at the request.

## What the Module Refuses

**It will not send a password over an unencrypted connection.** Not
with an option, not with a flag. A client offered only mechanisms that
would do that raises instead. `tls: 'disable'` turns off negotiating
TLS; it does not turn off this.

**It will not write a character set it cannot encode.** Asking for a
body in ISO 8859-1 raises rather than writing UTF-8 bytes under a
header that claims otherwise.

**It does not validate DNSSEC.** See [What a Signature Does Not
Say](#what-a-signature-does-not-say).

**It does not implement a POP3 server.** A POP3 server is an IMAP
server with almost everything removed, and running `mail.imap` is the
better answer.

**It does not decide whether mail is wanted.** There is no spam
filtering, no reputation, and no policy. DKIM tells you who signed
something; what to do about that is yours.

## Module Reference

The standard library reference documents every class and method. The
shape of the module:

| | |
| --- | --- |
| `mail.message()` | starts a message |
| `mail.parse(data)` | reads one |
| `mail.send(url, note, options)` | sends one, connection and all |
| `mail.smtp(url, options)` | a connection to a server that sends mail |
| `mail.imap(url, options)` | a connection to a server that stores it |
| `mail.pop3(url, options)` | a connection to a server that hands it over |
| `mail.parse_address(text)` | one address |
| `mail.parse_address_list(text)` | every address in a header |
| `mail.format_addresses(people)` | the other direction |
| `mail.attachment(data)` | starts an attachment |
| `mail.Attachment.from_file(path)` | one read from disk |

On an `Attachment`:

| | |
| --- | --- |
| `set_filename`, `set_content_type`, `set_encoding` | what it is |
| `set_disposition`, `set_cid`, `set_description` | how it is presented |
| `filename`, `content_type`, `disposition`, `cid`, `size`, `data` | reading them back |
| `to_part` | the message part it becomes |

On a `Message`:

| | |
| --- | --- |
| `set_from`, `set_sender`, `set_reply_to` | who it is from |
| `add_to`, `add_cc`, `add_bcc`, `set_to`, `set_cc`, `set_bcc` | who it is for |
| `set_subject`, `set_text`, `set_html` | what it says |
| `attach`, `embed` | what it carries, as an `Attachment` |
| `set_header`, `add_header`, `set_date`, `set_message_id` | anything else |
| `set_in_reply_to`, `set_references` | which conversation it belongs to |
| `sender`, `to`, `cc`, `bcc`, `reply_to`, `recipients` | reading them back |
| `subject`, `text`, `text_body`, `html_body` | reading what it says |
| `parts`, `walk`, `find_part`, `attachments` | the tree |
| `to_bytes`, `to_string` | writing it out |

The protocol modules:

| | |
| --- | --- |
| `mail.smtp` | `SmtpClient`, `SmtpServer`, `SmtpSession`, `Reply` |
| `mail.imap` | `ImapClient`, `ImapServer`, `Mailbox`, `Envelope`, `BodyPart`, `MessageInfo` |
| `mail.imap` | `MailStore`, `MaildirStore`, `MemoryStore`, `StoredMessage` |
| `mail.pop3` | `Pop3Client`, `Entry` |
| `mail.dkim` | `Signer`, `Signature`, `Result`, `verify`, `is_signed` |
| `mail.sasl` | `of`, `choose`, and a class per mechanism |
| `mail.pool` | `serve`, `start`, `Cluster` |

And the pieces underneath, for a program that needs them directly:

| | |
| --- | --- |
| `mail.address` | `parse`, `parse_list`, `parse_groups`, `format_list` |
| `mail.headers` | `Headers`, `fold`, `unfold` |
| `mail.encoding` | the transfer encodings, encoded words and parameters |
| `mail.content` | `ContentType`, `ContentDisposition` |
| `mail.stream` | `LineStream`, `connect`, `start_tls`, `endpoint` |
| `mail.imap.parser` | the IMAP grammar, for reading a response by hand |
