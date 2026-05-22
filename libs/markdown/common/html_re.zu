# Regexps to match html elements

var _attr_name     = '[a-zA-Z_:][a-zA-Z0-9:._-]*'

var _unquoted      = '[^"\'=<>`\\x00-\\x20]+'
var _single_quoted = "'[^']*'"
var _double_quoted = '"[^"]*"'

var _attr_value  = '(?:' + _unquoted + '|' + _single_quoted + '|' + _double_quoted + ')'

var _attribute   = '(?:\\s+' + _attr_name + '(?:\\s*=\\s*' + _attr_value + ')?)'

var _open_tag    = '<[A-Za-z][A-Za-z0-9\\-]*' + _attribute + '*\\s*\\/?>'

var _close_tag   = '<\\/[A-Za-z][A-Za-z0-9\\-]*\\s*>'
var _comment     = '<!---->|<!--(?:-?[^>-])(?:-?[^-])*-->'
var _processing  = '<[?][\\s\\S]*?[?]>'
var _declaration = '<![A-Z]+\\s+[^>]*>'
var _cdata       = '<!\\[CDATA\\[[\\s\\S]*?\\]\\]>'

var _LINK_RE = '((?:^|[^a-z0-9.+-])([a-z][a-z0-9.+-]*):\/\/[^\s.]+\.[\w][^\s]+)'
var LINKS_RE = '/${_LINK_RE}/i'
var LINKS_FULL_RE = '/^${_LINK_RE}\$/i'

var HTML_TAG_RE = '/^(?:' + _open_tag + '|' + _close_tag + '|' + _comment +
                        '|' + _processing + '|' + _declaration + '|' + _cdata + ')/'
var HTML_OPEN_CLOSE_TAG_RE = '^(?:' + _open_tag + '|' + _close_tag + ')'
