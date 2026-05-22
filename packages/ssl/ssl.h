#ifndef ZURI_SSL_MODULE_H
#define ZURI_SSL_MODULE_H

#define DEFINE_SSL_CONSTANT(v) \
  DECLARE_MODULE_METHOD(ssl_const_##v) { \
    RETURN_NUMBER(v); \
  }

#define DEFINE_SSL_STR_CONSTANT(v) \
  DECLARE_MODULE_METHOD(ssl_const_##v) { \
    RETURN_STRING(v); \
  }

#define DEFINE_SSL_PTR_CONSTANT(v) \
  DECLARE_MODULE_METHOD(ssl_const_##v) { \
    RETURN_PTR(v()); \
  }

#define GET_SSL_CONSTANT(v) \
  {#v, true, GET_MODULE_METHOD(ssl_const_##v)}

#define RETURN_SSL_ERROR() \
  RETURN_ERROR("SSL Error: %s", ossl_err_as_string())

#include <openssl/bio.h>
#include <openssl/err.h>
#include <openssl/pem.h>
#include <openssl/ssl.h>
#include <openssl/x509.h>
#include <openssl/x509v3.h>
#include <openssl/bn.h>  // for BN_bn2dec
#include <openssl/asn1.h>
#include <openssl/objects.h>

#endif //ZURI_SSL_MODULE_H
