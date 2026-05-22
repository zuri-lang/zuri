#!-- part of the html module

import os

var DEFAULT_ROOT_DIR = os.join_paths(os.cwd(), 'templates')

var VAR_RE = '/(?<!%)\{\{\s*(?P<variable>([a-z_][a-z0-9_\-|="\']*(\.[a-z0-9_\-|=]+((["\'])([^\'"]+)\5)?)*))\s*\}\}/i'
var FUNCTION_RE = '/(?<!%)\{\!\s*(?P<fn>[a-z0-9_]+)\s*\!\}/i'
var EXT_RE = '/[.][a-zA-Z]+$/'
var COMMENT_RE = '/(?=<!--)([\s\S]*?)-->\\n*/m'
var NUMBER_RE = '/^[-]?\d+$/'
var NUMBER_WITH_DECIMAL_RE = '/^[-]?\d+([.]\d+)$/'
var QUOTE_VALUE_RE = '/([\'"]).*\\1/'

var IF_ATTR = 'x-if'
var NOT_ATTR = 'x-not'
var FOR_ATTR = 'x-for'
var KEY_ATTR = 'x-key'
var VALUE_ATTR = 'x-value'
var PATH_ATTR = 'x-path'
var BASE_ATTR = 'base'
var NAME_ATTR = 'name'
var OVERRIDE_ATTR = 'override'

var INCLUDE_TAG = 'include'
var EXTEND_TAG = 'extend'
var DECLARE_TAG = 'declare'
var DEFINE_TAG = 'define'
var DEFAULT_EXT = '.html'
