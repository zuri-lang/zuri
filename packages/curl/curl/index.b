/**
 * @module curl
 * 
 * This module provides native bindings to libcurl's client-side 
 * URL transfer library, supporting DICT, FILE, FTP, FTPS, Gopher, 
 * HTTP, HTTPS, IMAP, IMAPS, LDAP, LDAPS, POP3, POP3S, RTMP, RTSP, 
 * SCP, SFTP, SMTP, SMTPS, Telnet and TFTP. libcurl supports SSL 
 * certificates, HTTP POST, HTTP PUT, FTP uploading, HTTP form based 
 * upload, proxies, cookies, user+password authentication (Basic, 
 * Digest, NTLM, Negotiate, Kerberos), file transfer resume, http 
 * proxy tunneling and more!
 * 
 * The following example shows the basic usage that visits a website 
 * and prints the output.
 * 
 * ```zuri
 * import curl
 * 
 * var c = curl.Curl()
 * c.set_option(curl.Option.URL, 'www.google.com')
 * c.set_option(curl.Option.FOLLOWLOCATION)
 * 
 * echo c.send()
 * c.close()
 * ```
 * 
 * See [Libcurl's official documentation](https://curl.se/libcurl/)
 * for more information.
 *
 * @copyright 2021, Richard Ore and Zuri contributors
 */

import .options { Option }
import .filetypes { FileType }
import .infos { Info }
import .auth { Auth }
import .usessl { UseSSL }
import .curl { * }
import _curl


/**
 * The libcurl version.
 * @type string
 */
var version = _curl.version

/**
 * Returns the number of seconds since the Epoch, January 1st 1970 00:00:00 in 
 * the UTC time zone, for the date and time that the date_string parameter specifies.
 * 
 * **PARSING DATES AND TIMES**
 * 
 * A "date" is a string containing several items separated by whitespace.
 * The order of the items is immaterial.  A date string may contain many
 * flavors of items:
 * 
 * - **Calendar date**
 *    Can be specified several ways. Month names can only be three-letter english 
 *    abbreviations, numbers can be zero-prefixed and the year may use 2 or 4 digits.  
 *    
 *    Examples: 06 Nov 1994, 06-Nov-94 and Nov-94 6.
 * 
 * - **Time of the day**
 *    This string specifies the time on a given day. You must specify it with 6 digits 
 *    with two colons: HH:MM:SS. To not include the time in a date string, will make the 
 *    function assume 00:00:00.
 * 
 *    Example: 18:19:21.
 * 
 * - **Time zone**
 *    Specifies international time zone. There are a few acronyms supported, but in 
 *    general you should instead use the specific relative time compared to UTC. 
 *    Supported formats include: -1200, MST, +0100.
 * 
 * - **Day of the week items**
 *    Specifies a day of the week. Days of the week may be spelled out in full (using 
 *    english): Sunday, Monday, etc or they may be abbreviated to their first three 
 *    letters. This is usually not info that adds anything.
 * 
 * - **Pure numbers**
 *    If a decimal number of the form YYYYMMDD appears, then YYYY is read as the year, 
 *    MM as the month number and DD as the day of the month, for the specified calendar 
 *    date.
 * 
 * **EXAMPLES**
 * 
 * ```sh
 * Sun, 06 Nov 1994 08:49:37 GMT
 * Sunday, 06-Nov-94 08:49:37 GMT
 * Sun Nov  6 08:49:37 1994
 * 06 Nov 1994 08:49:37 GMT
 * 06-Nov-94 08:49:37 GMT
 * Nov  6 08:49:37 1994
 * 06 Nov 1994 08:49:37
 * 06-Nov-94 08:49:37
 * 1994 Nov 6 08:49:37
 * GMT 08:49:37 06-Nov-94 Sunday
 * 94 6 Nov 08:49:37
 * 1994 Nov 6
 * 06-Nov-94
 * Sun Nov 6 94
 * 1994.Nov.6
 * Sun/Nov/6/94/GMT
 * Sun, 06 Nov 1994 08:49:37 CET
 * 06 Nov 1994 08:49:37 EST
 * Sun, 12 Sep 2004 15:05:58 -0700
 * Sat, 11 Sep 2004 21:32:11 +0200
 * 20040912 15:05:58 -0700
 * 20040911 +0200
 * ```
 * 
 * **STANDARDS**
 * 
 * This parser was written to handle date formats specified in RFC 822 (including the 
 * update in RFC 1123) using time zone name or time zone delta and RFC 850 (obsoleted 
 * by RFC 1036) and ANSI C's asctime() format. These formats are the only ones RFC 7231 
 * says HTTP applications may use.
 * 
 * @param string date_string
 * @returns number
 */
def get_time(date_string) {
  return _curl.getdate(date_string)
}
