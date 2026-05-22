#include <zuri.h>
#ifdef _WIN32 // wrapping to disable annoying messages from getopt.h in ming2
#define message(ignore)
#endif
#include <unistd.h>
#ifdef _WIN32
#undef message
#endif
#include "ssl.h"

#ifdef _WIN32
# define _WINSOCK_DEPRECATED_NO_WARNINGS 1
# include <winsock2.h>
# include <sdkddkver.h>
# include <ws2tcpip.h>

# define sleep			_sleep
# define ioctl ioctlsocket
#else
# include <sys/socket.h>
# include <sys/ioctl.h>
#endif

#define SSL_BUF_SIZE 16384

DEFINE_SSL_CONSTANT(SSL_FILETYPE_PEM)
DEFINE_SSL_CONSTANT(SSL_FILETYPE_ASN1)

DEFINE_SSL_CONSTANT(SSL_VERIFY_NONE)
DEFINE_SSL_CONSTANT(SSL_VERIFY_PEER)
DEFINE_SSL_CONSTANT(SSL_VERIFY_FAIL_IF_NO_PEER_CERT)
DEFINE_SSL_CONSTANT(SSL_VERIFY_CLIENT_ONCE)
DEFINE_SSL_CONSTANT(SSL_VERIFY_POST_HANDSHAKE)

DEFINE_SSL_CONSTANT(BIO_CLOSE)
DEFINE_SSL_CONSTANT(BIO_NOCLOSE)

DEFINE_SSL_PTR_CONSTANT(TLS_method)
DEFINE_SSL_PTR_CONSTANT(TLS_client_method)
DEFINE_SSL_PTR_CONSTANT(TLS_server_method)
DEFINE_SSL_PTR_CONSTANT(SSLv23_method)
DEFINE_SSL_PTR_CONSTANT(SSLv23_client_method)
DEFINE_SSL_PTR_CONSTANT(SSLv23_server_method)

DEFINE_SSL_PTR_CONSTANT(BIO_f_ssl)
DEFINE_SSL_PTR_CONSTANT(BIO_s_connect)
DEFINE_SSL_PTR_CONSTANT(BIO_s_accept)

// Adapted from https://en.wikibooks.org/wiki/OpenSSL/Error_handling
static char *ossl_err_as_string() {
  BIO *bio = BIO_new(BIO_s_mem());
  ERR_print_errors(bio);

  char *buffer = NULL;
  size_t length = BIO_get_mem_data(bio, &buffer);
  char *ret = (char *)calloc(1, 1 + length);
  if(ret) {
    memcpy(ret, buffer, length);
  }
  BIO_free(bio);
  return ret;
}

int ssl___SSL_verify_true_cb(int preverify_ok, X509_STORE_CTX *x509_ctx) {
  return 1;
}

void info_callback(const SSL *ssl, int where, int ret) {
  const char *str;
  int w = where & ~SSL_ST_MASK;
  if (w & SSL_ST_CONNECT) str = "SSL_connect";
  else if (w & SSL_ST_ACCEPT) str = "SSL_accept";
  else str = "undefined";

  printf("%s:%s\n", str, SSL_state_string_long(ssl));
}

DECLARE_MODULE_METHOD(ssl_ctx) {
  ENFORCE_ARG_COUNT(ctx, 1);
  ENFORCE_ARG_TYPE(ctx, 0, IS_PTR);

  SSL_CTX *ctx = SSL_CTX_new((SSL_METHOD*) AS_PTR(args[0])->pointer);

  dbg(
      SSL_CTX_set_num_tickets(ctx, 1);
      SSL_CTX_set_info_callback(ctx, info_callback);
  );

  RETURN_PTR(ctx);
}

DECLARE_MODULE_METHOD(ssl_ctx_free) {
  ENFORCE_ARG_COUNT(ctx_free, 1);
  ENFORCE_ARG_TYPE(ctx_free, 0, IS_PTR);
  SSL_CTX_free((SSL_CTX*)AS_PTR(args[0])->pointer);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_ctx_load_certs) {
  ENFORCE_ARG_COUNT(load_certs, 3);
  ENFORCE_ARG_TYPE(load_certs, 0, IS_PTR); // the pointer
  ENFORCE_ARG_TYPE(load_certs, 1, IS_STRING); // cert file
  ENFORCE_ARG_TYPE(load_certs, 2, IS_STRING); // private key file

  SSL_CTX *ctx = (SSL_CTX*)AS_PTR(args[0])->pointer;
  z_obj_string *cert_file = AS_STRING(args[1]);
  z_obj_string *key_file = AS_STRING(args[2]);

  if(SSL_CTX_use_certificate_file(ctx, cert_file->chars, SSL_FILETYPE_PEM) <= 0) {
    RETURN_FALSE;
  }
  if(SSL_CTX_use_PrivateKey_file(ctx, key_file->chars, SSL_FILETYPE_PEM) <= 0) {
    RETURN_FALSE;
  }

  if(!SSL_CTX_check_private_key(ctx)) {
    RETURN_FALSE;
  }

  RETURN_TRUE;
}

DECLARE_MODULE_METHOD(ssl_ctx_set_verify) {
  ENFORCE_ARG_COUNT(ctx_set_verify, 3);
  ENFORCE_ARG_TYPE(ctx_set_verify, 0, IS_PTR);
  ENFORCE_ARG_TYPE(ctx_set_verify, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(ctx_set_verify, 2, IS_BOOL);


  SSL_CTX *ctx = (SSL_CTX*)AS_PTR(args[0])->pointer;
  int mode = AS_NUMBER(args[1]);
  bool disable = AS_BOOL(args[2]);

  if(disable && mode != SSL_VERIFY_NONE) {
    SSL_CTX_set_verify(ctx, mode, ssl___SSL_verify_true_cb);
  } else {
    SSL_CTX_set_verify(ctx, mode, NULL);
  }
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_ctx_set_verify_locations) {
  ENFORCE_ARG_COUNT(ctx_set_verify_locations, 2);
  ENFORCE_ARG_TYPE(ctx_set_verify_locations, 0, IS_PTR);
  ENFORCE_ARG_TYPE(ctx_set_verify_locations, 1, IS_STRING);

  SSL_CTX *ctx = (SSL_CTX*)AS_PTR(args[0])->pointer;
  RETURN_BOOL(SSL_CTX_load_verify_locations(ctx, AS_C_STRING(args[1]), NULL) == 1);
}

DECLARE_MODULE_METHOD(ssl_set_ciphers) {
  ENFORCE_ARG_COUNT(set_ciphers, 2);
  ENFORCE_ARG_TYPE(set_ciphers, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_ciphers, 1, IS_STRING);

  SSL_CTX *ctx = (SSL_CTX*)AS_PTR(args[0])->pointer;
  z_obj_string *string = AS_STRING(args[1]);

  RETURN_BOOL(SSL_CTX_set_cipher_list(ctx, (const char*)string->chars) > 0);
}

DECLARE_MODULE_METHOD(ssl_new) {
  ENFORCE_ARG_COUNT(new, 1);
  ENFORCE_ARG_TYPE(new, 0, IS_PTR);

  SSL_CTX *ctx = (SSL_CTX*)AS_PTR(args[0])->pointer;

  SSL *ssl = SSL_new(ctx);

  dbg(
      SSL_set_msg_callback(ssl,SSL_trace);
      SSL_set_msg_callback_arg(ssl, BIO_new_fp(stdout,0));
  );


  RETURN_PTR(ssl);
}

DECLARE_MODULE_METHOD(ssl_ssl_free) {
  ENFORCE_ARG_COUNT(ssl_free, 1);
  ENFORCE_ARG_TYPE(ssl_free, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  SSL_free(ssl);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_connect_state) {
  ENFORCE_ARG_COUNT(set_connect_state, 1);
  ENFORCE_ARG_TYPE(set_connect_state, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  SSL_set_connect_state(ssl);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_accept_state) {
  ENFORCE_ARG_COUNT(set_accept_state, 1);
  ENFORCE_ARG_TYPE(set_accept_state, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  SSL_set_accept_state(ssl);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_tlsext_host_name) {
  ENFORCE_ARG_COUNT(set_tlsext_host_name, 2);
  ENFORCE_ARG_TYPE(set_tlsext_host_name, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_tlsext_host_name, 1, IS_STRING);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  RETURN_BOOL(SSL_set_tlsext_host_name(ssl, AS_C_STRING(args[1])) == 0);
}

static char *ASN1_TIME_to_string(ASN1_TIME *a) {
  char timebuf[256];
  BIO *bio = BIO_new(BIO_s_mem());

  ASN1_TIME_print(bio, a);
  BIO_gets(bio, timebuf, sizeof(timebuf));

  BIO_free(bio);

  size_t len = strlen(timebuf);
  char *buffer = malloc(sizeof(char) * (len + 1));
  memcpy(buffer, timebuf, len);
  buffer[len] = '\0';

  return buffer;
}

DECLARE_MODULE_METHOD(ssl_get_peer_certificate) {
  ENFORCE_ARG_COUNT(get_peer_certificate, 1);
  ENFORCE_ARG_TYPE(get_peer_certificate, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;

  X509 *cert = SSL_get_peer_certificate(ssl);
  if(!cert) {
    RETURN_NIL;
  }

  z_obj_dict *dict = (z_obj_dict *)GC(new_dict(vm));

  // Get certificate subject
  X509_NAME *subject = X509_get_subject_name(cert);
  if(subject) {
    char *subject_name = X509_NAME_oneline(subject, NULL, 0);
    if(subject_name) {
      dict_set_entry(vm, dict, GC_L_STRING("subject_name", 12), GC_TT_STRING(subject_name));
    } else {
      dict_set_entry(vm, dict, GC_L_STRING("subject_name", 12), EMPTY_STRING_VAL);
    }
  } else {
    dict_set_entry(vm, dict, GC_L_STRING("subject_name", 12), EMPTY_STRING_VAL);
  }

  // Get certificate issuer
  X509_NAME *issuer = X509_get_issuer_name(cert);
  if(issuer) {
    char *issuer_name = X509_NAME_oneline(issuer, NULL, 0);
    if(issuer_name) {
      dict_set_entry(vm, dict, GC_L_STRING("issuer_name", 11), GC_TT_STRING(issuer_name));
    } else {
      dict_set_entry(vm, dict, GC_L_STRING("issuer_name", 11), EMPTY_STRING_VAL);
    }
  } else {
    dict_set_entry(vm, dict, GC_L_STRING("issuer_name", 11), EMPTY_STRING_VAL);
  }

  // Get certificate serial number
  ASN1_INTEGER *serial = X509_get_serialNumber(cert);
  if(serial) {
    BIGNUM *serial_bn = ASN1_INTEGER_to_BN(serial, NULL);
    if(serial_bn) {
      char *serial_number = BN_bn2dec(serial_bn);
      if(serial_number) {
        dict_set_entry(vm, dict, GC_L_STRING("serial_number", 12), GC_STRING(serial_number));
        OPENSSL_free(serial_number);
      } else {
        dict_set_entry(vm, dict, GC_L_STRING("serial_number", 12), EMPTY_STRING_VAL);
      }
      BN_free(serial_bn);
    } else {
        dict_set_entry(vm, dict, GC_L_STRING("serial_number", 12), EMPTY_STRING_VAL);
    }
  } else {
    dict_set_entry(vm, dict, GC_L_STRING("serial_number", 12), EMPTY_STRING_VAL);
  }

  // Get certificate not before and not after dates
  ASN1_TIME *notBefore = X509_get_notBefore(cert);
  ASN1_TIME *notAfter = X509_get_notAfter(cert);
  // char *not_before = ASN1_TIME_to_string(notBefore);
  // char *not_after = ASN1_TIME_to_string(notAfter);
  dict_set_entry(vm, dict, GC_L_STRING("not_before", 10), GC_TT_STRING((char *)notBefore->data));
  dict_set_entry(vm, dict, GC_L_STRING("not_after", 9), GC_TT_STRING((char *)notAfter->data));


  // Get certificate signature algorithm
  int nid = X509_get_signature_nid(cert);
  const char *algorithm = OBJ_nid2sn(nid);
  if (algorithm) {
    dict_set_entry(vm, dict, GC_L_STRING("algorithm", 9), GC_STRING(algorithm));
  } else {
    dict_set_entry(vm, dict, GC_L_STRING("algorithm", 9), EMPTY_STRING_VAL);
  }

  // Get certificate public key
  EVP_PKEY *pubkey = X509_get_pubkey(cert);
  if(pubkey) {
    BIO *bio = BIO_new(BIO_s_mem());
    if(bio) {
      PEM_write_bio_PUBKEY(bio, pubkey);
      BUF_MEM *bptr;
      BIO_get_mem_ptr(bio, &bptr);
      if(bptr) {
        char *pubkey_str = malloc(bptr->length + 1);
        if(pubkey_str) {
          memcpy(pubkey_str, bptr->data, bptr->length);
          pubkey_str[bptr->length] = '\0';
          dict_set_entry(vm, dict, GC_L_STRING("public_key", 10), GC_TT_STRING(pubkey_str));
        } else {
          dict_set_entry(vm, dict, GC_L_STRING("public_key", 10), EMPTY_STRING_VAL);
        }
      } else {
        dict_set_entry(vm, dict, GC_L_STRING("public_key", 10), EMPTY_STRING_VAL);
      }

      BIO_free(bio);
    } else {
      dict_set_entry(vm, dict, GC_L_STRING("public_key", 10), EMPTY_STRING_VAL);
    }
  } else {
    dict_set_entry(vm, dict, GC_L_STRING("public_key", 10), EMPTY_STRING_VAL);
  }

  // Get certificate extensions
  z_obj_dict *extensions = (z_obj_dict *)GC(new_dict(vm));
  const STACK_OF(X509_EXTENSION) *exts = X509_get0_extensions(cert);
  if(exts) {
    for (int i = 0; i < sk_X509_EXTENSION_num(exts); i++) {
      X509_EXTENSION *ext = sk_X509_EXTENSION_value(exts, i);
      if(ext) {
        ASN1_OBJECT *obj = X509_EXTENSION_get_object(ext);
        if(obj) {
          char *ext_name = (char *)OBJ_nid2sn(OBJ_obj2nid(obj));
          if(ext_name) {
            BIO *bio = BIO_new(BIO_s_mem());
            if(bio) {
              X509V3_EXT_print(bio, ext, 0, 0);
              BUF_MEM *bptr;
              BIO_get_mem_ptr(bio, &bptr);
              if(bptr) {
                char *ext_value_str = malloc(bptr->length + 1);
                if(ext_value_str) {
                  memcpy(ext_value_str, bptr->data, bptr->length);
                  ext_value_str[bptr->length] = '\0';

                  dict_set_entry(vm, extensions, GC_STRING(ext_name), GC_TT_STRING(ext_value_str));
                }
              }
              BIO_free(bio);
            }
          }
        }
      }
    }
  }
  dict_set_entry(vm, dict, GC_L_STRING("extensions", 10), OBJ_VAL(extensions));

  // Free resources
  X509_free(cert);

  RETURN_OBJ(dict);
}

DECLARE_MODULE_METHOD(ssl_new_bio) {
  ENFORCE_ARG_COUNT(new_bio, 1);
  ENFORCE_ARG_TYPE(new_bio, 0, IS_PTR);

  BIO_METHOD *type = (BIO_METHOD*)AS_PTR(args[0])->pointer;
  RETURN_PTR(BIO_new(type));
}

DECLARE_MODULE_METHOD(ssl_bio_set_ssl) {
  ENFORCE_ARG_COUNT(bio_set_ssl, 3);
  ENFORCE_ARG_TYPE(bio_set_ssl, 0, IS_PTR); //bio
  ENFORCE_ARG_TYPE(bio_set_ssl, 1, IS_PTR); // ssl
  ENFORCE_ARG_TYPE(bio_set_ssl, 2, IS_NUMBER); // args

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  SSL *ssl = (SSL*)AS_PTR(args[1])->pointer;
  BIO_set_ssl(bio, ssl, (int)AS_NUMBER(args[2]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_conn_hostname) {
  ENFORCE_ARG_COUNT(set_conn_hostname, 2);
  ENFORCE_ARG_TYPE(set_conn_hostname, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_conn_hostname, 1, IS_STRING);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_conn_hostname(bio, AS_C_STRING(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_accept_name) {
  ENFORCE_ARG_COUNT(set_accept_name, 2);
  ENFORCE_ARG_TYPE(set_accept_name, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_accept_name, 1, IS_STRING);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_accept_name(bio, AS_C_STRING(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_conn_address) {
  ENFORCE_ARG_COUNT(set_conn_address, 2);
  ENFORCE_ARG_TYPE(set_conn_address, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_conn_address, 1, IS_STRING);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_conn_address(bio, AS_C_STRING(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_conn_port) {
  ENFORCE_ARG_COUNT(set_conn_port, 2);
  ENFORCE_ARG_TYPE(set_conn_port, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_conn_port, 1, IS_STRING);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_conn_port(bio, AS_C_STRING(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_accept_port) {
  ENFORCE_ARG_COUNT(set_accept_port, 2);
  ENFORCE_ARG_TYPE(set_accept_port, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_accept_port, 1, IS_STRING);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_accept_port(bio, AS_C_STRING(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_conn_family) {
  ENFORCE_ARG_COUNT(set_conn_family, 2);
  ENFORCE_ARG_TYPE(set_conn_family, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_conn_family, 1, IS_NUMBER);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_conn_ip_family(bio, AS_NUMBER(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_set_accept_family) {
  ENFORCE_ARG_COUNT(set_accept_family, 2);
  ENFORCE_ARG_TYPE(set_accept_family, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_accept_family, 1, IS_NUMBER);

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_accept_ip_family(bio, AS_NUMBER(args[1]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_get_conn_hostname) {
  ENFORCE_ARG_COUNT(get_conn_hostname, 1);
  ENFORCE_ARG_TYPE(get_conn_hostname, 0, IS_PTR);
  char *hostname = (char *)BIO_get_conn_hostname((BIO*)AS_PTR(args[0])->pointer);
  RETURN_STRING(hostname);
}

DECLARE_MODULE_METHOD(ssl_get_accept_name) {
  ENFORCE_ARG_COUNT(get_accept_name, 1);
  ENFORCE_ARG_TYPE(get_accept_name, 0, IS_PTR);
  char *hostname = (char *)BIO_get_accept_name((BIO*)AS_PTR(args[0])->pointer);
  RETURN_STRING(hostname);
}

DECLARE_MODULE_METHOD(ssl_get_conn_address) {
  ENFORCE_ARG_COUNT(get_conn_address, 1);
  ENFORCE_ARG_TYPE(get_conn_address, 0, IS_PTR);
  char *address = (char *)BIO_get_conn_address((BIO*)AS_PTR(args[0])->pointer);
  RETURN_STRING(address);
}

DECLARE_MODULE_METHOD(ssl_get_conn_port) {
  ENFORCE_ARG_COUNT(get_conn_port, 1);
  ENFORCE_ARG_TYPE(get_conn_port, 0, IS_PTR);
  char *port = (char *)BIO_get_conn_port((BIO*)AS_PTR(args[0])->pointer);
  RETURN_STRING(port);
}

DECLARE_MODULE_METHOD(ssl_get_accept_port) {
  ENFORCE_ARG_COUNT(get_accept_port, 1);
  ENFORCE_ARG_TYPE(get_accept_port, 0, IS_PTR);
  char *port = (char *)BIO_get_accept_port((BIO*)AS_PTR(args[0])->pointer);
  RETURN_STRING(port);
}

DECLARE_MODULE_METHOD(ssl_get_conn_family) {
  ENFORCE_ARG_COUNT(get_conn_family, 1);
  ENFORCE_ARG_TYPE(get_conn_family, 0, IS_PTR);
  RETURN_NUMBER(BIO_get_conn_ip_family((BIO*)AS_PTR(args[0])->pointer));
}

DECLARE_MODULE_METHOD(ssl_get_accept_family) {
  ENFORCE_ARG_COUNT(get_accept_family, 1);
  ENFORCE_ARG_TYPE(get_accept_family, 0, IS_PTR);
  RETURN_NUMBER(BIO_get_accept_ip_family((BIO*)AS_PTR(args[0])->pointer));
}

DECLARE_MODULE_METHOD(ssl_set_nbio) {
  ENFORCE_ARG_COUNT(set_nbio, 2);
  ENFORCE_ARG_TYPE(set_nbio, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_nbio, 1, IS_BOOL);
  BIO_set_nbio((BIO*)AS_PTR(args[0])->pointer, AS_BOOL(args[1]) ? 1 : 0);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_bio_push) {
  ENFORCE_ARG_COUNT(push, 2);
  ENFORCE_ARG_TYPE(push, 0, IS_PTR); // input
  ENFORCE_ARG_TYPE(push, 1, IS_PTR); // output
  BIO *in = (BIO*)AS_PTR(args[0])->pointer;
  BIO *out = (BIO*)AS_PTR(args[1])->pointer;
  RETURN_PTR(BIO_push(in, out));
}

DECLARE_MODULE_METHOD(ssl_bio_pop) {
  ENFORCE_ARG_COUNT(push, 2);
  ENFORCE_ARG_TYPE(push, 0, IS_PTR); // input
  BIO *in = (BIO*)AS_PTR(args[0])->pointer;
  RETURN_PTR(BIO_pop(in));
}

DECLARE_MODULE_METHOD(ssl_bio_write) {
  ENFORCE_ARG_COUNT(bio_write, 2);
  ENFORCE_ARG_TYPE(bio_write, 0, IS_PTR);
  ENFORCE_ARG_TYPE(bio_write, 1, IS_STRING); // data

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  z_obj_string *string = AS_STRING(args[1]);
  char *p = string->chars;
  int len = string->length;

  int off = 0, total = 0;
  for (;;) {
    int i = BIO_write(bio, &(p[off]), len);
    total += i;
    if (i <= 0) {
      if (BIO_should_retry(bio)) {
//        sleep(1);
        continue;
      }
      RETURN_NUMBER(-1);
    }
    off += i;
    len -= i;
    if (len <= 0)
      break;
  }

  RETURN_NUMBER(total);
}

DECLARE_MODULE_METHOD(ssl_write) {
  ENFORCE_ARG_COUNT(write, 2);
  ENFORCE_ARG_TYPE(write, 0, IS_PTR);
  ENFORCE_ARG_TYPE(write, 1, IS_BYTES); // data

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  z_obj_bytes *bytes = AS_BYTES(args[1]);

  SSL_set_mode(ssl, SSL_MODE_ENABLE_PARTIAL_WRITE);
  ERR_clear_error();

  unsigned char *buffer = (unsigned char*)bytes->bytes.bytes;
  int total = bytes->bytes.count;
  int processed = 0;

  do {
    int diff = total - processed;
    int write_size = diff < SSL_BUF_SIZE ? diff : SSL_BUF_SIZE;

    int rc = SSL_write(ssl, buffer + processed, write_size);
    if(rc < 0) {
      int error = SSL_get_error(ssl, rc);
      if(error == SSL_ERROR_WANT_WRITE) {
        continue;
      } else if(error == SSL_ERROR_ZERO_RETURN || error == SSL_ERROR_NONE) {
        break;
      } else {
        RETURN_FALSE; // error occurred
      }
    } else {
      processed += rc;
      if(processed == total) {
        break;
      }
    }
  } while(true);

  RETURN_TRUE;
}

DECLARE_MODULE_METHOD(ssl_read) {
  ENFORCE_ARG_COUNT(read, 3);
  ENFORCE_ARG_TYPE(read, 0, IS_PTR);
  ENFORCE_ARG_TYPE(read, 1, IS_NUMBER);
  ENFORCE_ARG_TYPE(read, 2, IS_BOOL);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  int length = AS_NUMBER(args[1]);
  bool is_blocking = AS_BOOL(args[2]);

  int total = 0;
  char buffer[SSL_BUF_SIZE];
  ERR_clear_error();

  int ssl_fd = SSL_get_fd(ssl);

#ifdef SSL_OP_IGNORE_UNEXPECTED_EOF
  SSL_set_options(ssl, SSL_OP_IGNORE_UNEXPECTED_EOF);
#endif

  fd_set read_fds;
  FD_ZERO(&read_fds);
  FD_SET(ssl_fd, &read_fds);

  struct timeval timeout;
  if(is_blocking) {
    int option_length = sizeof(timeout);

  #ifndef _WIN32
    int rc = getsockopt(ssl_fd, SOL_SOCKET, SO_RCVTIMEO, &timeout, (socklen_t *) &option_length);
  #else
    int rc = getsockopt(ssl_fd, SOL_SOCKET, SO_RCVTIMEO, (char *)&timeout, (socklen_t*)&option_length);
  #endif // !_WIN32

    if (rc != 0 || sizeof(timeout) != option_length || (timeout.tv_sec == 0 && timeout.tv_usec == 0)) {
      // set default timeout to 0 seconds
      timeout.tv_sec = 0;
      timeout.tv_usec = 0;
    }
  } else {
    // set default timeout to 0.05 seconds
    timeout.tv_sec = 0;
    timeout.tv_usec = 50000;
  }

  int ret = select(ssl_fd + 1, &read_fds, NULL, NULL, &timeout);
  if (ret == 0 || !FD_ISSET(ssl_fd, &read_fds)) {
    RETURN_NIL;
  } else if (ret < 0) {
    // Error
  }

  char *data = ALLOCATE(char, 1);
  memset(data, 0, sizeof(char));

  do {
    int diff = length - total;
    int read_count = length == -1 ? SSL_BUF_SIZE : (
      diff < SSL_BUF_SIZE ? diff : SSL_BUF_SIZE
    );

    ERR_clear_error();
    memset(buffer, 0, sizeof(buffer));

    int bytes = SSL_read(ssl, buffer, read_count);
    while(bytes > 0) {
      data = GROW_ARRAY(char, data, total, total + bytes + 1);
      if(data == NULL) {
        RETURN_ERROR("device out of memory.");
      }

      memcpy(data + total, buffer, bytes);
      total += bytes;
      data[total] = '\0';

      memset(buffer, 0, sizeof(buffer));
      ERR_clear_error();

      bytes = SSL_read(ssl, buffer, read_count);
    }

    int error = SSL_get_error(ssl, bytes);
    if(error == SSL_ERROR_SSL) {
      FREE(char, data);
      RETURN_NIL;
    }

    if(bytes == 0) {
      if(error == SSL_ERROR_WANT_READ) {
        continue;
      } else if(error == SSL_ERROR_WANT_WRITE) {
        SSL_do_handshake(ssl); // must want an handshake
        continue;
      } else if(error == SSL_ERROR_ZERO_RETURN || error == SSL_ERROR_NONE) {
        SSL_shutdown(ssl);
        break;
      }
    } else {
      if(select(ssl_fd + 1, &read_fds, NULL, NULL, &timeout) > 0) {
        continue;
      }

      if(SSL_pending(ssl) > 0) {
        continue;
      }
    }

    break;
  } while (1);

  RETURN_T_STRING(data, total > length && length != -1 ? length : total);
}

DECLARE_MODULE_METHOD(ssl_bio_read) {
  ENFORCE_ARG_COUNT(bio_read, 2);
  ENFORCE_ARG_TYPE(bio_read, 0, IS_PTR);
  ENFORCE_ARG_TYPE(bio_read, 1, IS_NUMBER);
  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  int buffer_length = AS_NUMBER(args[1]);

  char *data = ALLOCATE(char, 0);
  int total = 0;

  for (;;) {
    char buf[buffer_length];
    int i = BIO_read(bio, buf, buffer_length);
    if (i == 0 || (i == -1 && total > 0))
      break;
    if (i < 0) {
      if (BIO_should_retry(bio)) {
        sleep(1);
        continue;
      }
      FREE(char, data);
      RETURN_STRING(""); // error...
    }

    data = GROW_ARRAY(char, data, total, total + i + 1);

    memcpy(data + total, buf, i);
    total += i;
  }

  data[total] = '\0';
  RETURN_T_STRING(data, total);
}

DECLARE_MODULE_METHOD(ssl_should_retry) {
  ENFORCE_ARG_COUNT(should_retry, 1);
  ENFORCE_ARG_TYPE(should_retry, 0, IS_PTR);
  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  RETURN_BOOL(BIO_should_retry(bio));
}

DECLARE_MODULE_METHOD(ssl_do_connect) {
  ENFORCE_ARG_COUNT(should_retry, 1);
  ENFORCE_ARG_TYPE(should_retry, 0, IS_PTR);
  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;

  int result;
  for(;;) {
    result = BIO_do_connect(bio);
    if(result == 1) break;
    if (BIO_should_retry(bio)) {
      continue;
    }
    break;
  }

  RETURN_NUMBER(result);
}

DECLARE_MODULE_METHOD(ssl_error) {
  ENFORCE_ARG_RANGE(error, 1, 2);
  ENFORCE_ARG_TYPE(error, 0, IS_PTR);
  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  int ret = -1;
  if(arg_count == 2) {
    ENFORCE_ARG_TYPE(error_string, 1, IS_NUMBER);
    ret = AS_NUMBER(args[1]);
  }
  RETURN_NUMBER(SSL_get_error(ssl, ret));
}

DECLARE_MODULE_METHOD(ssl_error_string) {
  ENFORCE_ARG_RANGE(error_string, 1, 2);
  ENFORCE_ARG_TYPE(error_string, 0, IS_PTR);
  int ret = -1;
  if(arg_count == 2) {
    ENFORCE_ARG_TYPE(error_string, 1, IS_NUMBER);
    ret = AS_NUMBER(args[1]);
  }
  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  int code = SSL_get_error(ssl, ret);
  if(code != SSL_ERROR_SYSCALL) {
    // const char *err = ERR_reason_error_string(ERR_get_error());
    char *err = ossl_err_as_string();
    RETURN_STRING(err);
  } else if(errno > 0) {
    char *error = strerror(errno);
    RETURN_STRING(error);
  } else {
    RETURN_VALUE(EMPTY_STRING_VAL);
  }
}

DECLARE_MODULE_METHOD(ssl_accept) {
  ENFORCE_ARG_COUNT(accept, 1);
  ENFORCE_ARG_TYPE(accept, 0, IS_PTR);
  ERR_clear_error();
  RETURN_NUMBER(SSL_accept((SSL*)AS_PTR(args[0])->pointer));
}

DECLARE_MODULE_METHOD(ssl_connect) {
  ENFORCE_ARG_COUNT(connect, 1);
  ENFORCE_ARG_TYPE(connect, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  ERR_clear_error();

  int res;
  do {
    res = SSL_connect(ssl);
    int error = SSL_get_error(ssl, res);
    if(error != SSL_ERROR_WANT_READ && error != SSL_ERROR_WANT_WRITE && error != SSL_ERROR_WANT_CONNECT) {
      if(error == SSL_ERROR_SSL || error == SSL_ERROR_SYSCALL) {
        RETURN_SSL_ERROR();
      }
      break;
    }
  } while(res == -1);
  RETURN_BOOL(res > 0);
}

DECLARE_MODULE_METHOD(ssl_do_accept) {
  ENFORCE_ARG_COUNT(do_accept, 1);
  ENFORCE_ARG_TYPE(do_accept, 0, IS_PTR);
  ERR_clear_error();
  RETURN_BOOL(BIO_do_accept((BIO*)AS_PTR(args[0])->pointer));
}

DECLARE_MODULE_METHOD(ssl_free) {
  ENFORCE_ARG_COUNT(free, 1);
  ENFORCE_ARG_TYPE(free, 0, IS_PTR);
  BIO_free_all((BIO*)AS_PTR(args[0])->pointer);
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_bio_get_fd) {
  ENFORCE_ARG_COUNT(bio_get_fd, 1);
  ENFORCE_ARG_TYPE(bio_get_fd, 0, IS_PTR);
  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  int fd;
  BIO_get_fd(bio, &fd);
  RETURN_NUMBER(fd);
}

DECLARE_MODULE_METHOD(ssl_bio_set_fd) {
  ENFORCE_ARG_COUNT(bio_set_fd, 3);
  ENFORCE_ARG_TYPE(bio_set_fd, 0, IS_PTR);
  ENFORCE_ARG_TYPE(bio_set_fd, 1, IS_NUMBER); // fd
  ENFORCE_ARG_TYPE(bio_set_fd, 2, IS_NUMBER); // opt

  BIO *bio = (BIO*)AS_PTR(args[0])->pointer;
  BIO_set_fd(bio, AS_NUMBER(args[1]), AS_NUMBER(args[2]));
  RETURN;
}

DECLARE_MODULE_METHOD(ssl_get_fd) {
  ENFORCE_ARG_COUNT(get_fd, 1);
  ENFORCE_ARG_TYPE(get_fd, 0, IS_PTR);
  SSL *bio = (SSL*)AS_PTR(args[0])->pointer;
  RETURN_NUMBER(SSL_get_fd(bio));
}

DECLARE_MODULE_METHOD(ssl_set_fd) {
  ENFORCE_ARG_COUNT(set_fd, 2);
  ENFORCE_ARG_TYPE(set_fd, 0, IS_PTR);
  ENFORCE_ARG_TYPE(set_fd, 1, IS_NUMBER); // fd

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  RETURN_BOOL(SSL_set_fd(ssl, AS_NUMBER(args[1])) == 1);
}

DECLARE_MODULE_METHOD(ssl_shutdown) {
  ENFORCE_ARG_COUNT(shutdown, 1);
  ENFORCE_ARG_TYPE(shutdown, 0, IS_PTR);

  SSL *ssl = (SSL*)AS_PTR(args[0])->pointer;
  RETURN_BOOL(SSL_shutdown(ssl) >= 0);
}


void __ssl_module_preloader(z_vm *vm) {
#ifdef WATT32
  dbug_init();
  sock_init();
#endif
  SSL_library_init();
  SSL_load_error_strings();
  OpenSSL_add_all_algorithms();
  OpenSSL_add_all_ciphers();
}


CREATE_MODULE_LOADER(ssl) {

  static z_field_reg module_fields[] = {
      {NULL,       false, NULL},
  };

  static z_func_reg module_functions[] = {
      /**
       * constants
       */
      GET_SSL_CONSTANT(SSL_FILETYPE_PEM),
      GET_SSL_CONSTANT(SSL_FILETYPE_ASN1),

      /**
       * SSL Verify
       */
      GET_SSL_CONSTANT(SSL_VERIFY_NONE),
      GET_SSL_CONSTANT(SSL_VERIFY_PEER),
      GET_SSL_CONSTANT(SSL_VERIFY_FAIL_IF_NO_PEER_CERT),
      GET_SSL_CONSTANT(SSL_VERIFY_CLIENT_ONCE),
      GET_SSL_CONSTANT(SSL_VERIFY_POST_HANDSHAKE),

      /**
       * constant methods
       */
      GET_SSL_CONSTANT(TLS_method),
      GET_SSL_CONSTANT(TLS_client_method),
      GET_SSL_CONSTANT(TLS_server_method),
      GET_SSL_CONSTANT(SSLv23_method),
      GET_SSL_CONSTANT(SSLv23_client_method),
      GET_SSL_CONSTANT(SSLv23_server_method),

      /**
       * BIOs
       */
      GET_SSL_CONSTANT(BIO_CLOSE),
      GET_SSL_CONSTANT(BIO_NOCLOSE),
      GET_SSL_CONSTANT(BIO_f_ssl),
      GET_SSL_CONSTANT(BIO_s_connect),
      GET_SSL_CONSTANT(BIO_s_accept),

      /**
       * methods
       */
      {"ctx",   true,  GET_MODULE_METHOD(ssl_ctx)},
      {"ctx_free",   true,  GET_MODULE_METHOD(ssl_ctx_free)},
      {"ctx_set_verify",   true,  GET_MODULE_METHOD(ssl_ctx_set_verify)},
      {"ctx_set_verify_locations",   true,  GET_MODULE_METHOD(ssl_ctx_set_verify_locations)},
      {"load_certs",   true,  GET_MODULE_METHOD(ssl_ctx_load_certs)},
      {"new",   true,  GET_MODULE_METHOD(ssl_new)},
      {"ssl_free",   true,  GET_MODULE_METHOD(ssl_ssl_free)},
      {"set_connect_state",   true,  GET_MODULE_METHOD(ssl_set_connect_state)},
      {"set_accept_state",   true,  GET_MODULE_METHOD(ssl_set_accept_state)},
      {"set_tlsext_host_name",   true,  GET_MODULE_METHOD(ssl_set_tlsext_host_name)},
      {"new_bio",   true,  GET_MODULE_METHOD(ssl_new_bio)},
      {"set_ssl",   true,  GET_MODULE_METHOD(ssl_bio_set_ssl)},
      {"set_conn_hostname",   true,  GET_MODULE_METHOD(ssl_set_conn_hostname)},
      {"set_accept_name",   true,  GET_MODULE_METHOD(ssl_set_accept_name)},
      {"set_conn_address",   true,  GET_MODULE_METHOD(ssl_set_conn_address)},
      {"set_conn_port",   true,  GET_MODULE_METHOD(ssl_set_conn_port)},
      {"set_accept_port",   true,  GET_MODULE_METHOD(ssl_set_accept_port)},
      {"set_conn_family",   true,  GET_MODULE_METHOD(ssl_set_conn_family)},
      {"set_accept_family",   true,  GET_MODULE_METHOD(ssl_set_accept_family)},
      {"get_conn_hostname",   true,  GET_MODULE_METHOD(ssl_get_conn_hostname)},
      {"get_accept_name",   true,  GET_MODULE_METHOD(ssl_get_accept_name)},
      {"get_conn_address",   true,  GET_MODULE_METHOD(ssl_get_conn_address)},
      {"get_conn_port",   true,  GET_MODULE_METHOD(ssl_get_conn_port)},
      {"get_accept_port",   true,  GET_MODULE_METHOD(ssl_get_accept_port)},
      {"get_conn_family",   true,  GET_MODULE_METHOD(ssl_get_conn_family)},
      {"get_accept_family",   true,  GET_MODULE_METHOD(ssl_get_accept_family)},
      {"get_fd",   true,  GET_MODULE_METHOD(ssl_get_fd)},
      {"bio_get_fd",   true,  GET_MODULE_METHOD(ssl_bio_get_fd)},
      {"set_nbio",   true,  GET_MODULE_METHOD(ssl_set_nbio)},
      {"set_fd",   true,  GET_MODULE_METHOD(ssl_set_fd)},
      {"bio_set_fd",   true,  GET_MODULE_METHOD(ssl_bio_set_fd)},
      {"connect",   true,  GET_MODULE_METHOD(ssl_connect)},
      {"accept",   true,  GET_MODULE_METHOD(ssl_accept)},
      {"do_accept",   true,  GET_MODULE_METHOD(ssl_do_accept)},
      {"push",   true,  GET_MODULE_METHOD(ssl_bio_push)},
      {"pop",   true,  GET_MODULE_METHOD(ssl_bio_pop)},
      {"write",   true,  GET_MODULE_METHOD(ssl_write)},
      {"read",   true,  GET_MODULE_METHOD(ssl_read)},
      {"bio_write",   true,  GET_MODULE_METHOD(ssl_bio_write)},
      {"bio_read",   true,  GET_MODULE_METHOD(ssl_bio_read)},
      {"do_connect",   true,  GET_MODULE_METHOD(ssl_do_connect)},
      {"should_retry",   true,  GET_MODULE_METHOD(ssl_should_retry)},
      {"error",   true,  GET_MODULE_METHOD(ssl_error)},
      {"error_string",   true,  GET_MODULE_METHOD(ssl_error_string)},
      {"free",   true,  GET_MODULE_METHOD(ssl_free)},
      {"shutdown",   true,  GET_MODULE_METHOD(ssl_shutdown)},
      {"set_ciphers",   true,  GET_MODULE_METHOD(ssl_set_ciphers)},
      {"get_peer_certificate",   true,  GET_MODULE_METHOD(ssl_get_peer_certificate)},
      {NULL,    false, NULL},
  };

  static z_module_reg module = {
      .name = "_ssl",
      .fields = module_fields,
      .functions = module_functions,
      .classes = NULL,
      .preloader = &__ssl_module_preloader,
      .unloader = NULL
  };

  return &module;
}

#undef SSL_BUF_SIZE
