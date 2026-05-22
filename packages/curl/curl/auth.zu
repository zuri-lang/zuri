#!-- part of the curl module

import _curl

/**
 * Auth values for `Options.HTTPAUTH` and `Options.PROXYAUTH`
 */
class Auth {
  
  /**
   * No HTTP authentication
   * @static
   */
  static var NONE = _curl.CURLAUTH_NONE

  /**
   * HTTP Basic authentication (default)
   * @static
   */
  static var BASIC = _curl.CURLAUTH_BASIC

  /**
   * HTTP Digest authentication
   * @static
   */
  static var DIGEST = _curl.CURLAUTH_DIGEST

  /**
   * HTTP Negotiate (SPNEGO) authentication
   * @static
   */
  static var NEGOTIATE = _curl.CURLAUTH_NEGOTIATE

  /**
   * Alias for CURLAUTH_NEGOTIATE (deprecated)
   * @static
   */
  static var GSSNEGOTIATE = _curl.CURLAUTH_GSSNEGOTIATE

  /**
   * HTTP NTLM authentication
   * @static
   */
  static var NTLM = _curl.CURLAUTH_NTLM

  /**
   * HTTP Digest authentication with IE flavour
   * @static
   */
  static var DIGEST_IE = _curl.CURLAUTH_DIGEST_IE

  /**
   * HTTP NTLM authentication delegated to winbind helper
   * @static
   */
  static var NTLM_WB = _curl.CURLAUTH_NTLM_WB

  /**
   * HTTP Bearer token authentication
   * @static
   */
  static var BEARER = _curl.CURLAUTH_BEARER

  /**
   * Use together with a single other type to force no authentication or 
   * just that single type.
   * @static
   */
  static var ONLY = _curl.CURLAUTH_ONLY
          
  /**
   * All fine types set
   * @static
   */
  static var ANY = _curl.CURLAUTH_ANY

  /**
   * All fine types except Basic
   * @static
   */
  static var ANYSAFE = _curl.CURLAUTH_ANYSAFE

  # SSH AUTH

  /**
   * All types of SSH authentication supported by the server
   * @static
   */
  static var SSH_ANY = _curl.CURLSSH_AUTH_ANY

  /**
   * No SSH allowed
   * @static
   */
  static var SSH_NONE = _curl.CURLSSH_AUTH_NONE

  /**
   * Public/private key files for SSH authentication.
   * @static
   */
  static var SSH_PUBLICKEY = _curl.CURLSSH_AUTH_PUBLICKEY

  /**
   * Password for SSH authentication.
   * @static
   */
  static var SSH_PASSWORD = _curl.CURLSSH_AUTH_PASSWORD

  /**
   * Host key files for SSH authentication.
   * @static
   */
  static var SSH_HOST = _curl.CURLSSH_AUTH_HOST

  /**
   * Keyboard interactive SSH authentication.
   * @static
   */
  static var SSH_KEYBOARD = _curl.CURLSSH_AUTH_KEYBOARD

  /**
   * Agent (ssh-agent, pageant, etc.) for SSH authentication.
   * @static
   */
  static var SSH_AGENT = _curl.CURLSSH_AUTH_AGENT

  /**
   * gssapi (kerberos, etc.) for SSH authentication.
   * @static
   */
  static var SSH_GSSAPI = _curl.CURLSSH_AUTH_GSSAPI

  /**
   * The default SSH authentication (same as ANY).
   * @static
   */
  static var SSH_DEFAULT = _curl.CURLSSH_AUTH_DEFAULT
}
