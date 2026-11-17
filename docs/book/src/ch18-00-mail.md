# Mail

The `mail` module is Zuri's email stack. It covers the message format, the
three protocols used to move messages around, and the servers for two of
them. It's written in Zuri from the socket up, and it works with real mail
servers.

Email is older than almost everything it runs on, and it shows. A message is
text with a block of headers, a format defined in 1982 and extended ever
since to carry anything that isn't ASCII, which by now is most of what
people send. Sending a message uses SMTP, reading messages where they're
stored uses IMAP, and downloading them uses POP3. That's three protocols,
one format and a lot of accumulated history, and a program shouldn't have to
know about almost any of it.

That's the idea behind this module's design. You describe what's in a
message, and the module works out the rest: which parts it needs, how they
nest, which encoding each header and body needs, and what has to be escaped
so that a line containing a single dot doesn't end the message early. Your
program says who a message is from and what it says. It doesn't need to know
about `Content-Transfer-Encoding`.

- [Following Along](#following-along)
- [Introduction](#introduction)
- [Building a Message](#building-a-message)
  - [Addresses](#addresses)
  - [Text, HTML, or Both](#text-html-or-both)
  - [Attachments](#attachments)
  - [Images in HTML](#images-in-html)
  - [Custom Headers](#custom-headers)
  - [Replies and Threads](#replies-and-threads)
- [Reading a Message](#reading-a-message)
  - [The Message Tree](#the-message-tree)
  - [Finding the Body](#finding-the-body)
  - [Incoming Attachments](#incoming-attachments)
  - [Character Sets](#character-sets)
- [Sending Mail](#sending-mail)
  - [Creating Your Own Client](#creating-your-own-client)
  - [The Envelope](#the-envelope)
  - [TLS](#tls)
  - [Authenticating](#authenticating)
  - [Server Capabilities](#server-capabilities)
  - [Handling Failures](#handling-failures)
- [Reading Mail with IMAP](#reading-mail-with-imap)
  - [Opening a Mailbox](#opening-a-mailbox)
  - [Searching](#searching)
  - [Fetching Only What You Need](#fetching-only-what-you-need)
  - [Flags](#flags)
  - [Moving, Copying and Removing](#moving-copying-and-removing)
  - [Adding a Message to a Mailbox](#adding-a-message-to-a-mailbox)
  - [Waiting for New Mail](#waiting-for-new-mail)
- [Collecting Mail with POP3](#collecting-mail-with-pop3)
- [Running an SMTP Server](#running-an-smtp-server)
  - [The Handlers](#the-handlers)
  - [Rejecting Mail](#rejecting-mail)
  - [TLS and Authentication](#tls-and-authentication)
  - [Limits](#limits)
- [Running an IMAP Server](#running-an-imap-server)
  - [Where the Mail Is Stored](#where-the-mail-is-stored)
  - [Maildir](#maildir)
  - [Writing Your Own Store](#writing-your-own-store)
- [Serving Several Connections](#serving-several-connections)
- [Verifying Where Mail Came From](#verifying-where-mail-came-from)
  - [Signing](#signing)
  - [Verifying Signatures](#verifying-signatures)
  - [What a Signature Does Not Say](#what-a-signature-does-not-say)
- [Authentication Mechanisms](#authentication-mechanisms)
- [Errors](#errors)
- [Design Limits](#design-limits)
- [Module Reference](#module-reference)

## Following Along

Most of the examples below build on the same message. Here it is:

```zuri
import mail

var note = mail
    .message()
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

None of this uses the network. Building and reading messages is completely
separate from sending and receiving them, so a program that only needs to
create or parse a message never opens a socket.

## Introduction

The three protocols do quite different jobs.

**SMTP** moves a message from wherever it was written to a server that will
take responsibility for it. That's all it does. It has no concept of
folders, read messages or searches. A message goes in, and it's either
accepted or it isn't.

**IMAP** is for reading mail where it's stored. The mail stays on the
server, the client works with it there, and two devices looking at the same
mailbox see the same thing. Almost everything a mail client does uses IMAP.

**POP3** is for downloading mail. It offers a numbered list of messages and
a way to remove them. It's the wrong protocol for reading mail on several
devices, but the right one for a program whose job is to move everything in
a mailbox somewhere else.

The module covers the client side of all three protocols, and the server
side of the two where a server is worth having. There's no POP3 server, and
that's intentional: a POP3 server is an IMAP server with almost everything
removed, so `mail.imap` is the one to run.

## Building a Message

`mail.message()` creates a message. You set its contents one call at a time,
and each call returns the message, so you can chain them together:

```zuri
import mail

var note = mail
    .message()
    .set_from('ada@example.com')
    .add_to('grace@example.com')
    .set_subject('On Engines')
    .set_text('The analytical engine weaves algebraic patterns.')

echo note.content_type().mime_type()
```

```console
text/plain
```

A new message starts with a `Date` header, set to the time it was created,
and a `MIME-Version` header. It gets its `Message-ID` when it's sent,
because that's when the domain it belongs to is known.

### Addresses

Every method that sets addresses accepts text, an `Address`, or a list of
either:

```zuri
import mail

var note = mail
    .message()
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

`add_to()` keeps any recipients that are already there, so you can call it
in a loop. `set_to()` replaces them. There's the same pair of methods for
`Cc` and `Bcc`.

`Bcc` is removed from the message before it's passed to a server, after the
recipients have been taken from it. That's the whole point of a blind copy,
and it's easy to get wrong if you do it yourself.

A display name that contains characters a header can't hold is encoded for
you, and a name that contains a comma is quoted:

```zuri
import mail

echo mail.Address('gruss@example.com', 'Grüße').to_string()
echo mail.Address('ada@example.com', 'Lovelace, Ada').to_string()
```

```console
=?utf-8?B?R3LDvMOfZQ==?= <gruss@example.com>
"Lovelace, Ada" <ada@example.com>
```

When you read them back, you get the name, not the encoded form:

```zuri
import mail

echo mail.parse_address('=?utf-8?B?R3LDvMOfZQ==?= <gruss@example.com>').name
echo mail.parse_address('"Lovelace, Ada" <ada@example.com>').name
```

```console
Grüße
Lovelace, Ada
```

`mail.parse_address_list()` parses an entire header, including the comments
and groups that RFC 5322 allows but nobody remembers:

```zuri
import mail

var people = mail.parse_address_list(
  'Ada <ada@example.com> (the author), Engineers: grace@example.com, charles@example.com;',
)

echo people.map(@(person) => person.address)
```

```console
[ada@example.com, grace@example.com, charles@example.com]
```

The members of a group are returned along with the other addresses, since a
program sending mail cares about who the recipients are, not how they were
grouped. Use `mail.parse_address_groups()` if you need to keep the grouping.

### Text, HTML, or Both

Text on its own makes a plain text message, and HTML on its own makes an
HTML message. Both together make a `multipart/alternative` message, with the
plain text first, because a mail client that understands both is supposed to
show the last one it can display:

```zuri
import mail

var note = mail
    .message()
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

Setting the text again replaces it, wherever it is in the message, instead
of adding a second copy. The structure of the message follows from its
contents, so it stays correct as the contents change.

Outgoing text is written as UTF-8, or as US-ASCII if that's all it needs.
Asking for any other character set raises an error, because the module can't
encode into it, and a header that claimed otherwise would be wrong.

### Attachments

An attachment is an object in its own right. You build it the same way as a
message and pass it to `attach()`:

```zuri
import mail

var note = mail
    .message()
    .set_from('ada@example.com')
    .add_to('grace@example.com')
    .set_subject('The figures')
    .set_text('Attached.')
    .attach(
      mail.attachment('name,total\nengines,41\n').set_filename('figures.csv'),
    )

echo note.content_type().mime_type()
echo note.attachments().map(@(part) => part.filename())
echo note.attachments()[0].content_type().mime_type()
```

```console
multipart/mixed
[figures.csv]
text/csv
```

The media type was taken from the file name. The message became a
`multipart/mixed` message, with its previous contents as the first part.
That only happens once, however many files you attach.

| Method | Purpose |
| --- | --- |
| `set_filename(name)` | The file name to offer the file under |
| `set_content_type(type)` | The media type, when the file name isn't enough to tell |
| `set_disposition(kind)` | `attachment` by default, or `inline` |
| `set_cid(id)` | The identifier the HTML uses to refer to it |
| `set_encoding(name)` | The transfer encoding. If you don't set it, it's chosen based on the data. |
| `set_description(text)` | A description that some mail clients show next to the file |

Attaching a file from disk takes one call:

```zuri,ignore
note.attach(mail.Attachment.from_file('reports/q3.pdf'))
```

Both the name and the media type are taken from the path, unless you say
otherwise. A file name with characters that a header can't hold is encoded
as RFC 2231 describes, split across several lines if it's long, and read
back in one piece at the other end.

### Images in HTML

An image that the message displays, rather than offers as a download, is
attached like any other file and referred to by an identifier:

```zuri
import mail

var note = mail
    .message()
    .set_from('ada@example.com')
    .add_to('grace@example.com')
    .set_subject('The logo')

var source = note.embed(
  mail.attachment(bytes([137, 80, 78, 71])).set_filename('logo.png'),
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

`embed()` returns the reference to use in an `<img>` tag, and rearranges the
message into a `multipart/related` structure, which tells the mail client
that the two belong together. The HTML ends up as the first part, which
marks it as the one to display.

### Custom Headers

For any header that the other methods don't cover, use `set_header()`:

```zuri
import mail

var note = mail
    .message()
    .set_from('ada@example.com')
    .add_to('grace@example.com')
    .set_header('X-Priority', '1')
    .set_header('List-Unsubscribe', '<https://example.com/unsubscribe>')

echo note.headers.get('X-Priority', nil)
```

```console
1
```

The value is written exactly as you give it. If a header needs encoding, you
have to encode it first. The methods for addresses and the subject do this
for you, because those are the headers where people most often get it wrong.

`add_header()` keeps any existing header with the same name instead of
replacing it, which is what a header such as `Received` needs.

### Replies and Threads

A reply is an ordinary message with two extra headers. Getting them right is
what puts the reply in the same conversation in a mail client:

```zuri
import mail

var original = mail
    .message()
    .set_from('ada@example.com')
    .add_to('grace@example.com')
    .set_subject('On Engines')
    .set_message_id('<first@example.com>')

var reply = mail
    .message()
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

`references()` on the original message returns the chain it already belongs
to, so adding the original's own identifier extends the thread instead of
starting a new one.

## Reading a Message

`mail.parse()` takes the raw bytes and returns the message tree:

```zuri
import mail

var note = mail.parse(
  'From: Ada Lovelace <ada@example.com>\r\n' +
      'To: grace@example.com\r\n' +
      'Subject: =?utf-8?Q?On_Engines_and_Gr=C3=BC=C3=9Fe?=\r\n' +
      'Content-Type: text/plain; charset=utf-8\r\n' +
      '\r\n' +
      'The engine weaves algebraic patterns.',
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

Nothing is decoded until you ask for it. Reading the subject of a message
with a 20 MB attachment costs no more than reading the headers, which makes
it reasonable to parse every message in a mailbox and only look at some of
them.

### The Message Tree

A part is a message too. It has headers and a body, and its body can contain
more parts. The same class represents both, so the code for walking through
a message and reading a standalone one is the same.

```zuri
import mail

var note = mail
    .message()
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

`walk()` returns the whole tree, starting with the outermost part. `parts()`
returns one level of it, and `find_part()` returns the first part of a given
type anywhere in the tree.

### Finding the Body

Most programs want the text, wherever it is in the message:

```zuri
import mail

var note = mail.parse(
  mail
      .message()
      .set_from('ada@example.com')
      .add_to('grace@example.com')
      .set_text('the plain version')
      .set_html('<p>the html version</p>')
      .to_bytes(),
)

echo note.text_body()
echo note.html_body()
```

```console
the plain version
<p>the html version</p>
```

Both methods return `nil` if the message has no such part, which isn't the
same as an empty part. An attachment that happens to be `text/plain` isn't
mistaken for the body. A part that says it's an attachment, or that has a
file name without saying either way, is skipped.

### Incoming Attachments

```zuri
import mail

var note = mail.parse(
  mail
      .message()
      .set_from('ada@example.com')
      .add_to('grace@example.com')
      .set_text('Attached.')
      .attach(
        mail.attachment('name,total\nengines,41').set_filename('figures.csv'),
      )
      .to_bytes(),
)

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

`body_bytes()` decodes the transfer encoding, so both base64 and
quoted-printable return the original bytes. `text()` goes one step further
and also applies the character set.

### Character Sets

A header that contains anything outside ASCII stores it in encoded form,
using one of two encodings. Reading a header through the module decodes
both:

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

Whitespace between two adjacent encoded words is removed, which is what
allows a long subject to be split across several encoded words without
adding spaces that weren't there. Anything that isn't a well-formed encoded
word is left exactly as it is, including text that just happens to start
with `=?`.

A message body declares its character set in the `Content-Type` header. The
character sets that messages are likely to use are all supported: UTF-8,
US-ASCII, the ISO 8859 Latin sets 1 and 15, windows-1252, and UTF-16 in
either byte order. A character set the module doesn't know is read as UTF-8,
which keeps any ASCII text in it intact instead of throwing away the part
that would have been readable.

```zuri
import mail.encoding

echo encoding.decode_text(bytes([0x63, 0x61, 0x66, 0xe9]), 'iso-8859-1')
echo encoding.decode_text(bytes([0x93, 0x41, 0x94]), 'windows-1252')
```

```console
café
“A”
```

## Sending Mail

`mail.send()` opens a connection, sends a single message and closes the
connection again:

```zuri,ignore
import mail

mail.send(
  'smtp://mail.example.com',
  mail
      .message()
      .set_from('reports@example.com')
      .add_to('ada@example.com')
      .set_subject('Quarterly report')
      .set_text('The numbers are in.'),
  { username: 'reports', password: secret },
)
```

That's all you need to send mail from a program that sends one message at a
time.

### Creating Your Own Client

A program that sends many messages should keep one connection open for all
of them, because opening a connection involves a handshake and an
authentication exchange:

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

`quit()` ends the session politely and closes the connection. `close()` just
closes it, which is what you should do when something has gone wrong and the
conversation is no longer in a state the server would understand.

| Scheme | Port | Meaning |
| --- | --- | --- |
| `smtp://` | 587 | Submission, with TLS negotiated over the connection |
| `smtps://` | 465 | TLS from the first byte |

Port 25 is for one server relaying mail to another, not for a program
submitting mail, so it isn't used by default. Specify it explicitly if
that's really what you're doing.

### The Envelope

What the server is told and what the message says are two separate things.
Unless the options say otherwise, the envelope is taken from the message:
the sender from `From`, and the recipients from `To`, `Cc` and `Bcc`
combined, with duplicates removed.

```zuri,ignore
server.send(note, { from: 'bounces@example.com' })
server.send(note, {
  to: ['someone@example.com'],
})
```

The first example is what a mailing list does: the message says who wrote
it, and the envelope says where bounces should go. The second one delivers
to an address that the headers don't mention at all, which is how a blind
copy actually works.

An empty sender is the null path, which is the sender used for bounce
messages, so that a bounce can't itself bounce:

```zuri,ignore
server.send(bounce, { from: '' })
```

### TLS

By default, a client connects, reads the server's capabilities and
negotiates TLS before it sends anything that needs protecting:

| `tls` | What happens |
| --- | --- |
| `require` | Negotiates TLS, and refuses to continue without it. This is the default. |
| `prefer` | Negotiates TLS if the server offers it |
| `disable` | Doesn't ask for TLS |

`smtps://`, `imaps://` and `pop3s://` perform the TLS handshake before the
first byte instead, so `tls` has no effect with them.

Whatever `tls` is set to, a mechanism that sends the password itself is
never used on a connection that isn't encrypted. If the server offers
nothing else, the client raises an error instead of sending the password.
This can't be configured, and it's the one place where the module won't do
what it's told.

To trust a certificate that the system doesn't, pass a TLS configuration:

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

Put the credentials in the options, and the client chooses the strongest
mechanism that both sides support:

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

If you provide a token, a token-based mechanism is chosen. To use a specific
mechanism instead of letting the client choose, pass `mechanisms`. How the
choice is made is described in [Authentication
Mechanisms](#authentication-mechanisms).

A username and password in the connection string also work, which is
convenient when the string comes from configuration:

```zuri,ignore
mail.send('smtp://reports:secret@mail.example.com', note, nil)
```

### Server Capabilities

```zuri,ignore
echo server.capabilities().keys()
echo server.max_size()
```

If a message is larger than the server says it accepts, it's rejected before
it's sent instead of after it's been uploaded, which matters when the
message is the reason the connection is slow.

If the server supports `CHUNKING`, `send()` can send the message in pieces
without escaping anything:

```zuri,ignore
server.send(note, { chunking: true })
```

If the server supports `SIZE`, `8BITMIME` or `DSN`, those extensions are
used automatically. If it doesn't, the message is still sent.

### Handling Failures

If the server rejects something partway through a transaction, it's left
holding half of one. `send()` cancels the transaction before raising an
error, so the connection can still be used for the next message, instead of
the server responding with `503` to everything that follows. This is easy to
forget if you do it yourself:

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

Every message after the failed one is still sent.

## Reading Mail with IMAP

IMAP keeps mail on the server. A client opens a mailbox, searches it, and
fetches the messages it needs:

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

`select()` opens a mailbox for reading and writing. `examine()` opens it
read-only, which also means that reading a message doesn't mark it as read.
`close_mailbox()` closes the mailbox and removes any messages marked as
deleted, while `unselect()` closes it without removing them.

`uidvalidity` is how a server tells clients that its message numbering has
been reset. A client that remembers message identifiers between sessions has
to check it, because if it has changed, every identifier the client
remembers now refers to something else.

To list the mailboxes that exist:

```zuri,ignore
for box in inbox.list('', '*') {
  echo '${box.name} ${box.is_selectable() ? '' : '(container only)'}'
}
```

`*` matches anything, including the separator between levels of the
hierarchy. `%` matches anything except the separator, which lets you list a
single level. `status()` asks what's in a mailbox without opening it, which
is how a client can show unread counts for a dozen folders without selecting
each one:

```zuri,ignore
echo inbox.status('Archive', ['MESSAGES', 'UNSEEN'])
```

You give and get mailbox names in the form a person would read them. IMAP
writes names in a modified version of UTF-7, so `Entwürfe` is sent as
`Entw&APw-rfe` and `Tom & Jerry` as `Tom &- Jerry`. The client converts
names for every command that names a mailbox. After
`enable(['UTF8=ACCEPT'])` on a server that supports it, names are sent as
UTF-8 instead, and `enabled` lists what the server agreed to turn on.
`encoding.encode_mailbox_name()` and `decode_mailbox_name()` perform the
same conversion, for a program that talks to the server in some other way,
such as through `run()`.

### Searching

The search criteria are IMAP's own, and they read almost like English:

```zuri,ignore
inbox.search('UNSEEN', true)
inbox.search('FROM ada@example.com SINCE 1-Jan-2026', true)
inbox.search('SUBJECT engines LARGER 10000', true)
inbox.search('FLAGGED UNDELETED', true)
```

The second argument asks for unique identifiers instead of positions. A
position is only valid until something is removed from the mailbox and every
message after it is renumbered. An identifier stays valid after the
connection closes, and it's what a program that runs more than once should
remember. Pass `true` unless you're going to use the numbers straight away.

### Fetching Only What You Need

Fetching every message just to show a list of them is exactly the mistake
IMAP was designed to prevent:

```zuri,ignore
for info in inbox.fetch('1:50', 'ENVELOPE FLAGS RFC822.SIZE', false) {
  echo '${info.envelope.subject} (${info.size} bytes)'
}
```

The envelope contains the addresses and the date, already parsed by the
server. No message bodies were sent over the network at all.

| What to ask for | What you get |
| --- | --- |
| `ENVELOPE` | The addresses, subject and date |
| `FLAGS` | What has been done to the message |
| `RFC822.SIZE` | The size of the message |
| `INTERNALDATE` | When the server received it |
| `BODYSTRUCTURE` | The structure of the message, part by part |
| `BODY.PEEK[HEADER]` | The block of headers |
| `BODY.PEEK[]` | The whole message |
| `BODY.PEEK[2]` | A single part of it |

`BODYSTRUCTURE` is the one worth knowing about. It describes what the
message is made of without sending any of it, so a client can choose to
fetch the text and leave a large attachment on the server:

```zuri,ignore
var structure = inbox.fetch(uid, 'BODYSTRUCTURE', true)[0].structure

for part in structure.walk() {
  echo '${part.section} ${part.mime_type()} ${part.size}'
}
```

`section` is the number you use to request that part on its own.

`fetch_headers()` is a shortcut for requesting the block of headers, and
`fetch_message()` is a shortcut for requesting the whole message:

```zuri,ignore
inbox.fetch_message(uid, true, false) # leaves it unread
inbox.fetch_message(uid, true, true) # marks it read
```

Reading a message doesn't mark it as read unless you ask for that. A program
working through a mailbox shouldn't change what a person sees the next time
they open it.

### Flags

A flag records what has been done to a message:

```zuri,ignore
inbox.add_flags(uid, ['\\Seen'], true)
inbox.remove_flags(uid, ['\\Flagged'], true)
inbox.mark_seen(uid, true)
inbox.mark_deleted(uid, true)
```

`\Seen`, `\Answered`, `\Flagged`, `\Deleted` and `\Draft` have defined
meanings. A server can allow other flags, and lists them in the mailbox's
`permanent_flags`.

`mark_deleted()` only marks messages. Nothing is removed until you call
`expunge()`:

```zuri,ignore
inbox.mark_deleted(uid, true)
echo inbox.expunge()
```

`expunge()` returns the positions of the removed messages, highest first,
because each removal renumbers every message after it, and that's the order
in which they have to be applied.

### Moving, Copying and Removing

```zuri,ignore
inbox.copy(uid, 'Archive', true)
inbox.move(uid, 'Archive', true)
```

`move()` uses the server's own `MOVE` command if it has one. Otherwise, it
does what `MOVE` was invented to replace: it copies the message, marks it as
deleted and expunges it. Either way, the message ends up in one place.

`create()`, `delete()`, `rename()`, `subscribe()` and `unsubscribe()` do
what their names say.

### Adding a Message to a Mailbox

`append()` adds a message to a mailbox without sending it anywhere, which is
how a sent message gets into the Sent folder:

```zuri,ignore
inbox.append('Sent', note, ['\\Seen'], nil)
```

The flags are the ones to store it with, and `\Seen` is the usual choice for
a message the account sent itself. The last argument is the internal date,
and if you don't provide it, the time of arrival is used.

### Waiting for New Mail

`idle()` waits for the server to report something, instead of asking
repeatedly:

```zuri,ignore
inbox.on_event(@(response) {
  echo 'the server said ${response.name()}'
})

while true {
  inbox.idle(1500000)
}
```

A server might drop a connection that has been idle for too long, which is
why the wait has a time limit and the loop starts it again. The default is
25 minutes, which is what RFC 2177 recommends.

`noop()` does the same thing without waiting. It gives the server a chance
to report any changes, and stops the connection from becoming idle at all.

## Collecting Mail with POP3

POP3 hands over the mail and forgets about it. There's no searching and
there are no folders, just a numbered list of messages and a way to remove
them:

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

`list()` returns every message with its size and, if the server provides
one, an identifier that stays valid after the session ends. The message
number doesn't. It's only valid until something is removed and the remaining
messages are renumbered.

Nothing is actually removed until you call `quit()`. `delete()` only marks
messages, and `reset()` removes all the marks. If a connection drops partway
through, the mailbox is left exactly as it was, which protects you from a
program that fails in the middle. It also means that closing the connection
without calling `quit()` is how you cancel a run.

`top()` fetches the headers and the first few lines of the body, which is
enough to decide whether the rest is worth fetching:

```zuri,ignore
var preview = mailbox.top(entry.number, 5)
```

If the server's greeting offers `APOP`, it's used instead of sending the
password, and if the server supports SASL, those mechanisms are preferred
over both. `USER`/`PASS` is the last resort, and it's only used over TLS.

## Running an SMTP Server

`SmtpServer` implements the protocol but has no policy of its own. Handlers
decide what to accept:

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

| Handler | Called with | When |
| --- | --- | --- |
| `on_connect` | The session | A connection opens, before the greeting |
| `on_auth` | The session and the credentials | A client authenticates |
| `on_mail` | The session and the sender | A transaction starts |
| `on_rcpt` | The session and a recipient | Once for each recipient |
| `on_data` | The session and the message | The whole message has arrived |
| `on_close` | The session | The connection closes, however it was closed |
| `on_error` | The error and the session | Something inside the server failed |

The session holds what's known so far: who connected, whether the connection
is encrypted, who authenticated, the sender, and the recipients accepted so
far. `session.state` is an empty dictionary where your handlers can store
anything, and it lasts as long as the connection.

### Rejecting Mail

A handler that returns nothing accepts. A handler that returns a code and a
message rejects with that code and message:

```zuri,ignore
server.on_mail(@(session, sender) {
  if blocklist.contains(sender.domain) {
    return { code: 550, message: 'not accepted from there', enhanced: '5.7.1' }
  }
})
```

A handler that raises an error indicates a failure on the server's side, not
a bad message, so the sender is told `451` and asked to try again later.
This distinction matters, because a database that's temporarily down
shouldn't cause mail to bounce.

Rejecting in `on_rcpt` is the most useful option. It tells the sender
straight away which address is wrong, while it's still connected and can do
something about it, instead of accepting the message and then sending a
bounce to an address that might not exist either.

The null sender, which is what bounces are sent from, reaches `on_mail` as
an empty string rather than an address. If you reject mail from it, your
server won't be able to receive bounces, so handle it deliberately.

### TLS and Authentication

```zuri,ignore
server.use_tls(file('cert.pem').read(), file('key.pem').read())
```

That's what allows the server to offer `STARTTLS`. Everything a client said
before the handshake is discarded afterwards, including who it claimed to
be, because none of it was protected.

Authentication needs one handler, and a second one for the mechanisms that
prove a password without sending it:

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

`on_password` is what makes `CRAM-MD5` possible. Checking the proof means
calculating it again, which means knowing the password. A server that can't
provide the password doesn't advertise the mechanism. Without `on_auth`, the
server doesn't advertise any authentication at all.

`PLAIN` and `LOGIN` are only advertised once the connection is encrypted.
`require_auth` rejects mail from a client that hasn't authenticated, and
`require_tls` rejects mail on a connection that isn't encrypted.

### Limits

| Option | Default | What it does |
| --- | --- | --- |
| `max_size` | 35 MB | The largest message to accept |
| `max_recipients` | 100 | The number of recipients a single message can have |
| `timeout` | 300000 | How many milliseconds a client can stay silent |

A client that declares a size over the limit is rejected at `MAIL`, before
it uploads anything. A client that doesn't declare a size is rejected when
it goes over the limit. After ten invalid commands in a row, the connection
is dropped.

## Running an IMAP Server

`ImapServer` runs the session state machine and answers every command from a
`MailStore`. Which mail exists and where it's kept is up to the store:

```zuri,ignore
import mail.imap { ImapServer, MaildirStore }

var store = MaildirStore('/var/mail')

store.add_account('ada', secret)

ImapServer({ port: 143 }, store).listen()
```

The server implements IMAP4rev1, along with `UNSELECT`, `MOVE`, `IDLE`,
`LITERAL+`, `SASL-IR` and `ID`. It advertises exactly what it implements, so
a client that reads the capability list and trusts it won't run into
problems.

### Where the Mail Is Stored

Two stores are included. `MemoryStore` keeps everything in memory and
forgets it when the process exits. That's what a test needs, and also what a
server embedded in a larger program needs when the mail itself isn't
important:

```zuri
import mail
import mail.imap { MemoryStore }

var store = MemoryStore(['INBOX', 'Archive'])

store.add_account('ada', 'secret')
store.append(
  'ada',
  'INBOX',
  mail
      .message()
      .set_from('grace@example.com')
      .add_to('ada@example.com')
      .set_subject('On Bugs')
      .set_text('Found one.')
      .to_bytes(),
  nil,
  nil,
)

echo store.mailboxes('ada')
echo store.messages('ada', 'INBOX').length()
echo store.messages('ada', 'INBOX')[0].uid
```

```console
[Archive, INBOX]
1
1
```

`MaildirStore` implements the same interface on disk.

### Maildir

Each account is a directory that holds its INBOX directly, with every other
mailbox next to it under a name starting with a dot. This is the Maildir++
layout, which other mail tools understand. A mailbox written by this server
can be read by other tools, and mail delivered by other tools shows up here.

Every mailbox consists of the three directories that Maildir defines. A
message is first written into `tmp`, which nothing reads from, and only
moved into place once it's complete, so a reader never sees half a message.
`new` is where a delivery agent puts mail that nobody has looked at yet, and
`cur` is where a message is kept once a client has seen the mailbox, with
its flags recorded in the file name after `:2,`. Windows doesn't allow a
colon in a file name, so there the flags come after `;2,` instead, as they
do in mbsync. An account on disk looks like this:

```text
ada/
  cur/   new/   tmp/   zuri-uidlist
  .Archive/
    cur/   new/   tmp/   zuri-uidlist
```

That compatibility is the reason to use it. A delivery agent can drop a
message in, and the server finds it the next time it looks, without a shared
database or any protocol between them.

IMAP needs a message number that stays the same when a file is renamed, and
Maildir doesn't have one, so each mailbox keeps a small index file next to
its directories that records which file has which number. When a message's
file disappears, its number is retired instead of being reused, so a client
that remembers the number is told the message is missing instead of being
given a different one.

Mail is stored on disk, but accounts aren't:

```zuri,ignore
store.add_account('ada', secret)

store.set_authenticator(@(username, password) {
  return accounts.verify(username, password)
})
```

Where an application keeps its passwords is up to the application, not the
mail library. A store with an authenticator can't provide a password, so the
server stops advertising the mechanisms that need one.

### Writing Your Own Store

A store only has to implement the same methods. Nothing in the interface
requires files:

| Method | Purpose |
| --- | --- |
| `authenticate(username, password)` | Checks whether the credentials are correct |
| `password_of(username)` | Returns the password, for the mechanisms that need it |
| `has_passwords()` | Whether the store can provide passwords at all |
| `mailboxes(username)` | Lists the mailboxes |
| `exists`, `create`, `remove`, `rename` | Manage the mailboxes themselves |
| `messages(username, mailbox)` | Returns every message in a mailbox, oldest first |
| `append(username, mailbox, raw, flags, received)` | Adds a message |
| `set_flags(username, mailbox, uid, flags)` | Changes a message's flags |
| `expunge(username, mailbox)` | Removes the messages marked as deleted |
| `counters(username, mailbox)` | Returns `uidnext` and `uidvalidity` |

Subclass `MailStore` and implement these methods, and an `ImapServer` can
serve whatever data is behind them.

## Serving Several Connections

Both servers handle one connection until it ends before accepting the next.
That suits the protocols, but it doesn't work for more than one client at a
time. `mail.pool` puts a pool of isolates behind a single listening socket:

```zuri,ignore
import mail.pool
import .my_server

pool.serve(my_server.build, { host: '0.0.0.0', port: 143, workers: 8 })
```

`build` runs inside each worker to create that worker's own server, because
isolates don't share anything and each one needs its own server. It can be
any function. Keeping it in a separate module is just a convenient place to
put each worker's setup:

```zuri,ignore
# my_server.zu
import mail.imap { ImapServer, MaildirStore }

def build() {
  var store = MaildirStore('/var/mail')

  store.set_authenticator(accounts.verify)

  return ImapServer({}, store)
}
```

An IMAP connection can stay open for hours, so the pool size limits how many
clients can be served at once, rather than how quickly they're served. Size
the pool for the number of clients, not the rate of requests.

`pool.start()` does everything except running the accept loop. Use it when
you need to know the address before the first connection, as you would in a
test.

## Verifying Where Mail Came From

A receiving server has no reason to trust a `From` header. With DKIM, the
sending domain signs the message when it's sent, and the receiver checks the
signature against a key published in that domain's own DNS.

### Signing

```zuri,ignore
import mail.dkim { Signer }

var signer = Signer('example.com', 'default', private_key)

signer.sign(note)
```

The signature covers the body and the most important headers, and the header
it adds goes at the top. Sign the message last, once nothing else about it
will change. Changing a signed header afterwards breaks the signature.
`mail.send()` sets the `Message-ID` and `Date` headers if they're missing,
so either let it do that or set them yourself before signing.

| Option | Default | What it does |
| --- | --- | --- |
| `algorithm` | `rsa-sha256` | The signing algorithm. The alternative is `ed25519-sha256`. |
| `canonicalisation` | `relaxed/relaxed` | The canonicalization for the headers and then the body |
| `headers` | A sensible set | Which headers to sign |
| `expires_in` | None | The number of seconds until the signature expires |

`relaxed` tolerates the changes to whitespace and line folding that a mail
server might make along the way. `simple` covers the exact bytes, so any
change at all breaks the signature. Both are supported. `relaxed/relaxed` is
what almost everyone uses, which is why it's the default.

The public key goes in DNS under the selector:

```console
default._domainkey.example.com  TXT  "v=DKIM1; k=rsa; p=MIIBIjANBg..."
```

### Verifying Signatures

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

You get one result for each signature, in the order they appear in the
message. A message without any signatures gives an empty list. That isn't a
failure, it just means nobody signed the message.

The key is looked up through `net.resolver`, unless you pass `verify()`
something else to look it up with. That's useful for a program with its own
resolver or its own cache:

```zuri,ignore
dkim.verify(incoming, @(name) {
  return my_dns.text_records(name)
})
```

A parsed message is verified against the bytes it was parsed from, because
that's the only thing a signature can be checked against. If you change a
parsed message and verify it again, the result describes the message as it
arrived, not the one you now have.

### What a Signature Does Not Say

A valid signature means the domain vouches for the message. It doesn't mean
the message is wanted, that the `From` header matches the signing domain, or
that the sender is who they appear to be to a human reader. Deciding how
much a signature from a particular domain is worth is a separate question,
and the answer is usually DMARC.

DNSSEC isn't validated. DNSSEC signatures are passed along when a server
sends them and the `dnssec` option asks for them, but nothing here checks
them. A program that needs a validated answer needs a validating resolver
and a trusted connection to it, which is what the `tls` option of
`net.resolver` provides.

## Authentication Mechanisms

All three protocols use the same authentication mechanisms, and `mail.sasl`
implements them once rather than three times. A client is given credentials
and chooses the strongest mechanism that both sides support:

| Mechanism | What it does |
| --- | --- |
| `SCRAM-SHA-256`, `SCRAM-SHA-1` | Proves the password without sending it, and also proves that the server knew it |
| `CRAM-MD5` | Proves the password without sending it, but proves nothing about the server |
| `XOAUTH2`, `OAUTHBEARER` | Sends a bearer token, which is what the large providers require |
| `PLAIN`, `LOGIN` | Sends the password itself, so they're never used without TLS |
| `EXTERNAL` | Sends nothing, because the client certificate already identifies the client |

Mechanisms are preferred in the order of that table. Use SCRAM wherever a
server supports it. The server stores something derived from the password
rather than the password itself, the client never sends the password, and
the exchange ends with the server proving that it knew the password too,
which protects against a server that simply accepts everything.

Channel binding, the `-PLUS` variant of SCRAM, isn't supported. It needs a
value from the TLS session that `net.tls` doesn't expose.

To use a specific mechanism instead of letting the client choose:

```zuri,ignore
SmtpClient.connect(url, {
  username: 'reports',
  password: secret,
  mechanisms: ['SCRAM-SHA-256'],
})
```

## Errors

Every error is a `MailError`, so catching that catches every error the
module raises itself.

| Error | Raised when |
| --- | --- |
| `MessageError` | The bytes aren't a message, or the message contradicts itself |
| `ProtocolError` | The server sent something the protocol doesn't allow |
| `ConnectionClosed` | The connection closed in the middle of a conversation |
| `AuthenticationError` | The credentials were rejected, or no usable mechanism was offered |
| `StateError` | A command was used somewhere it doesn't make sense |
| `MailboxError` | A mailbox doesn't exist or can't be created |
| `SmtpError` | The SMTP server rejected something, with a reply code |
| `ImapError` | The IMAP server responded with `NO` or `BAD` |
| `Pop3Error` | The POP3 server responded with `-ERR` |

The distinction worth designing a mail program around is the one SMTP makes:

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

A 4xx reply means the server couldn't accept the message right now, and the
sender should try again later. A 5xx reply means it won't accept the
message, and trying again won't change that. Queue messages that get the
first and bounce messages that get the second. Treating them the same is how
a mail queue either loses mail or sends it forty times.

`SmtpError` has a `code`, the three-digit reply code, and an `enhanced`
code, the more detailed code from RFC 3463, when the server sends one. Only
rely on the first digit of either.

`ImapError` has a `status`, which is `NO` when the server understood the
command but refused it, and `BAD` when it didn't understand the command at
all. `BAD` points to a problem with the client rather than with the request.

## Design Limits

**It never sends a password over an unencrypted connection.** There's no
option or flag to change this. If the only mechanisms available would send
the password unprotected, the client raises an error instead.
`tls: 'disable'` turns off TLS negotiation, but it doesn't turn off this
check.

**It never writes a character set it can't encode.** Asking for a body in
ISO 8859-1 raises an error, instead of writing UTF-8 bytes under a header
that claims otherwise.

**It doesn't validate DNSSEC.** See [What a Signature Does Not
Say](#what-a-signature-does-not-say).

**It doesn't implement a POP3 server.** A POP3 server is an IMAP server with
almost everything removed, and running `mail.imap` is the better option.

**It doesn't decide whether mail is wanted.** There's no spam filtering,
reputation system or policy. DKIM tells you who signed a message, and what
you do with that information is up to you.

## Module Reference

The standard library reference documents every class and method. Here's an
overview of the module:

| Function | Description |
| --- | --- |
| `mail.message()` | Starts a new message |
| `mail.parse(data)` | Parses a message |
| `mail.send(url, note, options)` | Sends a message, handling the connection for you |
| `mail.smtp(url, options)` | Connects to a server that sends mail |
| `mail.imap(url, options)` | Connects to a server that stores mail |
| `mail.pop3(url, options)` | Connects to a server that hands over mail |
| `mail.parse_address(text)` | Parses a single address |
| `mail.parse_address_list(text)` | Parses every address in a header |
| `mail.format_addresses(people)` | Formats a list of addresses |
| `mail.attachment(data)` | Starts a new attachment |
| `mail.Attachment.from_file(path)` | Creates an attachment from a file on disk |

Methods of an `Attachment`:

| Methods | Purpose |
| --- | --- |
| `set_filename`, `set_content_type`, `set_encoding` | Describe what it is |
| `set_disposition`, `set_cid`, `set_description` | Describe how it's presented |
| `filename`, `content_type`, `disposition`, `cid`, `size`, `data` | Read these values back |
| `to_part` | Returns the message part it becomes |

Methods of a `Message`:

| Methods | Purpose |
| --- | --- |
| `set_from`, `set_sender`, `set_reply_to` | Who it's from |
| `add_to`, `add_cc`, `add_bcc`, `set_to`, `set_cc`, `set_bcc` | Who it's for |
| `set_subject`, `set_text`, `set_html` | What it says |
| `attach`, `embed` | What it carries, as an `Attachment` |
| `set_header`, `add_header`, `set_date`, `set_message_id` | Everything else |
| `set_in_reply_to`, `set_references` | Which conversation it belongs to |
| `sender`, `to`, `cc`, `bcc`, `reply_to`, `recipients` | Read the addresses back |
| `subject`, `text`, `text_body`, `html_body` | Read the contents back |
| `parts`, `walk`, `find_part`, `attachments` | Work with the message tree |
| `to_bytes`, `to_string` | Write it out |

The protocol modules:

| Module | Contents |
| --- | --- |
| `mail.smtp` | `SmtpClient`, `SmtpServer`, `SmtpSession`, `Reply` |
| `mail.imap` | `ImapClient`, `ImapServer`, `Mailbox`, `Envelope`, `BodyPart`, `MessageInfo` |
| `mail.imap` | `MailStore`, `MaildirStore`, `MemoryStore`, `StoredMessage` |
| `mail.pop3` | `Pop3Client`, `Entry` |
| `mail.dkim` | `Signer`, `Signature`, `Result`, `verify`, `is_signed` |
| `mail.sasl` | `of`, `choose`, and a class for each mechanism |
| `mail.pool` | `serve`, `start`, `Cluster` |

And the lower-level modules, for programs that need them directly:

| Module | Contents |
| --- | --- |
| `mail.address` | `parse`, `parse_list`, `parse_groups`, `format_list` |
| `mail.headers` | `Headers`, `fold`, `unfold` |
| `mail.encoding` | The transfer encodings, encoded words and parameters |
| `mail.content` | `ContentType`, `ContentDisposition` |
| `mail.stream` | `LineStream`, `connect`, `start_tls`, `endpoint` |
| `mail.imap.parser` | The IMAP grammar, for reading a response yourself |
