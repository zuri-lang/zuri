import os
import date
import curl {
  CurlMime
}

/**
 * Message class can be used to construct an email message. 
 * 
 * The class has several methods that can be used to set various properties 
 * of the email message.
 */
class Message {
  var _from = ''
  var _from_name = ''
  var _to = []
  var _cc = []
  var _bcc = []
  var _subject = ''
  var _headers = []
  var _attachments = {}
  var _text = ''
  var _html = ''

  /**
   * @constructor
   */
  Message() {
    # mime will be used by smtp and therefore is not to be 
    # exposed or documented outside.
    self.mime = nil
  }

  /**
   * Set the sender of the email message.
   * 
   * @param string from
   * @returns self
   */
  from(from) {
    if !self._from {
      if from.match('/\s+<[^>]+>/') {
        var split = from.split('<')
        self._from_name = split[0]
        self._from = '<' + split[1].trim()
      } else {
        self._from = from
      }
      return self
    }
    raise Exception('from() can only be called once.')
  }

  /**
   * Add one or more recipients to the email message.
   * 
   * @param string|list[string] to
   * @returns self
   */
  to(to) {
    if is_string(to) self._to.append(to)
    else self._to.extend(to)
    return self
  }

  /**
   * Add one or more Cc recipients to the email message.
   * 
   * @param string|list[string] cc
   * @returns self
   */
  cc(cc) {
    if is_string(cc) self._cc.append(cc)
    else self._cc.extend(cc)
    return self
  }

  /**
   * Add one or more Bcc recipients to the email message.
   * 
   * @param string|list[string] bcc
   * @returns self
   */
  bcc(bcc) {
    if is_string(bcc) self._bcc.append(bcc)
    else self._bcc.extend(bcc)
    return self
  }

  /**
   * Add a reply-to address to the email message.
   * 
   * @param string to
   * @returns self
   */
  reply_to(to) {
    self._headers.append('Reply-To: ${to}')
    return self
  }

  /**
   * Set the subject of the email message.
   * 
   * @param string subject
   * @returns self
   */
  subject(subject) {
    self._subject = subject
    return self
  }

  /**
   * Add one or more headers to the email message.
   * 
   * @param string|list|dict header
   * @returns self
   */
  header(header) {
    if is_string(header) {
      self._headers.append(header)
    } else if is_list(header) {
      self._headers.extend(header)
    } else {
      for key, value in header {
        self._headers.append('${key}: ${value}')
      }
    }
    return self
  }

  /**
   * Add one or more attachments to the email message.
   * 
   * @param string path
   * @param string name (Optional)
   * @returns self
   */
  attachment(path, name) {
    if !name name = os.base_name(path)
    self._attachments.set(name, path)
    return self
  }

  /**
   * Set the plain text body of the email message. 
   * 
   * @param string text
   * @returns self
   */
  text(text) {
    self._text = text
    return self
  }

  /**
   * Set the html body of the email.
   * 
   * @param string html
   * @returns self
   */
  html(html) {
    self._html = html
    return self
  }

  # leave undocumented since its used internally.
  build(curl) {
    var headers = ['Date: ${date().format("r")}']
    headers.append('From: ${self._from_name}${self._from}')

    # set addresses
    headers.append('To: ${", ".join(self._to)}')
    headers.append('Cc: ${", ".join(self._cc)}')
    headers.append('Bcc: ${", ".join(self._bcc)}')

    # add the user headers
    headers.extend(self._headers)

    # set subject
    if self._subject {
      headers.append('Subject: ${self._subject}')
    }

    var mime = CurlMime(curl)
    var alt = CurlMime(curl)
    if self._text {
      alt.add_data(self._text)
    } else {
      # generate text from html
      alt.add_data(self._html.replace('/<br\/?>/', '\n').replace('/<[^>]+>/', ''))
    }

    if self._html {
      alt.add_as(self._html, 'text/html')
      # replace by an alternative mime
      mime.add_mime(alt, 'multipart/alternative')
    } else {
      mime = alt
    }

    for name, path in self._attachments {
      if !file(path).exists() or os.dir_exists(path)
        raise Exception('file "${path}" not found')
      mime.add_file(name, path)
    }

    return {
      headers: headers,
      mime: mime,
      from: self._from,
      to: self._to + self._cc,
    }
  }
}


/**
 * Returns a new instance of [[mail.Message]].
 * 
 * @returns Message
 * @default
 */
def message() {
  return Message()
}
