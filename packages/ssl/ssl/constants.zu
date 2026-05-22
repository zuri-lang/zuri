#!-- part of the ssl module


import _ssl


/**
 * SSL_FILETYPE_PEM
 */
var SSL_FILETYPE_PEM = _ssl.SSL_FILETYPE_PEM()

/**
 * SSL_FILETYPE_ASN1
 */
var SSL_FILETYPE_ASN1 = _ssl.SSL_FILETYPE_ASN1()



/**
 * Server mode:
 * :  The server will not send a client certificate request to the client, 
 *    so the client will not send a certificate.
 * 
 * Client mode: 
 * :  If not using an anonymous cipher (by default disabled), 
 *    the server will send a certificate which will be checked. The handshake 
 *    will be continued regardless of the verification result.
 */
var SSL_VERIFY_NONE = _ssl.SSL_VERIFY_NONE()

/**
 * Server mode: 
 * :  The server sends a client certificate request to the client. 
 *    The certificate returned (if any) is checked. If the verification process fails, 
 *    the TLS/SSL handshake is immediately terminated with an alert message containing 
 *    the reason for the verification failure. The behaviour can be controlled by the 
 *    additional SSL_VERIFY_FAIL_IF_NO_PEER_CERT, SSL_VERIFY_CLIENT_ONCE and 
 *    SSL_VERIFY_POST_HANDSHAKE flags.
 * 
 * Client mode:
 * :  The server certificate is verified. If the verification process 
 *    fails, the TLS/SSL handshake is immediately terminated with an alert message 
 *    containing the reason for the verification failure. If no server certificate is sent, 
 *    because an anonymous cipher is used, SSL_VERIFY_PEER is ignored.
 */
var SSL_VERIFY_PEER = _ssl.SSL_VERIFY_PEER()

/**
 * Server mode:
 * :  If the client did not return a certificate, the TLS/SSL handshake is immediately 
 *    terminated with a "handshake failure" alert. This flag must be used together 
 *    with SSL_VERIFY_PEER.
 * 
 * Client mode: 
 * :  Ignored
 */
var SSL_VERIFY_FAIL_IF_NO_PEER_CERT = _ssl.SSL_VERIFY_FAIL_IF_NO_PEER_CERT()

/**
 * Server mode:
 * :  Only request a client certificate once during the connection. Do not 
 *    ask for a client certificate again during renegotiation or post-authentication if a 
 *    certificate was requested during the initial handshake. This flag must be used together 
 *    with SSL_VERIFY_PEER.
 * 
 * Client mode: 
 * :  Ignored
 */
var SSL_VERIFY_CLIENT_ONCE = _ssl.SSL_VERIFY_CLIENT_ONCE()

/**
 * Server mode: 
 * :  The server will not send a client certificate request during the initial 
 *    handshake, but will send the request via SSL_verify_client_post_handshake(). This allows 
 *    the SSL_CTX or SSL to be configured for post-handshake peer verification before the 
 *    handshake occurs. This flag must be used together with SSL_VERIFY_PEER. TLSv1.3 only; no 
 *    effect on pre-TLSv1.3 connections.
 * 
 * Client mode: 
 * :  Ignored
 */
var SSL_VERIFY_POST_HANDSHAKE = _ssl.SSL_VERIFY_POST_HANDSHAKE()



/**
 * TLS method
 */
var TLS_method = _ssl.TLS_method()

/**
 * TLS client method
 */
var TLS_client_method = _ssl.TLS_client_method()

/**
 * TLS server method
 */
var TLS_server_method = _ssl.TLS_server_method()

/**
 * SSLv23 method
 */
var SSLv23_method = _ssl.SSLv23_method()

/**
 * SSLv23 client method
 */
var SSLv23_client_method = _ssl.SSLv23_client_method()

/**
 * SSLv23 server method
 */
var SSLv23_server_method = _ssl.SSLv23_server_method()



/**
 * BIO_CLOSE
 */
var BIO_CLOSE = _ssl.BIO_CLOSE()

/**
 * BIO_NOCLOSE
 */
var BIO_NOCLOSE = _ssl.BIO_NOCLOSE()

/**
 * SSL BIO method f_ssl
 * 
 * > I/O performed on an SSL BIO communicates using the SSL protocol 
 * > with the SSLs read and write BIOs. If an SSL connection is not 
 * > established then an attempt is made to establish one on the first 
 * > I/O call.
 */
var BIO_f_ssl = _ssl.BIO_f_ssl()

/**
 * SSL BIO method connect
 * 
 * > Using connect BIOs, TCP/IP connections can be made and data 
 * > transferred using only BIO routines. In this way any platform 
 * > specific operations are hidden by the BIO abstraction.
 */
var BIO_s_connect = _ssl.BIO_s_connect()

/**
 * SSL BIO method accept
 * 
 * > Using accept BIOs, TCP/IP connections can be accepted and data 
 * > transferred using only BIO routines. In this way any platform specific 
 * > operations are hidden by the BIO abstraction.
 */
var BIO_s_accept = _ssl.BIO_s_accept()
