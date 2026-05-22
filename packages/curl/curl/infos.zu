#!-- part of the curl module

import _curl

/**
 * cURL request and response informations for `get_info()`
 */
class Info {

  /**
   * Gets the last used effective URL. If follow redirects is enabled, 
   * it may very well not be the same value you set in the original request.
   * @static
   */
  static var EFFECTIVE_URL = _curl.CURLINFO_EFFECTIVE_URL

  /**
   * The last received HTTP, FTP or SMTP response code. The value will be 
   * zero if no server response code has been received.
   * @static
   * @note A proxy's CONNECT response should be read with HTTP_CONNECTCODE and not this.
   */
  static var RESPONSE_CODE = _curl.CURLINFO_RESPONSE_CODE

  /**
   * The total time in seconds for the previous transfer, including name resolving, 
   * TCP connect etc. The value represents the time in seconds, including fractions.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var TOTAL_TIME = _curl.CURLINFO_TOTAL_TIME

  /**
   * The total time in seconds from the start until the name resolving was completed.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var NAMELOOKUP_TIME = _curl.CURLINFO_NAMELOOKUP_TIME

  /**
   * The total time in seconds from the start until the connection to the remote host 
   * (or proxy) was completed.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var CONNECT_TIME = _curl.CURLINFO_CONNECT_TIME

  /**
   * The time, in seconds, it took from the start until a file transfer is just about 
   * to begin.
   * 
   * This time-stamp includes all pre-transfer commands and negotiations that are 
   * specific to the particular protocol(s) involved. It includes the sending of the 
   * protocol- specific protocol instructions that triggers a transfer.
   * 
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var PRETRANSFER_TIME = _curl.CURLINFO_PRETRANSFER_TIME

  /**
   * The total amount of bytes that were uploaded.
   * @static
   */
  static var SIZE_UPLOAD = _curl.CURLINFO_SIZE_UPLOAD_T

  /**
   * The total amount of bytes that were  downloaded.  The amount is only for the 
   * latest transfer and will be reset again for each new transfer. This counts actual 
   * payload data, what's also commonly called body. All meta and header data are 
   * excluded and will not be counted in this number.
   * @static
   */
  static var SIZE_DOWNLOAD = _curl.CURLINFO_SIZE_DOWNLOAD_T

  /**
   * The average download speed that curl measured for the complete download. Measured 
   * in bytes/second.
   * @static
   */
  static var SPEED_DOWNLOAD = _curl.CURLINFO_SPEED_DOWNLOAD_T

  /**
   * The average upload speed that curl measured for the complete upload. Measured 
   * in bytes/second.
   * @static
   */
  static var SPEED_UPLOAD = _curl.CURLINFO_SPEED_UPLOAD_T

  /**
   * The total size of all the headers received. Measured in number of bytes.
   * @static
   */
  static var HEADER_SIZE = _curl.CURLINFO_HEADER_SIZE

  /**
   * The total size of the issued requests. This is so far only for HTTP requests. 
   * @static
   * @note This may be  more than one request if `Options.FOLLOWLOCATION` is enabled.
   */
  static var REQUEST_SIZE = _curl.CURLINFO_REQUEST_SIZE

  /**
   * The result of the server SSL certificate verification that was requested (using 
   * the `Options.SSL_VERIFYPEER` option).
   * @static
   * @note `0` is a positive result. Non-zero is an error.
   */
  static var SSL_VERIFYRESULT = _curl.CURLINFO_SSL_VERIFYRESULT

  /**
   * The remote time of the retrieved document (in number of seconds since 1 jan 1970 
   * in the GMT/UTC time zone). If you get -1, it can be because of many reasons (it might 
   * be unknown, the server might hide it or the server doesn't support the command that 
   * tells document time etc) and the time of the document is unknown.
   * 
   * You _MUST_ to collect this information before the transfer is made, by using the 
   * `Options.FILETIME` option to `set_option()` or you will unconditionally get a -1 back.
   * @static
   */
  static var FILETIME = _curl.CURLINFO_FILETIME_T

  /**
   * The content-length of the download. This is the value read from the `Content-Length:` 
   * field. It is -1 if the size isn't known.
   * @static
   */
  static var CONTENT_LENGTH_DOWNLOAD = _curl.CURLINFO_CONTENT_LENGTH_DOWNLOAD_T

  /**
   * The content-length of the upload. It is -1 if the size isn't known.
   * @static
   */
  static var CONTENT_LENGTH_UPLOAD = _curl.CURLINFO_CONTENT_LENGTH_UPLOAD_T

  /**
   * The time, in seconds, it took from the start until the first byte is received by 
   * `curl`. This includes `PRETRANSFER_TIME` and also the time the server needs to 
   * calculate the result.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var STARTTRANSFER_TIME = _curl.CURLINFO_STARTTRANSFER_TIME

  /**
   * The content-type of the downloaded object. This is the value read from the 
   * `Content-Type:` field. If you get `nil`, it means that the server didn't send a 
   * valid Content-Type header or that the protocol used doesn't support this.
   * @static
   */
  static var CONTENT_TYPE = _curl.CURLINFO_CONTENT_TYPE

  /**
   * The total time, in seconds, it took for all redirection steps include name lookup, 
   * connect, pretransfer and transfer before final transaction was started. 
   * @static
   * @note It contains the complete execution time for multiple redirections.

   */
  static var REDIRECT_TIME = _curl.CURLINFO_REDIRECT_TIME

  /**
   * The total number of redirections that were actually followed.
   * @static
   */
  static var REDIRECT_COUNT = _curl.CURLINFO_REDIRECT_COUNT

  /**
   * The last received HTTP proxy response code to a CONNECT request. The returned value 
   * will be zero if no such response code was available.
   * @static
   */
  static var HTTP_CONNECTCODE = _curl.CURLINFO_HTTP_CONNECTCODE

  /**
   * A bitmask indicating the authentication method(s) available according to the 
   * previous response.
   * @static
   */
  static var HTTPAUTH_AVAIL = _curl.CURLINFO_HTTPAUTH_AVAIL

  /**
   * A bitmask indicating the authentication method(s) available according to the 
   * previous response.
   * @static
   */
  static var PROXYAUTH_AVAIL = _curl.CURLINFO_PROXYAUTH_AVAIL

  /**
   * The errno variable from a connect failure.  Note that the value is only set on 
   * failure, it is not reset upon a successful operation. The number is OS and system 
   * specific.
   * @static
   */
  static var OS_ERRNO = _curl.CURLINFO_OS_ERRNO

  /**
   * How many new connections `curl` had to create to achieve the previous transfer 
   * (only the successful connects are counted). Combined with `REDIRECT_COUNT` you are 
   * able to know how many times `curl` successfully reused existing connection(s) or not.
   * @static
   */
  static var NUM_CONNECTS = _curl.CURLINFO_NUM_CONNECTS

  /**
   * A list of all cookies curl knows (expired ones, too). If there are no cookies, an 
   * empty list is returned.
   * @static
   * 
   * > Cookies that were imported in the Set-Cookie format without a domain name may not 
   * > exported by this option.
   */
  static var COOKIELIST = _curl.CURLINFO_COOKIELIST

  /**
   * A string holding the path of the entry path. That is the initial path `curl` ended up 
   * in when logging on to the remote FTP server. This value is `nil` if something is wrong.
   * @static
   */
  static var FTP_ENTRY_PATH = _curl.CURLINFO_FTP_ENTRY_PATH

  /**
   * The URL a redirect would take you to if you would enable `FOLLOWLOCATION`. This can 
   * come very handy if you think using the built-in `curl` redirect logic isn't good
   * enough for you but you would still prefer to avoid implementing all the magic of 
   * figuring out the new URL.
   * @static
   */
  static var REDIRECT_URL = _curl.CURLINFO_REDIRECT_URL

  /**
   * A string holding the IP address of the most recent connection done with this `curl` 
   * handle. 
   * @static
   * @note This string may be IPv6 when that is enabled.
   */
  static var PRIMARY_IP = _curl.CURLINFO_PRIMARY_IP

  /**
   * The time, in seconds, it took from the start until the SSL/SSH connect/handshake to 
   * the remote host was completed.  This time is most often very near to the 
   * `PRETRANSFER_TIME` time, except for cases such as HTTP pipelining where the pretransfer
   * time can be delayed due to waits in line for the pipeline and more.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var APPCONNECT_TIME = _curl.CURLINFO_APPCONNECT_TIME

  /**
   * Lists with info about the certificate chain, assuming you had `Options.CERTINFO` 
   * enabled when the request was made. Information in each entry of the list is provided 
   * in a series of data in the format "name:content" where the content is for the specific 
   * named data.
   * @static
   */
  static var CERTINFO = _curl.CURLINFO_CERTINFO

  /**
   * The number `1` if the condition provided in the previous request didn't match 
   * (see `Options.TIMECONDITION`). Alas, if this returns a `1` you know that the 
   * reason you didn't get data in return is because it didn't fulfill the condition. 
   * This value will be zero if the condition was met. This can also return `1` if the 
   * server responded with a 304 HTTP status code, for example after sending a custom 
   * "If-Match-*" header.
   * @static
   */
  static var CONDITION_UNMET = _curl.CURLINFO_CONDITION_UNMET

  /**
   * A string holding the most recent RTSP Session ID.
   * @static
   * 
   * > Applications wishing to resume an RTSP session on another connection should 
   * > retrieve this info before closing the active connection.
   */
  static var RTSP_SESSION_ID = _curl.CURLINFO_RTSP_SESSION_ID

  /**
   * The next CSeq that will be used by the application.
   * @static
   */
  static var RTSP_CLIENT_CSEQ = _curl.CURLINFO_RTSP_CLIENT_CSEQ

  /**
   * The next CSeq that is expected by the application.
   * @static
   * 
   * > Applications wishing to resume an RTSP session on another connection should retrieve 
   * > this info before closing the active connection.
   */
  static var RTSP_SERVER_CSEQ = _curl.CURLINFO_RTSP_SERVER_CSEQ

  /**
   * The most recently received CSeq from the server.
   * @static
   */
  static var RTSP_CSEQ_RECV = _curl.CURLINFO_RTSP_CSEQ_RECV

  /**
   * The destination port of the most recent connection done with the `curl` instance.
   * @static
   */
  static var PRIMARY_PORT = _curl.CURLINFO_PRIMARY_PORT

  /**
   * A string holding the IP address of the local end of most recent connection done 
   * with the `curl` instance. 
   * @static
   * @note This string may be IPv6 when that is enabled.
   */
  static var LOCAL_IP = _curl.CURLINFO_LOCAL_IP

  /**
   * The local port number of the most recent connection done with the `curl` instance.
   * @static
   */
  static var LOCAL_PORT = _curl.CURLINFO_LOCAL_PORT

  /**
   * The HTTP version used in the last http connection.
   * @static
   */
  static var HTTP_VERSION = _curl.CURLINFO_HTTP_VERSION

  /**
   * The result of the certificate verification that was requested (using the 
   * `Options.PROXY_SSL_VERIFYPEER` option. 
   * @static
   * @note This is only used for HTTPS proxies.
   */
  static var PROXY_SSL_VERIFYRESULT = _curl.CURLINFO_PROXY_SSL_VERIFYRESULT

  /**
   * The protocol used in the last request.
   * @static
   */
  static var PROTOCOL = _curl.CURLINFO_SCHEME

  /**
   * A string holding the URL scheme used for the most recent connection done with 
   * this `curl` instance.
   * @static
   */
  static var SCHEME = _curl.CURLINFO_SCHEME

  /**
   * The total time in microseconds for the previous transfer, including name resolving, 
   * TCP connect etc.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var TOTAL_TIME_T = _curl.CURLINFO_TOTAL_TIME_T

  /**
   * The total time in microseconds from the start until the name resolving was completed.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var NAMELOOKUP_TIME_T = _curl.CURLINFO_NAMELOOKUP_TIME_T

  /**
   * The total time in microseconds from the start until the connection to the remote 
   * host (or proxy) was completed.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var CONNECT_TIME_T = _curl.CURLINFO_CONNECT_TIME_T

  /**
   * The total time in microseconds from the start until the file transfer is just about 
   * to begin. This includes all pre-transfer commands and negotiations that are specific 
   * to the particular protocol(s) involved. It does not involve the sending of the 
   * protocol- specific request that triggers a transfer.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var PRETRANSFER_TIME_T = _curl.CURLINFO_PRETRANSFER_TIME_T

  /**
   * The total time in microseconds from the start until the first byte is received by 
   * `curl`. This includes `PRETRANSFER_TIME_T` and also the time the server needs to 
   * calculate the result.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var STARTTRANSFER_TIME_T = _curl.CURLINFO_STARTTRANSFER_TIME_T

  /**
   * The total time in microseconds it took for all redirection steps include name lookup,
   * connect, pretransfer and transfer before final transaction was started. It contains 
   * the complete execution time for  multiple redirections.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var REDIRECT_TIME_T = _curl.CURLINFO_REDIRECT_TIME_T

  /**
   * The total time in microseconds from the start until the SSL/SSH connect/handshake 
   * to the remote host was completed.  This time is most often very near to the 
   * `PRETRANSFER_TIME_T` time, except for cases such as HTTP pipelining where the 
   * pretransfer time can be delayed due to waits in line for the pipeline and more.
   * @static
   * @note When a redirect is followed, the time from each request is added together.
   */
  static var APPCONNECT_TIME_T = _curl.CURLINFO_APPCONNECT_TIME_T

  /**
   * The number of seconds the HTTP server suggests the client should wait until the 
   * next request is issued. 
   * @static
   * @note The information from the "Retry-After:" header.
   * 
   * > While the HTTP header might contain a fixed date string, the `RETRY_AFTER` will 
   * > always return number of seconds to wait - or zero if there was no header or the 
   * > header couldn't be parsed.
   */
  static var RETRY_AFTER = _curl.CURLINFO_RETRY_AFTER
}
