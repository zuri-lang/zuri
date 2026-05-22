# Parser state class

import ..common.utils { is_space }
import ..token as _tkn

class BlockState {
  BlockState(src, md, env, tokens) {
    var ch, s, start, pos, len, indent, offset, indent_found
  
    self.src = src
  
    # link to parser instance
    self.md     = md
  
    self.env = env
  
    #
    # Internal state variables
    #
  
    self.tokens = tokens
  
    self.b_marks = []  # line begin offsets for fast jumps
    self.e_marks = []  # line end offsets for fast jumps
    self.t_shift = []  # offsets of the first non-space characters (tabs not expanded)
    self.s_count = []  # indents for each line (tabs expanded)
  
    # An amount of virtual spaces (tabs expanded) between beginning
    # of each line (b_marks) and real beginning of that line.
    #
    # It exists only as a hack because blockquotes override b_marks
    # losing information in the process.
    #
    # It's used only when expanding tabs, you can think about it as
    # an initial tab length, e.g. bs_count=21 applied to string `\t123`
    # means first tab should be expanded to 4-21%4 == 3 spaces.
    #
    self.bs_count = []
  
    # block parser variables
    self.blk_indent  = 0 # required block content indent (for example, if we are
                         # inside a list, it would be positioned after list marker)
    self.line       = 0 # line index in src
    self.line_max    = 0 # lines count
    self.tight      = false  # loose/tight mode for lists
    self.dd_indent   = -1 # indent of the current dd block (-1 if there isn't any)
    self.list_indent = -1 # indent of the current list block (-1 if there isn't any)
  
    # can be 'blockquote', 'list', 'root', 'paragraph' or 'reference'
    # used in lists to determine if they interrupt a paragraph
    self.parent_type = 'root'
  
    self.level = 0
  
    # renderer
    self.result = ''
  
    # Create caches
    # Generate markers.
    s = self.src
    indent_found = false
  
    start = pos = indent = offset = 0
    iter len = s.length(); pos < len; pos++ {
      ch = s[pos]
  
      if !indent_found {
        if is_space(ch) {
          indent++
  
          if ch == '\t' {
            offset += 4 - offset % 4
          } else {
            offset++
          }
          continue
        } else {
          indent_found = true
        }
      }
  
      if ch == '\n' or pos == len - 1 {
        if ch != '\n' pos++
        self.b_marks.append(start)
        self.e_marks.append(pos)
        self.t_shift.append(indent)
        self.s_count.append(offset)
        self.bs_count.append(0)
  
        indent_found = false
        indent = 0
        offset = 0
        start = pos + 1
      }
    }
  
    # Push fake entry to simplify cache bounds checks
    self.b_marks.append(s.length())
    self.e_marks.append(s.length())
    self.t_shift.append(0)
    self.s_count.append(0)
    self.bs_count.append(0)
  
    self.line_max = self.b_marks.length() - 1 # don't count last fake line
  }

  push(type, tag, nesting) {
    var tkn = _tkn.Token(type, tag, nesting)
    tkn.block = true
  
    if nesting < 0 self.level-- # closing tag
    tkn.level = self.level
    if nesting > 0 self.level++ # opening tag
  
    self.tokens.append(tkn)
    return tkn
  }

  is_empty(line) {
    return self.b_marks[line] + self.t_shift[line] >= self.e_marks[line]
  }

  skip_empty_lines(from) {
    iter var max = self.line_max; from < max; from++ {
      if self.b_marks[from] + self.t_shift[from] < self.e_marks[from] {
        break
      }
    }
    return from
  }

  skip_spaces(pos) {
    iter var max = self.src.length(); pos < max; pos++ {
      if !is_space(self.src[pos]) break
    }
    return pos
  }

  skip_spaces_back(pos, min) {
    if pos <= min return pos
  
    while pos > min {
      if !is_space(self.src[pos--]) return pos + 1
    }
    return pos
  }

  skip_chars(pos, code) {
    iter var max = self.src.length(); pos < max; pos++ {
      if self.src[pos] != code break
    }
    return pos
  }

  skip_chars_back(pos, code, min) {
    if pos <= min return pos
  
    while pos > min {
      if code != self.src[pos--] return pos + 1
    }
    return pos
  }
  
  get_lines(begin, end, indent, keep_last_LF) {
    var i, line_indent, ch, first, last, queue, line_start,
        line = begin
  
    if begin >= end {
      return ''
    }
  
    queue = [nil] * (end - begin)
  
    iter i = 0; line < end; i++ {
      line_indent = 0
      line_start = first = self.b_marks[line]
  
      if line + 1 < end or keep_last_LF {
        # No need for bounds check because we have fake entry on tail.
        last = self.e_marks[line] + 1
      } else {
        last = self.e_marks[line]
      }
  
      while first < last and line_indent < indent {
        ch = self.src[first]
  
        if is_space(ch) {
          if ch == '\t' {
            line_indent += 4 - (line_indent + self.bs_count[line]) % 4
          } else {
            line_indent++
          }
        } else if first - line_start < self.t_shift[line] {
          # patched t_shift masked characters to look like spaces (blockquotes, list markers)
          line_indent++
        } else {
          break
        }
  
        first++
      }
  
      if line_indent > indent {
        # partially expanding tabs in code blocks, e.g '\t\tfoobar'
        # with indent=2 becomes '  \tfoobar'
        queue[i] = (' ' * (line_indent - indent)) + self.src[first, last]
      } else {
        queue[i] = self.src[first, last]
      }

      line++
    }
  
    return ''.join(queue)
  }

  var Token = _tkn.Token
}

