#include <zuri.h>
#include <openssl/evp.h>
#include <openssl/provider.h>
#include "fnv.h"
#include "gost.h"

static OSSL_PROVIDER *default_provider = NULL, *legacy_provider = NULL;

void z__hash_module_preloader(z_vm* vm) {
  OpenSSL_add_all_digests();
  default_provider = OSSL_PROVIDER_load(NULL, "default");
  legacy_provider = OSSL_PROVIDER_load(NULL, "legacy");
}

void z__hash_module_unloader(z_vm* vm) {
  if (default_provider) OSSL_PROVIDER_unload(default_provider);
  if (legacy_provider) OSSL_PROVIDER_unload(legacy_provider);
  default_provider = legacy_provider = NULL;
}

static void FNV1(unsigned char *data, int length, unsigned char digest[4]) {
  FNV132_CTX ctx;
  FNV132Init(&ctx);
  FNV132Update(&ctx, data, length);
  FNV132Final(&ctx, digest);
}

static void FNV1a(unsigned char *data, int length, unsigned char digest[4]) {
  FNV132_CTX ctx;
  FNV132Init(&ctx);
  FNV1a32Update(&ctx, data, length);
  FNV132Final(&ctx, digest);
}

static void FNV164(unsigned char *data, int length, unsigned char digest[8]) {
  FNV164_CTX ctx;
  FNV164Init(&ctx);
  FNV164Update(&ctx, data, length);
  FNV164Final(&ctx, digest);
}

static void FNV1a64(unsigned char *data, int length, unsigned char digest[8]) {
  FNV164_CTX ctx;
  FNV164Init(&ctx);
  FNV1a64Update(&ctx, data, length);
  FNV164Final(&ctx, digest);
}

static void GOSTString(unsigned char *data, unsigned int data_len, unsigned char digest[32]) {
  GOST_CTX ctx;
  GOSTInit(&ctx);
  GOSTUpdate(&ctx, data, data_len);
  GOSTFinal(digest, &ctx);
}

DECLARE_MODULE_METHOD(hash__hash) {
  ENFORCE_ARG_COUNT(digest, 2);
  ENFORCE_ARG_TYPE(digest, 0, IS_STRING);
  ENFORCE_ARG_TYPES(digest, 1, IS_STRING, IS_BYTES);

  char* digest = AS_C_STRING(args[0]);

  const EVP_MD* md = EVP_get_digestbyname(digest);
  if (md == NULL) {
    RETURN_ERROR("Unknown message digest: %s", digest);
  }

  unsigned char md_value[EVP_MAX_MD_SIZE];
  unsigned int md_len;

  EVP_MD_CTX* ctx = EVP_MD_CTX_new();
  EVP_DigestInit_ex(ctx, md, NULL);

  if (IS_STRING(args[1])) {
    const z_obj_string* data = AS_STRING(args[1]);
    EVP_DigestUpdate(ctx, data->chars, data->length);
  } else {
    const z_obj_bytes* data = AS_BYTES(args[1]);
    EVP_DigestUpdate(ctx, data->bytes.bytes, data->bytes.count);
  }

  EVP_DigestFinal_ex(ctx, md_value, &md_len);
  EVP_MD_CTX_free(ctx);

  RETURN_OBJ(copy_bytes(vm, md_value, md_len));
}

DECLARE_MODULE_METHOD(hash__id) {
  ENFORCE_ARG_COUNT(hash, 1);
  METHOD_OVERRIDE(hash, 4);
  RETURN_NUMBER((double) hash_value(args[0]));
}

DECLARE_MODULE_METHOD(hash__fnv1) {
  ENFORCE_ARG_COUNT(fnv1, 1);
  ENFORCE_ARG_TYPES(fnv1, 0, IS_STRING, IS_BYTES);

  unsigned char result[4];
  if (IS_STRING(args[0])) {
    z_obj_string* string = AS_STRING(args[0]);
    FNV1((unsigned char*)string->chars, string->length, result);
  } else {
    z_obj_bytes* bytes = AS_BYTES(args[0]);
    FNV1(bytes->bytes.bytes, bytes->bytes.count, result);
  }

  RETURN_OBJ(copy_bytes(vm, result, 4));
}

DECLARE_MODULE_METHOD(hash__fnv1a) {
  ENFORCE_ARG_COUNT(fnv1a, 1);
  ENFORCE_ARG_TYPES(fnv1a, 0, IS_STRING, IS_BYTES);

  unsigned char result[4];
  if (IS_STRING(args[0])) {
    z_obj_string* string = AS_STRING(args[0]);
    FNV1a((unsigned char*)string->chars, string->length, result);
  } else {
    z_obj_bytes* bytes = AS_BYTES(args[0]);
    FNV1a(bytes->bytes.bytes, bytes->bytes.count, result);
  }

  RETURN_OBJ(copy_bytes(vm, result, 4));
}

DECLARE_MODULE_METHOD(hash__fnv1_64) {
  ENFORCE_ARG_COUNT(fnv1_64, 1);
  ENFORCE_ARG_TYPES(fnv1_64, 0, IS_STRING, IS_BYTES);

  unsigned char result[8];
  if (IS_STRING(args[0])) {
    z_obj_string* string = AS_STRING(args[0]);
    FNV164((unsigned char*)string->chars, string->length, result);
  } else {
    z_obj_bytes* bytes = AS_BYTES(args[0]);
    FNV164(bytes->bytes.bytes, bytes->bytes.count, result);
  }

  RETURN_OBJ(copy_bytes(vm, result, 8));
}

DECLARE_MODULE_METHOD(hash__fnv1a_64) {
  ENFORCE_ARG_COUNT(fnv1a64, 1);
  ENFORCE_ARG_TYPES(fnv1a64, 0, IS_STRING, IS_BYTES);

  unsigned char result[8];
  if (IS_STRING(args[0])) {
    z_obj_string* string = AS_STRING(args[0]);
    FNV1a64((unsigned char*)string->chars, string->length, result);
  } else {
    z_obj_bytes* bytes = AS_BYTES(args[0]);
    FNV1a64(bytes->bytes.bytes, bytes->bytes.count, result);
  }

  RETURN_OBJ(copy_bytes(vm, result, 8));
}

DECLARE_MODULE_METHOD(hash__gost) {
  ENFORCE_ARG_COUNT(gost, 1);
  ENFORCE_ARG_TYPES(gost, 0, IS_STRING, IS_BYTES);

  unsigned char result[32];
  if (IS_STRING(args[0])) {
    z_obj_string* string = AS_STRING(args[0]);
    GOSTString((unsigned char*)string->chars, string->length, result);
  } else {
    z_obj_bytes* bytes = AS_BYTES(args[0]);
    GOSTString(bytes->bytes.bytes, bytes->bytes.count, result);
  }

  RETURN_OBJ(copy_bytes(vm, result, 32));
}

CREATE_MODULE_LOADER(hash) {
  static z_func_reg module_functions[] = {
    {"hash", true, GET_MODULE_METHOD(hash__hash)},
    {"id", true, GET_MODULE_METHOD(hash__id)},
    {"fnv1", true, GET_MODULE_METHOD(hash__fnv1)},
    {"fnv1a", true, GET_MODULE_METHOD(hash__fnv1a)},
    {"fnv1_64", true, GET_MODULE_METHOD(hash__fnv1_64)},
    {"fnv1a_64", true, GET_MODULE_METHOD(hash__fnv1a_64)},
    {"gost", true, GET_MODULE_METHOD(hash__gost)},
    {NULL, false, NULL},
  };

  static z_module_reg module = {
    .name = "_hash",
    .fields = NULL,
    .functions = module_functions,
    .classes = NULL,
    .preloader = &z__hash_module_preloader,
    .unloader = &z__hash_module_unloader
  };

  return &module;
}
