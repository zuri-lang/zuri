#!-- part of the curl module

import _curl

/**
 * cURL request options for `set_option()`
 */
class Option {

   /**
    * The full URL to get/put 
    * @static
    */
   static var URL = _curl.CURLOPT_URL

   /**
    * Port number to connect to, if other than default. 
    * @static
    */
   static var PORT = _curl.CURLOPT_PORT

   /**
    * Name of proxy to use. 
    * @static
    */
   static var PROXY = _curl.CURLOPT_PROXY

   /**
    * "user:password;options" to use when fetching. 
    * @static
    */
   static var USERPWD = _curl.CURLOPT_USERPWD

   /**
    * "user:password" to use with proxy. 
    * @static
    */
   static var PROXYUSERPWD = _curl.CURLOPT_PROXYUSERPWD

   /**
    * Range to get, specified as an ASCII string. 
    * @static
    */
   static var RANGE = _curl.CURLOPT_RANGE

   /**
    * Buffer to receive error messages in, must be at least CURL_ERROR_SIZE
    * bytes big. 
    * @static
    */
   static var ERRORBUFFER = _curl.CURLOPT_ERRORBUFFER

   /**
    * Time-out the read operation after this amount of seconds 
    * @static
    */
   static var TIMEOUT = _curl.CURLOPT_TIMEOUT

   /**
    * If the CURLOPT_INFILE is used, this can be used to inform libcurl about
    * how large the file being sent really is. That allows better error
    * checking and better verifies that the upload was successful. -1 means
    * unknown size.
    *
    * For large file support, there is also a _LARGE version of the key
    * which takes an off_t type, allowing platforms with larger off_t
    * sizes to handle larger files.  See below for INFILESIZE_LARGE.  
    * @static
    */
   static var INFILESIZE = _curl.CURLOPT_INFILESIZE

   /**
    * POST static input fields. 
    * @static
    */
   static var POSTFIELDS = _curl.CURLOPT_POSTFIELDS

   /**
    * Set the referrer page (needed by some CGIs) 
    * @static
    */
   static var REFERER = _curl.CURLOPT_REFERER

   /**
    * Set the FTP PORT string (interface name, named or numerical IP address)
       Use i.e '-' to use default address. 
   * @static
    */
   static var FTPPORT = _curl.CURLOPT_FTPPORT

   /**
    * Set the User-Agent string (examined by some CGIs) 
    * @static
    */
   static var USERAGENT = _curl.CURLOPT_USERAGENT

   /**
    * Set the "low speed limit" 
    * @static
    */
   static var LOW_SPEED_LIMIT = _curl.CURLOPT_LOW_SPEED_LIMIT

   /**
    * Set the "low speed time" 
    * @static
    */
   static var LOW_SPEED_TIME = _curl.CURLOPT_LOW_SPEED_TIME

   /**
    * Set the continuation offset.
    *
    * Note there is also a _LARGE version of this key which uses
    * off_t types, allowing for large file offsets on platforms which
    * use larger-than-32-bit off_t's.  Look below for RESUME_FROM_LARGE.
    * @static
    */
   static var RESUME_FROM = _curl.CURLOPT_RESUME_FROM

   /**
    * Set cookie in request: 
    * @static
    */
   static var COOKIE = _curl.CURLOPT_COOKIE

   /**
    * This points to a list of HTTP header strings. This
    * list is also used for RTSP (in spite of its name) 
    * @static
    */
   static var HTTPHEADER = _curl.CURLOPT_HTTPHEADER

   /**
    * This points to a linked list of post entries. 
    * @static
    */
   static var HTTPPOST = _curl.CURLOPT_MIMEPOST

   /**
    * name of the file keeping your private SSL-certificate 
    * @static
    */
   static var SSLCERT = _curl.CURLOPT_SSLCERT

   /**
    * password for the SSL or SSH private key 
    * @static
    */
   static var KEYPASSWD = _curl.CURLOPT_KEYPASSWD

   /**
    * send TYPE parameter? 
    * @static
    */
   static var CRLF = _curl.CURLOPT_CRLF

   /**
    * send linked-list of QUOTE commands 
    * @static
    */
   static var QUOTE = _curl.CURLOPT_QUOTE

   /**
    * point to a file to read the initial cookies from, also enables
    * "cookie awareness" 
    * @static
    */
   static var COOKIEFILE = _curl.CURLOPT_COOKIEFILE

   /**
    * What version to specifically try to use.
    * See CURL_SSLVERSION defines below. 
    * @static
    */
   static var SSLVERSION = _curl.CURLOPT_SSLVERSION

   /**
    * What kind of HTTP time condition to use, see defines 
    * @static
    */
   static var TIMECONDITION = _curl.CURLOPT_TIMECONDITION

   /**
    * Time to use with the above condition. Specified in number of seconds
    * since 1 Jan 1970 
    * @static
    */
   static var TIMEVALUE = _curl.CURLOPT_TIMEVALUE

   /**
    * Custom request, for customizing the get command like
    * HTTP: DELETE, TRACE and others
    * FTP: to use a different list command
    * @static
    */
   static var CUSTOMREQUEST = _curl.CURLOPT_CUSTOMREQUEST

   /**
    * FILE handle to use instead of stderr 
    * @static
    */
   static var STDERR = _curl.CURLOPT_STDERR

   /**
    * send linked-list of post-transfer QUOTE commands 
    * @static
    */
   static var POSTQUOTE = _curl.CURLOPT_POSTQUOTE

   /**
    * talk a lot 
    * @static
    */
   static var VERBOSE = _curl.CURLOPT_VERBOSE

   /**
    * throw the header out too 
    * @static
    */
   static var HEADER = _curl.CURLOPT_HEADER

   /**
    * shut off the progress meter 
    * @static
    */
   static var NOPROGRESS = _curl.CURLOPT_NOPROGRESS

   /**
    * use HEAD to get http document 
    * @static
    */
   static var NOBODY = _curl.CURLOPT_NOBODY

   /**
    * no output on http error codes >= 400 
    * @static
    */
   static var FAILONERROR = _curl.CURLOPT_FAILONERROR

   /**
    * this is an upload 
    * @static
    */
   static var UPLOAD = _curl.CURLOPT_UPLOAD

   /**
    * HTTP POST method 
    * @static
    */
   static var POST = _curl.CURLOPT_POST

   /**
    * bare names when listing directories 
    * @static
    */
   static var DIRLISTONLY = _curl.CURLOPT_DIRLISTONLY

   /**
    * Append instead of overwrite on upload! 
    * @static
    */
   static var APPEND = _curl.CURLOPT_APPEND

   /**
    * Specify whether to read the user+password from the .netrc or the URL.
    * This must be one of the CURL_NETRC_* enums below. 
    * @static
    */
   static var NETRC = _curl.CURLOPT_NETRC

   /**
    * use Location: Luke! 
    * @static
    */
   static var FOLLOWLOCATION = _curl.CURLOPT_FOLLOWLOCATION

   /**
    * transfer data in text/ASCII format 
    * @static
    */
   static var TRANSFERTEXT = _curl.CURLOPT_TRANSFERTEXT

   /**
    * HTTP PUT 
    * @static
    */
   static var PUT = _curl.CURLOPT_UPLOAD

   /**
    * We want the referrer field set automatically when following locations 
    * @static
    */
   static var AUTOREFERER = _curl.CURLOPT_AUTOREFERER

   /**
    * Port of the proxy, can be set in the proxy string as well with:
    * "[host]:[port]" 
    * @static
    */
   static var PROXYPORT = _curl.CURLOPT_PROXYPORT

   /**
    * size of the POST input data, if strlen() is not good to use 
    * @static
    */
   static var POSTFIELDSIZE = _curl.CURLOPT_POSTFIELDSIZE

   /**
    * tunnel non-http operations through a HTTP proxy 
    * @static
    */
   static var HTTPPROXYTUNNEL = _curl.CURLOPT_HTTPPROXYTUNNEL

   /**
    * Set the interface string to use as outgoing network interface 
    * @static
    */
   static var INTERFACE = _curl.CURLOPT_INTERFACE

   /**
    * Set the krb4/5 security level, this also enables krb4/5 awareness.  This
    * is a string, 'clear', 'safe', 'confidential' or 'private'.  If the string
    * is set but doesn't match one of these, 'private' will be used.  
    * @static
    */
   static var KRBLEVEL = _curl.CURLOPT_KRBLEVEL

   /**
    * Set if we should verify the peer in ssl handshake, set 1 to verify. 
    * @static
    */
   static var SSL_VERIFYPEER = _curl.CURLOPT_SSL_VERIFYPEER

   /**
    * The CApath or CAfile used to validate the peer certificate
    * this option is used only if SSL_VERIFYPEER is true 
    * @static
    */
   static var CAINFO = _curl.CURLOPT_CAINFO

   /**
    * Maximum number of http redirects to follow 
    * @static
    */
   static var MAXREDIRS = _curl.CURLOPT_MAXREDIRS

   /**
    * Pass a long set to 1 to get the date of the requested document (if
    * possible)! Pass a zero to shut it off. 
    * @static
    */
   static var FILETIME = _curl.CURLOPT_FILETIME

   /**
    * This points to a linked list of telnet options 
    * @static
    */
   static var TELNETOPTIONS = _curl.CURLOPT_TELNETOPTIONS

   /**
    * Max amount of cached alive connections 
    * @static
    */
   static var MAXCONNECTS = _curl.CURLOPT_MAXCONNECTS

   /**
    * Set to explicitly use a new connection for the upcoming transfer.
    * Do not use this unless you're absolutely sure of this, as it makes the
    * operation slower and is less friendly for the network. 
    * @static
    */
   static var FRESH_CONNECT = _curl.CURLOPT_FRESH_CONNECT

   /**
    * Set to explicitly forbid the upcoming transfer's connection to be re-used
    * when done. Do not use this unless you're absolutely sure of this, as it
    * makes the operation slower and is less friendly for the network. 
    * @static
    */
   static var FORBID_REUSE = _curl.CURLOPT_FORBID_REUSE

   /**
    * Time-out connect operations after this amount of seconds, if connects are
    * OK within this time, then fine... This only aborts the connect phase. 
    * @static
    */
   static var CONNECTTIMEOUT = _curl.CURLOPT_CONNECTTIMEOUT

   /**
    * Set this to force the HTTP request to get back to GET. Only really usable
    * if POST, PUT or a custom request have been used first.
    * @static
    */
   static var HTTPGET = _curl.CURLOPT_HTTPGET

   /**
    * Set if we should verify the Common name from the peer certificate in ssl
    * handshake, set 1 to check existence, 2 to ensure that it matches the
    * provided hostname. 
    * @static
    */
   static var SSL_VERIFYHOST = _curl.CURLOPT_SSL_VERIFYHOST

   /**
    * Specify which file name to write all known cookies in after completed
    * operation. Set file name to "-" (dash) to make it go to stdout. 
    * @static
    */
   static var COOKIEJAR = _curl.CURLOPT_COOKIEJAR

   /**
    * Specify which SSL ciphers to use 
    * @static
    */
   static var SSL_CIPHER_LIST = _curl.CURLOPT_SSL_CIPHER_LIST

   /**
    * Specify which HTTP version to use! This must be set to one of the
    * CURL_HTTP_VERSION* enums set below. 
    * @static
    */
   static var HTTP_VERSION = _curl.CURLOPT_HTTP_VERSION

   /**
    * Specifically switch on or off the FTP engine's use of the EPSV command. By
    * default, that one will always be attempted before the more traditional
    * PASV command. 
    * @static
    */
   static var FTP_USE_EPSV = _curl.CURLOPT_FTP_USE_EPSV

   /**
    * type of the file keeping your SSL-certificate ("DER", "PEM", "ENG") 
    * @static
    */
   static var SSLCERTTYPE = _curl.CURLOPT_SSLCERTTYPE

   /**
    * name of the file keeping your private SSL-key 
    * @static
    */
   static var SSLKEY = _curl.CURLOPT_SSLKEY

   /**
    * type of the file keeping your private SSL-key ("DER", "PEM", "ENG") 
    * @static
    */
   static var SSLKEYTYPE = _curl.CURLOPT_SSLKEYTYPE

   /**
    * crypto engine for the SSL-sub system 
    * @static
    */
   static var SSLENGINE = _curl.CURLOPT_SSLENGINE

   /**
    * set the crypto engine for the SSL-sub system as default
    * the param has no meaning...
    * @static
    */
   static var SSLENGINE_DEFAULT = _curl.CURLOPT_SSLENGINE_DEFAULT

   /**
    * DEPRECATED, do not use! 
    * @static
    */
   static var DNS_USE_GLOBAL_CACHE = _curl.CURLOPT_SHARE

   /**
    * DNS cache timeout 
    * @static
    */
   static var DNS_CACHE_TIMEOUT = _curl.CURLOPT_DNS_CACHE_TIMEOUT

   /**
    * send linked-list of pre-transfer QUOTE commands 
    * @static
    */
   static var PREQUOTE = _curl.CURLOPT_PREQUOTE

   /**
    * mark this as start of a cookie session 
    * @static
    */
   static var COOKIESESSION = _curl.CURLOPT_COOKIESESSION

   /**
    * The CApath directory used to validate the peer certificate
    * this option is used only if SSL_VERIFYPEER is true 
    * @static
    */
   static var CAPATH = _curl.CURLOPT_CAPATH

   /**
    * Instruct libcurl to use a smaller receive buffer 
    * @static
    */
   static var BUFFERSIZE = _curl.CURLOPT_BUFFERSIZE

   /**
    * Instruct libcurl to not use any signal/alarm handlers, even when using
    * timeouts. This option is useful for multi-threaded applications.
    * See libcurl-the-guide for more background information. 
    * @static
    */
   static var NOSIGNAL = _curl.CURLOPT_NOSIGNAL

   /**
    * Provide a CURLShare for mutexing non-ts data 
    * @static
    */
   static var SHARE = _curl.CURLOPT_SHARE

   /**
    * indicates type of proxy. accepted values are CURLPROXY_HTTP (default),
    * CURLPROXY_HTTPS, CURLPROXY_SOCKS4, CURLPROXY_SOCKS4A and
    * CURLPROXY_SOCKS5. 
    * @static
    */
   static var PROXYTYPE = _curl.CURLOPT_PROXYTYPE

   /**
    * Set the Accept-Encoding string. Use this to tell a server you would like
    * the response to be compressed. Before 7.21.6, this was known as
    * CURLOPT_ENCODING 
    * @static
    */
   static var ACCEPT_ENCODING = _curl.CURLOPT_ACCEPT_ENCODING

   /**
    * Set pointer to private data 
    * @static
    */
   static var PRIVATE = _curl.CURLOPT_PRIVATE

   /**
    * Set aliases for HTTP 200 in the HTTP Response header 
    * @static
    */
   static var HTTP200ALIASES = _curl.CURLOPT_HTTP200ALIASES

   /**
    * Continue to send authentication (user+password) when following locations,
    * even when hostname changed. This can potentially send off the name
    * and password to whatever host the server decides. 
    * @static
    */
   static var UNRESTRICTED_AUTH = _curl.CURLOPT_UNRESTRICTED_AUTH

   /**
    * Specifically switch on or off the FTP engine's use of the EPRT command (
    * it also disables the LPRT attempt). By default, those ones will always be
    * attempted before the good old traditional PORT command. 
    * @static
    */
   static var FTP_USE_EPRT = _curl.CURLOPT_FTP_USE_EPRT

   /**
    * Set this to a bitmask value to enable the particular authentications
    * methods you like. Use this in combination with CURLOPT_USERPWD.
    * Note that setting multiple bits may cause extra network round-trips. 
    * @static
    */
   static var HTTPAUTH = _curl.CURLOPT_HTTPAUTH

   /**
    * FTP Option that causes missing dirs to be created on the remote server.
    * In 7.19.4 we introduced the convenience enums for this option using the
    * CURLFTP_CREATE_DIR prefix.
    * @static
    */
   static var FTP_CREATE_MISSING_DIRS = _curl.CURLOPT_FTP_CREATE_MISSING_DIRS

   /**
    * Set this to a bitmask value to enable the particular authentications
    * methods you like. Use this in combination with CURLOPT_PROXYUSERPWD.
    * Note that setting multiple bits may cause extra network round-trips. 
    * @static
    */
   static var PROXYAUTH = _curl.CURLOPT_PROXYAUTH

   /**
    * FTP option that changes the timeout, in seconds, associated with
    * getting a response.  This is different from transfer timeout time and
    * essentially places a demand on the FTP server to acknowledge commands
    * in a timely manner. 
    * @static
    */
   static var FTP_RESPONSE_TIMEOUT = _curl.CURLOPT_FTP_RESPONSE_TIMEOUT

   /**
    * This option that changes the timeout, in seconds, associated with
    * getting a response from a server.
    * @static
    */
   static var SERVER_RESPONSE_TIMEOUT = _curl.CURLOPT_SERVER_RESPONSE_TIMEOUT

   /**
    * Set this option to one of the CURL_IPRESOLVE_* defines (see below) to
    * tell libcurl to use those IP versions only. This only has effect on
    * systems with support for more than one, i.e IPv4 _and_ IPv6. 
    * @static
    */
   static var IPRESOLVE = _curl.CURLOPT_IPRESOLVE

   /**
    * Set this option to limit the size of a file that will be downloaded from
    * an HTTP or FTP server.
    * 
    * > There is also _LARGE version which adds large file support for
    * platforms which have larger off_t sizes.  See MAXFILESIZE_LARGE below. 
    * @static
    */
   static var MAXFILESIZE = _curl.CURLOPT_MAXFILESIZE

   /**
    * See the comment for INFILESIZE above, but in short, specifies
    * the size of the file being uploaded.  -1 means unknown.
    * @static
    */
   static var INFILESIZE_LARGE = _curl.CURLOPT_INFILESIZE_LARGE

   /**
    * Sets the continuation offset.  There is also a CURLOPTTYPE_LONG version
    * of this; look above for RESUME_FROM.
    
   * @static
    */
   static var RESUME_FROM_LARGE = _curl.CURLOPT_RESUME_FROM_LARGE

   /**
    * Sets the maximum size of data that will be downloaded from
    * an HTTP or FTP server.  See MAXFILESIZE above for the LONG version.
    * @static
    */
   static var MAXFILESIZE_LARGE = _curl.CURLOPT_MAXFILESIZE_LARGE

   /**
    * Set this option to the file name of your .netrc file you want libcurl
    * to parse (using the CURLOPT_NETRC option). If not set, libcurl will do
    * a poor attempt to find the user's home directory and check for a .netrc
    * file in there. 
    * @static
    */
   static var NETRC_FILE = _curl.CURLOPT_NETRC_FILE

   /**
    * Enable SSL/TLS for FTP, pick one of:
    * CURLUSESSL_TRY     - try using SSL, proceed anyway otherwise
    * CURLUSESSL_CONTROL - SSL for the control connection or fail
    * CURLUSESSL_ALL     - SSL for all communication or fail
    * @static
    */
   static var USE_SSL = _curl.CURLOPT_USE_SSL

   /**
    * The _LARGE version of the standard POSTFIELDSIZE option 
    * @static
    */
   static var POSTFIELDSIZE_LARGE = _curl.CURLOPT_POSTFIELDSIZE_LARGE

   /**
    * Enable/disable the TCP Nagle algorithm 
    * @static
    */
   static var TCP_NODELAY = _curl.CURLOPT_TCP_NODELAY

   /**
    * When FTP over SSL/TLS is selected (with CURLOPT_USE_SSL), this option
    * can be used to change libcurl's default action which is to first try
    * "AUTH SSL" and then "AUTH TLS" in this order, and proceed when a OK
    * response has been received.
    * 
    * Available parameters are:
    * CURLFTPAUTH_DEFAULT - let libcurl decide
    * CURLFTPAUTH_SSL     - try "AUTH SSL" first, then TLS
    * CURLFTPAUTH_TLS     - try "AUTH TLS" first, then SSL
    * @static
    */
   static var FTPSSLAUTH = _curl.CURLOPT_FTPSSLAUTH

   /**
    * null-terminated string for pass on to the FTP server when asked for
    * "account" info 
    * @static
    */
   static var FTP_ACCOUNT = _curl.CURLOPT_FTP_ACCOUNT

   /**
    * feed cookie into cookie engine 
    * @static
    */
   static var COOKIELIST = _curl.CURLOPT_COOKIELIST

   /**
    * ignore Content-Length 
    * @static
    */
   static var IGNORE_CONTENT_LENGTH = _curl.CURLOPT_IGNORE_CONTENT_LENGTH

   /**
    * Set to non-zero to skip the IP address received in a 227 PASV FTP server
    * response. Typically used for FTP-SSL purposes but is not restricted to
    * that. libcurl will then instead use the same IP address it used for the
    * control connection. 
    * @static
    */
   static var FTP_SKIP_PASV_IP = _curl.CURLOPT_FTP_SKIP_PASV_IP

   /**
    * Select "file method" to use when doing FTP, see the curl ftpmethod
    * above. 
    * @static
    */
   static var FTP_FILEMETHOD = _curl.CURLOPT_FTP_FILEMETHOD

   /**
    * Local port number to bind the socket to 
    * @static
    */
   static var LOCALPORT = _curl.CURLOPT_LOCALPORT

   /**
    * Number of ports to try, including the first one set with LOCALPORT.
    * Thus, setting it to 1 will make no additional attempts but the first.
    * @static
    */
   static var LOCALPORTRANGE = _curl.CURLOPT_LOCALPORTRANGE

   /**
    * no transfer, set up connection and let application use the socket by
    * extracting it with CURLINFO_LASTSOCKET 
    * @static
    */
   static var CONNECT_ONLY = _curl.CURLOPT_CONNECT_ONLY

   /**
    * limit-rate: maximum number of bytes per second to send 
    * @static
    */
   static var MAX_SEND_SPEED_LARGE = _curl.CURLOPT_MAX_SEND_SPEED_LARGE

   /**
    * limit-rate: maximum number of bytes per second to receive 
    * @static
    */
   static var MAX_RECV_SPEED_LARGE = _curl.CURLOPT_MAX_RECV_SPEED_LARGE

   /**
    * Pointer to command string to send if USER/PASS fails. 
    * @static
    */
   static var FTP_ALTERNATIVE_TO_USER = _curl.CURLOPT_FTP_ALTERNATIVE_TO_USER

   /**
    * set to 0 to disable session ID re-use for this transfer, default is
    * enabled (== 1) 
    * @static
    */
   static var SSL_SESSIONID_CACHE = _curl.CURLOPT_SSL_SESSIONID_CACHE

   /**
    * allowed SSH authentication methods 
    * @static
    */
   static var SSH_AUTH_TYPES = _curl.CURLOPT_SSH_AUTH_TYPES

   /**
    * Used by scp/sftp to do public key authentication 
    * @static
    */
   static var SSH_PUBLIC_KEYFILE = _curl.CURLOPT_SSH_PUBLIC_KEYFILE

   /**
    * Used by scp/sftp to do private key authentication 
    * @static
    */
   static var SSH_PRIVATE_KEYFILE = _curl.CURLOPT_SSH_PRIVATE_KEYFILE

   /**
    * Send CCC (Clear Command Channel) after authentication 
    * @static
    */
   static var FTP_SSL_CCC = _curl.CURLOPT_FTP_SSL_CCC

   /**
    * Same as TIMEOUT, but with ms resolution 
    * @static
    */
   static var TIMEOUT_MS = _curl.CURLOPT_TIMEOUT_MS

   /**
    * Same as CONNECTTIMEOUT, but with ms resolution 
    * @static
    */
   static var CONNECTTIMEOUT_MS = _curl.CURLOPT_CONNECTTIMEOUT_MS

   /**
    * set to zero to disable the libcurl's decoding and thus pass the raw body
    * data to the application even when it is encoded/compressed via transfter encoding
    * @static
    */
   static var HTTP_TRANSFER_DECODING = _curl.CURLOPT_HTTP_TRANSFER_DECODING

   /**
    * set to zero to disable the libcurl's decoding and thus pass the raw body
    * data to the application even when it is encoded/compressed via content encoding
    * @static
    */
   static var HTTP_CONTENT_DECODING = _curl.CURLOPT_HTTP_CONTENT_DECODING

   /**
    * Permission used when creating new files on the remote
    * server for protocols that support it, SFTP/SCP/FILE 
    * @static
    */
   static var NEW_FILE_PERMS = _curl.CURLOPT_NEW_FILE_PERMS

   /**
    * Permission used when creating new directories on the remote
    * server for protocols that support it, SFTP/SCP/FILE 
    * @static
    */
   static var NEW_DIRECTORY_PERMS = _curl.CURLOPT_NEW_DIRECTORY_PERMS

   /**
    * Set the behavior of POST when redirecting. Values must be set to one
    * of CURL_REDIR* defines below. This used to be called CURLOPT_POST301 
    * @static
    */
   static var POSTREDIR = _curl.CURLOPT_POSTREDIR

   /**
    * used by scp/sftp to verify the host's public key 
    * @static
    */
   static var SSH_HOST_PUBLIC_KEY_MD5 = _curl.CURLOPT_SSH_HOST_PUBLIC_KEY_MD5

   /**
    * POST volatile input fields. 
    * @static
    */
   static var COPYPOSTFIELDS = _curl.CURLOPT_COPYPOSTFIELDS

   /**
    * set transfer mode (;type=<a|i>) when doing FTP via an HTTP proxy 
    * @static
    */
   static var PROXY_TRANSFER_MODE = _curl.CURLOPT_PROXY_TRANSFER_MODE

   /**
    * CRL file 
    * @static
    */
   static var CRLFILE = _curl.CURLOPT_CRLFILE

   /**
    * Issuer certificate 
    * @static
    */
   static var ISSUERCERT = _curl.CURLOPT_ISSUERCERT

   /**
    * (IPv6) Address scope 
    * @static
    */
   static var ADDRESS_SCOPE = _curl.CURLOPT_ADDRESS_SCOPE

   /**
    * Collect certificate chain info and allow it to get retrievable with
    * CURLINFO_CERTINFO after the transfer is complete. 
    * @static
    */
   static var CERTINFO = _curl.CURLOPT_CERTINFO

   /**
    * "name" (username) to use when fetching. 
    * @static
    */
   static var USERNAME = _curl.CURLOPT_USERNAME

   /**
    * "pwd" (password) to use when fetching. 
    * @static
    */
   static var PASSWORD = _curl.CURLOPT_PASSWORD

   /**
    * "name" (username) to use with Proxy when fetching. 
    * @static
    */
   static var PROXYUSERNAME = _curl.CURLOPT_PROXYUSERNAME

   /**
    * "pwd" (password) to use with Proxy when fetching. 
    * @static
    */
   static var PROXYPASSWORD = _curl.CURLOPT_PROXYPASSWORD

   /**
    * Comma separated list of hostnames defining no-proxy zones. These should
    * match both hostnames directly, and hostnames within a domain. For
    * example, local.com will match local.com and www.local.com, but NOT
    * notlocal.com or www.notlocal.com. For compatibility with other
    * implementations of this, .local.com will be considered to be the same as
    * local.com. A single * is the only valid wildcard, and effectively
    * disables the use of proxy. 
    * @static
    */
   static var NOPROXY = _curl.CURLOPT_NOPROXY

   /**
    * block size for TFTP transfers 
    * @static
    */
   static var TFTP_BLKSIZE = _curl.CURLOPT_TFTP_BLKSIZE

   /**
    * Socks Service 
    * @static
    */
   static var SOCKS5_GSSAPI_NEC = _curl.CURLOPT_SOCKS5_GSSAPI_NEC

   /**
    * set the bitmask for the protocols that are allowed to be used for the
    * transfer, which thus helps the app which takes URLs from users or other
    * external inputs and want to restrict what protocol(s) to deal
    * with. Defaults to CURLPROTO_ALL. 
    * @static
    */
   static var PROTOCOLS = _curl.CURLOPT_PROTOCOLS_STR != 0 ? 
      _curl.CURLOPT_PROTOCOLS_STR : 
      _curl.CURLOPT_PROTOCOLS

   /**
    * set the bitmask for the protocols that libcurl is allowed to follow to,
    * as a subset of the CURLOPT_PROTOCOLS ones. That means the protocol needs
    * to be set in both bitmasks to be allowed to get redirected to. 
    * @static
    */
   static var REDIR_PROTOCOLS = _curl.CURLOPT_REDIR_PROTOCOLS_STR != 0 ?
      _curl.CURLOPT_REDIR_PROTOCOLS_STR :
      _curl.CURLOPT_REDIR_PROTOCOLS

   /**
    * set the SSH knownhost file name to use 
    * @static
    */
   static var SSH_KNOWNHOSTS = _curl.CURLOPT_SSH_KNOWNHOSTS

   /**
    * set the SMTP mail originator 
    * @static
    */
   static var MAIL_FROM = _curl.CURLOPT_MAIL_FROM

   /**
    * set the list of SMTP mail receiver(s) 
    * @static
    */
   static var MAIL_RCPT = _curl.CURLOPT_MAIL_RCPT

   /**
    * FTP: send PRET before PASV 
    * @static
    */
   static var FTP_USE_PRET = _curl.CURLOPT_FTP_USE_PRET

   /**
    * RTSP request method (OPTIONS, SETUP, PLAY, etc...) 
    * @static
    */
   static var RTSP_REQUEST = _curl.CURLOPT_RTSP_REQUEST

   /**
    * The RTSP session identifier 
    * @static
    */
   static var RTSP_SESSION_ID = _curl.CURLOPT_RTSP_SESSION_ID

   /**
    * The RTSP stream URI 
    * @static
    */
   static var RTSP_STREAM_URI = _curl.CURLOPT_RTSP_STREAM_URI

   /**
    * The Transport: header to use in RTSP requests 
    * @static
    */
   static var RTSP_TRANSPORT = _curl.CURLOPT_RTSP_TRANSPORT

   /**
    * Manually initialize the client RTSP CSeq for this handle 
    * @static
    */
   static var RTSP_CLIENT_CSEQ = _curl.CURLOPT_RTSP_CLIENT_CSEQ

   /**
    * Manually initialize the server RTSP CSeq for this handle 
    * @static
    */
   static var RTSP_SERVER_CSEQ = _curl.CURLOPT_RTSP_SERVER_CSEQ

   /**
    * Turn on wildcard matching 
    * @static
    */
   static var WILDCARDMATCH = _curl.CURLOPT_WILDCARDMATCH

   /**
    * send linked-list of name:port:address sets 
    * @static
    */
   static var RESOLVE = _curl.CURLOPT_RESOLVE

   /**
    * Set a username for authenticated TLS 
    * @static
    */
   static var TLSAUTH_USERNAME = _curl.CURLOPT_TLSAUTH_USERNAME

   /**
    * Set a password for authenticated TLS 
    * @static
    */
   static var TLSAUTH_PASSWORD = _curl.CURLOPT_TLSAUTH_PASSWORD

   /**
    * Set authentication type for authenticated TLS 
    * @static
    */
   static var TLSAUTH_TYPE = _curl.CURLOPT_TLSAUTH_TYPE

   /**
    * Set to 1 to enable the "TE:" header in HTTP requests to ask for
    * compressed transfer-encoded responses. Set to 0 to disable the use of TE:
    * in outgoing requests. The current default is 0, but it might change in a
    * future libcurl release.
    * 
    * libcurl will ask for the compressed methods it knows of, and if that
    * isn't any, it will not ask for transfer-encoding at all even if this
    * option is set to 1.
    * @static
    */
   static var TRANSFER_ENCODING = _curl.CURLOPT_TRANSFER_ENCODING

   /**
    * allow GSSAPI credential delegation 
    * @static
    */
   static var GSSAPI_DELEGATION = _curl.CURLOPT_GSSAPI_DELEGATION

   /**
    * Set the name servers to use for DNS resolution 
    * @static
    */
   static var DNS_SERVERS = _curl.CURLOPT_DNS_SERVERS

   /**
    * Time-out accept operations (currently for FTP only) after this amount
    * of milliseconds. 
    * @static
    */
   static var ACCEPTTIMEOUT_MS = _curl.CURLOPT_ACCEPTTIMEOUT_MS

   /**
    * Set TCP keepalive 
    * @static
    */
   static var TCP_KEEPALIVE = _curl.CURLOPT_TCP_KEEPALIVE

   /**
    * non-universal keepalive idle time (Linux, AIX, HP-UX, more) 
    * @static
    */
   static var TCP_KEEPIDLE = _curl.CURLOPT_TCP_KEEPIDLE

   /**
    * non-universal keepalive interval (Linux, AIX, HP-UX, more) 
    * @static
    */
   static var TCP_KEEPINTVL = _curl.CURLOPT_TCP_KEEPINTVL

   /**
    * Enable/disable specific SSL features with a bitmask, see CURLSSLOPT_* 
    * @static
    */
   static var SSL_OPTIONS = _curl.CURLOPT_SSL_OPTIONS

   /**
    * Set the SMTP auth originator 
    * @static
    */
   static var MAIL_AUTH = _curl.CURLOPT_MAIL_AUTH

   /**
    * Enable/disable SASL initial response 
    * @static
    */
   static var SASL_IR = _curl.CURLOPT_SASL_IR

   /**
    * The XOAUTH2 bearer token 
    * @static
    */
   static var XOAUTH2_BEARER = _curl.CURLOPT_XOAUTH2_BEARER

   /**
    * Set the interface string to use as outgoing network
    * interface for DNS requests.
    * Only supported by the c-ares DNS backend 
    * @static
    */
   static var DNS_INTERFACE = _curl.CURLOPT_DNS_INTERFACE

   /**
    * Set the local IPv4 address to use for outgoing DNS requests.
    * Only supported by the c-ares DNS backend 
    * @static
    */
   static var DNS_LOCAL_IP4 = _curl.CURLOPT_DNS_LOCAL_IP4

   /**
    * Set the local IPv6 address to use for outgoing DNS requests.
    * Only supported by the c-ares DNS backend 
    * @static
    */
   static var DNS_LOCAL_IP6 = _curl.CURLOPT_DNS_LOCAL_IP6

   /**
    * Set authentication options directly 
    * @static
    */
   static var LOGIN_OPTIONS = _curl.CURLOPT_LOGIN_OPTIONS

   /**
    * Enable/disable TLS NPN extension (http2 over ssl might fail without) 
    * @static
    */
   static var SSL_ENABLE_NPN = _curl.CURLOPT_SSL_ENABLE_NPN

   /**
    * Enable/disable TLS ALPN extension (http2 over ssl might fail without) 
    * @static
    */
   static var SSL_ENABLE_ALPN = _curl.CURLOPT_SSL_ENABLE_ALPN

   /**
    * Time to wait for a response to a HTTP request containing an
    * Expect: 100-continue header before sending the data anyway. 
    * @static
    */
   static var EXPECT_100_TIMEOUT_MS = _curl.CURLOPT_EXPECT_100_TIMEOUT_MS

   /**
    * This points to a list of headers used for proxy requests only.
    * @static
    */
   static var PROXYHEADER = _curl.CURLOPT_PROXYHEADER

   /**
    * Pass in a bitmask of "header options" 
    * @static
    */
   static var HEADEROPT = _curl.CURLOPT_HEADEROPT

   /**
    * The public key in DER form used to validate the peer public key
       this option is used only if SSL_VERIFYPEER is true 
   * @static
    */
   static var PINNEDPUBLICKEY = _curl.CURLOPT_PINNEDPUBLICKEY

   /**
    * Path to Unix domain socket 
    * @static
    */
   static var UNIX_SOCKET_PATH = _curl.CURLOPT_UNIX_SOCKET_PATH

   /**
    * Set if we should verify the certificate status. 
    * @static
    */
   static var SSL_VERIFYSTATUS = _curl.CURLOPT_SSL_VERIFYSTATUS

   /**
    * Set if we should enable TLS false start. 
    * @static
    */
   static var SSL_FALSESTART = _curl.CURLOPT_SSL_FALSESTART

   /**
    * Do not squash dot-dot sequences 
    * @static
    */
   static var PATH_AS_IS = _curl.CURLOPT_PATH_AS_IS

   /**
    * Proxy Service Name 
    * @static
    */
   static var PROXY_SERVICE_NAME = _curl.CURLOPT_PROXY_SERVICE_NAME

   /**
    * Service Name 
    * @static
    */
   static var SERVICE_NAME = _curl.CURLOPT_SERVICE_NAME

   /**
    * Wait/don't wait for pipe/mutex to clarify 
    * @static
    */
   static var PIPEWAIT = _curl.CURLOPT_PIPEWAIT

   /**
    * Set the protocol used when curl is given a URL without a protocol 
    * @static
    */
   static var DEFAULT_PROTOCOL = _curl.CURLOPT_DEFAULT_PROTOCOL

   /**
    * Set stream weight, 1 - 256 (default is 16) 
    * @static
    */
   static var STREAM_WEIGHT = _curl.CURLOPT_STREAM_WEIGHT

   /**
    * Set stream dependency on another CURL handle 
    * @static
    */
   static var STREAM_DEPENDS = _curl.CURLOPT_STREAM_DEPENDS

   /**
    * Set E-xclusive stream dependency on another CURL handle 
    * @static
    */
   static var STREAM_DEPENDS_E = _curl.CURLOPT_STREAM_DEPENDS_E

   /**
    * Do not send any tftp option requests to the server 
    * @static
    */
   static var TFTP_NO_OPTIONS = _curl.CURLOPT_TFTP_NO_OPTIONS

   /**
    * Linked-list of host:port:connect-to-host:connect-to-port,
       overrides the URL's host:port (only for the network layer) 
   * @static
    */
   static var CONNECT_TO = _curl.CURLOPT_CONNECT_TO

   /**
    * Set TCP Fast Open 
    * @static
    */
   static var TCP_FASTOPEN = _curl.CURLOPT_TCP_FASTOPEN

   /**
    * Continue to send data if the server responds early with an
    * HTTP status code >= 300 
    * @static
    */
   static var KEEP_SENDING_ON_ERROR = _curl.CURLOPT_KEEP_SENDING_ON_ERROR

   /**
    * The CApath or CAfile used to validate the proxy certificate
    * this option is used only if PROXY_SSL_VERIFYPEER is true 
    * @static
    */
   static var PROXY_CAINFO = _curl.CURLOPT_PROXY_CAINFO

   /**
    * The CApath directory used to validate the proxy certificate
    * this option is used only if PROXY_SSL_VERIFYPEER is true 
    * @static
    */
   static var PROXY_CAPATH = _curl.CURLOPT_PROXY_CAPATH

   /**
    * Set if we should verify the proxy in ssl handshake,
    * set 1 to verify. 
    * @static
    */
   static var PROXY_SSL_VERIFYPEER = _curl.CURLOPT_PROXY_SSL_VERIFYPEER

   /**
    * Set if we should verify the Common name from the proxy certificate in ssl
    * handshake, set 1 to check existence, 2 to ensure that it matches
    * the provided hostname. 
    * @static
    */
   static var PROXY_SSL_VERIFYHOST = _curl.CURLOPT_PROXY_SSL_VERIFYHOST

   /**
    * What version to specifically try to use for proxy.
    * See CURL_SSLVERSION defines below. 
    * @static
    */
   static var PROXY_SSLVERSION = _curl.CURLOPT_PROXY_SSLVERSION

   /**
    * Set a username for authenticated TLS for proxy 
    * @static
    */
   static var PROXY_TLSAUTH_USERNAME = _curl.CURLOPT_PROXY_TLSAUTH_USERNAME

   /**
    * Set a password for authenticated TLS for proxy 
    * @static
    */
   static var PROXY_TLSAUTH_PASSWORD = _curl.CURLOPT_PROXY_TLSAUTH_PASSWORD

   /**
    * Set authentication type for authenticated TLS for proxy 
    * @static
    */
   static var PROXY_TLSAUTH_TYPE = _curl.CURLOPT_PROXY_TLSAUTH_TYPE

   /**
    * name of the file keeping your private SSL-certificate for proxy 
    * @static
    */
   static var PROXY_SSLCERT = _curl.CURLOPT_PROXY_SSLCERT

   /**
    * type of the file keeping your SSL-certificate ("DER", "PEM", "ENG") for
    * proxy 
    * @static
    */
   static var PROXY_SSLCERTTYPE = _curl.CURLOPT_PROXY_SSLCERTTYPE

   /**
    * name of the file keeping your private SSL-key for proxy 
    * @static
    */
   static var PROXY_SSLKEY = _curl.CURLOPT_PROXY_SSLKEY

   /**
    * type of the file keeping your private SSL-key ("DER", "PEM", "ENG") for
    * proxy 
    * @static
    */
   static var PROXY_SSLKEYTYPE = _curl.CURLOPT_PROXY_SSLKEYTYPE

   /**
    * password for the SSL private key for proxy 
    * @static
    */
   static var PROXY_KEYPASSWD = _curl.CURLOPT_PROXY_KEYPASSWD

   /**
    * Specify which SSL ciphers to use for proxy 
    * @static
    */
   static var PROXY_SSL_CIPHER_LIST = _curl.CURLOPT_PROXY_SSL_CIPHER_LIST

   /**
    * CRL file for proxy 
    * @static
    */
   static var PROXY_CRLFILE = _curl.CURLOPT_PROXY_CRLFILE

   /**
    * Enable/disable specific SSL features with a bitmask for proxy, see
    * CURLSSLOPT_* 
    * @static
    */
   static var PROXY_SSL_OPTIONS = _curl.CURLOPT_PROXY_SSL_OPTIONS

   /**
    * Name of pre proxy to use. 
    * @static
    */
   static var PRE_PROXY = _curl.CURLOPT_PRE_PROXY

   /**
    * The public key in DER form used to validate the proxy public key
    * this option is used only if PROXY_SSL_VERIFYPEER is true 
    * @static
    */
   static var PROXY_PINNEDPUBLICKEY = _curl.CURLOPT_PROXY_PINNEDPUBLICKEY

   /**
    * Path to an abstract Unix domain socket 
    * @static
    */
   static var ABSTRACT_UNIX_SOCKET = _curl.CURLOPT_ABSTRACT_UNIX_SOCKET

   /**
    * Suppress proxy CONNECT response headers from user callbacks 
    * @static
    */
   static var SUPPRESS_CONNECT_HEADERS = _curl.CURLOPT_SUPPRESS_CONNECT_HEADERS

   /**
    * The request target, instead of extracted from the URL 
    * @static
    */
   static var REQUEST_TARGET = _curl.CURLOPT_REQUEST_TARGET

   /**
    * bitmask of allowed auth methods for connections to SOCKS5 proxies 
    * @static
    */
   static var SOCKS5_AUTH = _curl.CURLOPT_SOCKS5_AUTH

   /**
    * Enable/disable SSH compression 
    * @static
    */
   static var SSH_COMPRESSION = _curl.CURLOPT_SSH_COMPRESSION

   /**
    * Post MIME data. 
    * @static
    */
   static var MIMEPOST = _curl.CURLOPT_MIMEPOST

   /**
    * The data that will be used as the body of the request.
    * @static
    */
   static var READDATA = _curl.CURLOPT_READDATA

   /**
    * Time to use with the CURLOPT_TIMECONDITION. Specified in number of
    * seconds since 1 Jan 1970. 
    * @static
    */
   static var TIMEVALUE_LARGE = _curl.CURLOPT_TIMEVALUE_LARGE

   /**
    * Head start in milliseconds to give happy eyeballs. 
    * @static
    */
   static var HAPPY_EYEBALLS_TIMEOUT_MS = _curl.CURLOPT_HAPPY_EYEBALLS_TIMEOUT_MS

   /**
    * send HAProxy PROXY protocol header? 
    * @static
    */
   static var HAPROXYPROTOCOL = _curl.CURLOPT_HAPROXYPROTOCOL

   /**
    * shuffle addresses before use when DNS returns multiple 
    * @static
    */
   static var DNS_SHUFFLE_ADDRESSES = _curl.CURLOPT_DNS_SHUFFLE_ADDRESSES

   /**
    * Specify which TLS 1.3 ciphers suites to use 
    * @static
    */
   static var TLS13_CIPHERS = _curl.CURLOPT_TLS13_CIPHERS

   /**
    * Specify which TLS 1.3 ciphers suites to use with a proxy
    * @static
    */
   static var PROXY_TLS13_CIPHERS = _curl.CURLOPT_PROXY_TLS13_CIPHERS

   /**
    * Disallow specifying username/login in URL. 
    * @static
    */
   static var DISALLOW_USERNAME_IN_URL = _curl.CURLOPT_DISALLOW_USERNAME_IN_URL

   /**
    * DNS-over-HTTPS URL 
    * @static
    */
   static var DOH_URL = _curl.CURLOPT_DOH_URL

   /**
    * Preferred buffer size to use for uploads 
    * @static
    */
   static var UPLOAD_BUFFERSIZE = _curl.CURLOPT_UPLOAD_BUFFERSIZE

   /**
    * Time in ms between connection upkeep calls for long-lived connections. 
    * @static
    */
   static var UPKEEP_INTERVAL_MS = _curl.CURLOPT_UPKEEP_INTERVAL_MS

   /**
    * Specify URL using CURL URL API. 
    * @static
    */
   static var CURLU = _curl.CURLOPT_CURLU

   /**
    * set this to 1L to allow HTTP/0.9 responses or 0L to disallow 
    * @static
    */
   static var HTTP09_ALLOWED = _curl.CURLOPT_HTTP09_ALLOWED

   /**
    * alt-svc control bitmask 
    * @static
    */
   static var ALTSVC_CTRL = _curl.CURLOPT_ALTSVC_CTRL

   /**
    * alt-svc cache file name to possibly read from/write to 
    * @static
    */
   static var ALTSVC = _curl.CURLOPT_ALTSVC

   /**
    * maximum age of a connection to consider it for reuse (in seconds) 
    * @static
    */
   static var MAXAGE_CONN = _curl.CURLOPT_MAXAGE_CONN

   /**
    * SASL authorisation identity 
    * @static
    */
   static var SASL_AUTHZID = _curl.CURLOPT_SASL_AUTHZID
}
