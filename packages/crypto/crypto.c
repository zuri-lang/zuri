#include <zuri.h>
#include <stdlib.h>

#include <openssl/evp.h>
#include <openssl/rand.h>
#include <openssl/rsa.h>
#include <openssl/ec.h>
#include <openssl/kdf.h>
#include <openssl/ecdsa.h>
#include <openssl/pem.h>
#include <openssl/err.h>
#include <openssl/bn.h>
#include <openssl/core_names.h>
#include <openssl/param_build.h>

#include <argon2.h>

/* -------------------------------------------------------------------------
 * Internal helpers
 * ---------------------------------------------------------------------- */

/* Drain the OpenSSL error queue and return a static description string.
 * The returned pointer is valid until the next call to openssl_error(). */
static const char *openssl_error(void) {
    unsigned long e = ERR_get_error();
    if (e == 0) return "unknown OpenSSL error";
    static char buf[256];
    ERR_error_string_n(e, buf, sizeof(buf));
    ERR_clear_error();
    return buf;
}

/* Encode len bytes of src into a newly heap-allocated lowercase hex string.
 * Caller owns the returned buffer. */
static char *bytes_to_hex(const unsigned char *src, size_t len) {
    static const char hex[] = "0123456789abcdef";
    char *dst = malloc(len * 2 + 1);
    if (!dst) return NULL;
    for (size_t i = 0; i < len; i++) {
        dst[i * 2]     = hex[(src[i] >> 4) & 0xf];
        dst[i * 2 + 1] = hex[src[i] & 0xf];
    }
    dst[len * 2] = '\0';
    return dst;
}

/* Write an EVP_PKEY to a PEM BIO, return a heap-allocated string.
 * private_key selects PKCS#8 private or SubjectPublicKeyInfo public. */
static char *evp_key_to_pem(EVP_PKEY *pkey, int private_key) {
    BIO *bio = BIO_new(BIO_s_mem());
    if (!bio) return NULL;
    int ok = private_key
        ? PEM_write_bio_PrivateKey(bio, pkey, NULL, NULL, 0, NULL, NULL)
        : PEM_write_bio_PUBKEY(bio, pkey);
    if (!ok) { BIO_free(bio); return NULL; }
    BUF_MEM *bptr;
    BIO_get_mem_ptr(bio, &bptr);
    char *out = malloc(bptr->length + 1);
    if (out) {
        memcpy(out, bptr->data, bptr->length);
        out[bptr->length] = '\0';
    }
    BIO_free(bio);
    return out;
}

/* Read an EVP_PKEY from a PEM-encoded string.  Tries private key first,
 * falls back to the public key. */
static EVP_PKEY *pem_to_evp_key(const char *pem, size_t len) {
    BIO *bio = BIO_new_mem_buf(pem, (int)len);
    if (!bio) return NULL;
    EVP_PKEY *key = PEM_read_bio_PrivateKey(bio, NULL, NULL, NULL);
    if (!key) {
        BIO_reset(bio);
        key = PEM_read_bio_PUBKEY(bio, NULL, NULL, NULL);
    }
    BIO_free(bio);
    return key;
}


/* =========================================================================
 * Section 1 — Secure random bytes
 * ====================================================================== */

/*
 * crypto_random_bytes(n: number) -> bytes
 *
 * Returns n cryptographically secure random bytes using OpenSSL's CSPRNG.
 */
DECLARE_MODULE_METHOD(crypto_random_bytes) {
    ENFORCE_ARG_COUNT(random_bytes, 1);
    if (!IS_NUMBER(args[0])) RETURN_ERROR("random_bytes() expects a number");
    int n = (int)AS_NUMBER(args[0]);
    if (n <= 0 || n > 65536) RETURN_ERROR("random_bytes(): length must be 1..65536");
    unsigned char *buf = malloc(n);
    if (!buf) RETURN_ERROR("random_bytes(): out of memory");
    if (RAND_bytes(buf, n) != 1) {
        free(buf);
        RETURN_ERROR("random_bytes(): CSPRNG failure");
    }
    RETURN_BYTES(buf, n);
}


/* =========================================================================
 * Section 2 — AES-GCM (authenticated encryption)
 * ====================================================================== */

/*
 * crypto_aes_gcm_encrypt(key: bytes, iv: bytes, plaintext: bytes, aad: bytes?) -> bytes
 *
 * Encrypts plaintext with AES-GCM.  key must be 16, 24, or 32 bytes
 * (AES-128/192/256).  iv should be 12 bytes.  Returns the ciphertext
 * with the 16-byte authentication tag appended.
 *
 * The optional aad parameter supplies additional authenticated data that
 * is authenticated but not encrypted.
 */
DECLARE_MODULE_METHOD(crypto_aes_gcm_encrypt) {
    ENFORCE_ARG_RANGE(aes_gcm_encrypt, 3, 4);
    ENFORCE_ARG_TYPE(aes_gcm_encrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_gcm_encrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_gcm_encrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj  = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj   = AS_BYTES(args[1]);
    z_obj_bytes *pt_obj   = AS_BYTES(args[2]);

    const unsigned char *key = (const unsigned char *)key_obj->bytes.bytes;
    int                  kl  = key_obj->bytes.count;
    const unsigned char *iv  = (const unsigned char *)iv_obj->bytes.bytes;
    int                  ivl = iv_obj->bytes.count;
    const unsigned char *pt  = (const unsigned char *)pt_obj->bytes.bytes;
    int                  ptl = pt_obj->bytes.count;

    const EVP_CIPHER *cipher = NULL;
    switch (kl) {
        case 16: cipher = EVP_aes_128_gcm(); break;
        case 24: cipher = EVP_aes_192_gcm(); break;
        case 32: cipher = EVP_aes_256_gcm(); break;
        default: RETURN_ERROR("aes_gcm_encrypt(): key must be 16, 24, or 32 bytes");
    }

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    if (!ctx) RETURN_ERROR("aes_gcm_encrypt(): context allocation failed");

    int ok = EVP_EncryptInit_ex(ctx, cipher, NULL, NULL, NULL);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, ivl, NULL);
    ok = ok && EVP_EncryptInit_ex(ctx, NULL, NULL, key, iv);

    if (!ok) {
        EVP_CIPHER_CTX_free(ctx);
        RETURN_ERROR("aes_gcm_encrypt(): init failed");
    }

    /* Optional AAD */
    if (arg_count == 4 && IS_BYTES(args[3])) {
        z_obj_bytes *aad_obj = AS_BYTES(args[3]);
        int dummy;
        if (!EVP_EncryptUpdate(ctx, NULL, &dummy,
                (const unsigned char *)aad_obj->bytes.bytes,
                aad_obj->bytes.count)) {
            EVP_CIPHER_CTX_free(ctx);
            RETURN_ERROR("aes_gcm_encrypt(): AAD processing failed");
        }
    }

    unsigned char *out = malloc(ptl + 16);
    if (!out) { EVP_CIPHER_CTX_free(ctx); RETURN_ERROR("aes_gcm_encrypt(): out of memory"); }

    int out_len = 0, final_len = 0;
    ok  = EVP_EncryptUpdate(ctx, out, &out_len, pt, ptl);
    ok  = ok && EVP_EncryptFinal_ex(ctx, out + out_len, &final_len);
    ok  = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_GET_TAG, 16, out + out_len + final_len);
    EVP_CIPHER_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("aes_gcm_encrypt(): encryption failed"); }
    RETURN_BYTES(out, out_len + final_len + 16);
}

/*
 * crypto_aes_gcm_decrypt(key: bytes, iv: bytes, ciphertext: bytes, aad: bytes?) -> bytes
 *
 * Decrypts AES-GCM ciphertext (produced by aes_gcm_encrypt).  The last
 * 16 bytes of ciphertext are treated as the authentication tag.  Raises
 * an error if authentication fails.
 */
DECLARE_MODULE_METHOD(crypto_aes_gcm_decrypt) {
    ENFORCE_ARG_RANGE(aes_gcm_decrypt, 3, 4);
    ENFORCE_ARG_TYPE(aes_gcm_decrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_gcm_decrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_gcm_decrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj  = AS_BYTES(args[1]);
    z_obj_bytes *ct_obj  = AS_BYTES(args[2]);

    int kl  = key_obj->bytes.count;
    int ivl = iv_obj->bytes.count;
    int ctl = ct_obj->bytes.count;

    if (ctl < 16) RETURN_ERROR("aes_gcm_decrypt(): ciphertext too short (minimum 16 bytes for tag)");

    const unsigned char *key = (const unsigned char *)key_obj->bytes.bytes;
    const unsigned char *iv  = (const unsigned char *)iv_obj->bytes.bytes;
    const unsigned char *ct  = (const unsigned char *)ct_obj->bytes.bytes;
    int ptl = ctl - 16;

    const EVP_CIPHER *cipher = NULL;
    switch (kl) {
        case 16: cipher = EVP_aes_128_gcm(); break;
        case 24: cipher = EVP_aes_192_gcm(); break;
        case 32: cipher = EVP_aes_256_gcm(); break;
        default: RETURN_ERROR("aes_gcm_decrypt(): key must be 16, 24, or 32 bytes");
    }

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    if (!ctx) RETURN_ERROR("aes_gcm_decrypt(): context allocation failed");

    int ok = EVP_DecryptInit_ex(ctx, cipher, NULL, NULL, NULL);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_IVLEN, ivl, NULL);
    ok = ok && EVP_DecryptInit_ex(ctx, NULL, NULL, key, iv);

    if (!ok) {
        EVP_CIPHER_CTX_free(ctx);
        RETURN_ERROR("aes_gcm_decrypt(): init failed");
    }

    if (arg_count == 4 && IS_BYTES(args[3])) {
        z_obj_bytes *aad_obj = AS_BYTES(args[3]);
        int dummy;
        if (!EVP_DecryptUpdate(ctx, NULL, &dummy,
                (const unsigned char *)aad_obj->bytes.bytes,
                aad_obj->bytes.count)) {
            EVP_CIPHER_CTX_free(ctx);
            RETURN_ERROR("aes_gcm_decrypt(): AAD processing failed");
        }
    }

    unsigned char *out = malloc(ptl > 0 ? ptl : 1);
    if (!out) { EVP_CIPHER_CTX_free(ctx); RETURN_ERROR("aes_gcm_decrypt(): out of memory"); }

    int out_len = 0, final_len = 0;
    ok = EVP_DecryptUpdate(ctx, out, &out_len, ct, ptl);

    /* Set expected tag before finalising */
    unsigned char tag[16];
    memcpy(tag, ct + ptl, 16);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_GCM_SET_TAG, 16, tag);
    int auth_ok = ok ? EVP_DecryptFinal_ex(ctx, out + out_len, &final_len) : 0;
    EVP_CIPHER_CTX_free(ctx);

    if (!auth_ok) {
        free(out);
        RETURN_ERROR("aes_gcm_decrypt(): authentication tag mismatch — data is corrupt or tampered");
    }
    RETURN_BYTES(out, out_len + final_len);
}


/* =========================================================================
 * Section 3 — AES-CBC
 * ====================================================================== */

/*
 * crypto_aes_cbc_encrypt(key: bytes, iv: bytes, plaintext: bytes) -> bytes
 *
 * Encrypts plaintext with AES-CBC and PKCS#7 padding.  key must be 16,
 * 24, or 32 bytes; iv must be 16 bytes.
 */
DECLARE_MODULE_METHOD(crypto_aes_cbc_encrypt) {
    ENFORCE_ARG_COUNT(aes_cbc_encrypt, 3);
    ENFORCE_ARG_TYPE(aes_cbc_encrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_cbc_encrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_cbc_encrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj  = AS_BYTES(args[1]);
    z_obj_bytes *pt_obj  = AS_BYTES(args[2]);

    int kl  = key_obj->bytes.count;
    int ivl = iv_obj->bytes.count;
    int ptl = pt_obj->bytes.count;

    if (ivl != 16) RETURN_ERROR("aes_cbc_encrypt(): iv must be exactly 16 bytes");

    const EVP_CIPHER *cipher = NULL;
    switch (kl) {
        case 16: cipher = EVP_aes_128_cbc(); break;
        case 24: cipher = EVP_aes_192_cbc(); break;
        case 32: cipher = EVP_aes_256_cbc(); break;
        default: RETURN_ERROR("aes_cbc_encrypt(): key must be 16, 24, or 32 bytes");
    }

    int out_buf_len = ptl + 16;  /* PKCS#7 padding adds at most one block */
    unsigned char *out = malloc(out_buf_len);
    if (!out) RETURN_ERROR("aes_cbc_encrypt(): out of memory");

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    int ok = ctx
        && EVP_EncryptInit_ex(ctx, cipher, NULL,
               (const unsigned char *)key_obj->bytes.bytes,
               (const unsigned char *)iv_obj->bytes.bytes);

    int out_len = 0, final_len = 0;
    ok = ok && EVP_EncryptUpdate(ctx, out, &out_len,
                   (const unsigned char *)pt_obj->bytes.bytes, ptl);
    ok = ok && EVP_EncryptFinal_ex(ctx, out + out_len, &final_len);
    if (ctx) EVP_CIPHER_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("aes_cbc_encrypt(): encryption failed"); }
    RETURN_BYTES(out, out_len + final_len);
}

/*
 * crypto_aes_cbc_decrypt(key: bytes, iv: bytes, ciphertext: bytes) -> bytes
 *
 * Decrypts AES-CBC ciphertext and strips PKCS#7 padding.
 */
DECLARE_MODULE_METHOD(crypto_aes_cbc_decrypt) {
    ENFORCE_ARG_COUNT(aes_cbc_decrypt, 3);
    ENFORCE_ARG_TYPE(aes_cbc_decrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_cbc_decrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(aes_cbc_decrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj  = AS_BYTES(args[1]);
    z_obj_bytes *ct_obj  = AS_BYTES(args[2]);

    int kl  = key_obj->bytes.count;
    int ivl = iv_obj->bytes.count;
    int ctl = ct_obj->bytes.count;

    if (ivl != 16) RETURN_ERROR("aes_cbc_decrypt(): iv must be exactly 16 bytes");

    const EVP_CIPHER *cipher = NULL;
    switch (kl) {
        case 16: cipher = EVP_aes_128_cbc(); break;
        case 24: cipher = EVP_aes_192_cbc(); break;
        case 32: cipher = EVP_aes_256_cbc(); break;
        default: RETURN_ERROR("aes_cbc_decrypt(): key must be 16, 24, or 32 bytes");
    }

    unsigned char *out = malloc(ctl);
    if (!out) RETURN_ERROR("aes_cbc_decrypt(): out of memory");

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    int ok = ctx
        && EVP_DecryptInit_ex(ctx, cipher, NULL,
               (const unsigned char *)key_obj->bytes.bytes,
               (const unsigned char *)iv_obj->bytes.bytes);

    int out_len = 0, final_len = 0;
    ok = ok && EVP_DecryptUpdate(ctx, out, &out_len,
                   (const unsigned char *)ct_obj->bytes.bytes, ctl);
    ok = ok && EVP_DecryptFinal_ex(ctx, out + out_len, &final_len);
    if (ctx) EVP_CIPHER_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("aes_cbc_decrypt(): decryption failed — bad padding or wrong key"); }
    RETURN_BYTES(out, out_len + final_len);
}


/* =========================================================================
 * Section 4 — ChaCha20-Poly1305
 * ====================================================================== */

/*
 * crypto_chacha20_encrypt(key: bytes, iv: bytes, plaintext: bytes, aad: bytes?) -> bytes
 *
 * Encrypts with ChaCha20-Poly1305 (RFC 8439).  key must be 32 bytes;
 * iv must be 12 bytes.  Returns ciphertext + 16-byte Poly1305 tag.
 */
DECLARE_MODULE_METHOD(crypto_chacha20_encrypt) {
    ENFORCE_ARG_RANGE(chacha20_encrypt, 3, 4);
    ENFORCE_ARG_TYPE(chacha20_encrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(chacha20_encrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(chacha20_encrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj  = AS_BYTES(args[1]);
    z_obj_bytes *pt_obj  = AS_BYTES(args[2]);

    if (key_obj->bytes.count != 32) RETURN_ERROR("chacha20_encrypt(): key must be exactly 32 bytes");
    if (iv_obj->bytes.count  != 12) RETURN_ERROR("chacha20_encrypt(): iv must be exactly 12 bytes");

    int ptl = pt_obj->bytes.count;
    unsigned char *out = malloc(ptl + 16);
    if (!out) RETURN_ERROR("chacha20_encrypt(): out of memory");

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    int ok = ctx && EVP_EncryptInit_ex(ctx, EVP_chacha20_poly1305(), NULL, NULL, NULL);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, 12, NULL);
    ok = ok && EVP_EncryptInit_ex(ctx, NULL, NULL,
                    (const unsigned char *)key_obj->bytes.bytes,
                    (const unsigned char *)iv_obj->bytes.bytes);

    if (!ok) {
        if (ctx) EVP_CIPHER_CTX_free(ctx);
        free(out);
        RETURN_ERROR("chacha20_encrypt(): init failed");
    }

    if (arg_count == 4 && IS_BYTES(args[3])) {
        z_obj_bytes *aad_obj = AS_BYTES(args[3]);
        int dummy;
        EVP_EncryptUpdate(ctx, NULL, &dummy,
            (const unsigned char *)aad_obj->bytes.bytes, aad_obj->bytes.count);
    }

    int out_len = 0, final_len = 0;
    ok  = EVP_EncryptUpdate(ctx, out, &out_len,
              (const unsigned char *)pt_obj->bytes.bytes, ptl);
    ok  = ok && EVP_EncryptFinal_ex(ctx, out + out_len, &final_len);
    ok  = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_GET_TAG, 16, out + out_len + final_len);
    EVP_CIPHER_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("chacha20_encrypt(): encryption failed"); }
    RETURN_BYTES(out, out_len + final_len + 16);
}

/*
 * crypto_chacha20_decrypt(key: bytes, iv: bytes, ciphertext: bytes, aad: bytes?) -> bytes
 *
 * Decrypts ChaCha20-Poly1305 ciphertext.  Raises an error on tag
 * mismatch — the returned bytes are never accessible on failure.
 */
DECLARE_MODULE_METHOD(crypto_chacha20_decrypt) {
    ENFORCE_ARG_RANGE(chacha20_decrypt, 3, 4);
    ENFORCE_ARG_TYPE(chacha20_decrypt, 0, IS_BYTES);
    ENFORCE_ARG_TYPE(chacha20_decrypt, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(chacha20_decrypt, 2, IS_BYTES);

    z_obj_bytes *key_obj = AS_BYTES(args[0]);
    z_obj_bytes *iv_obj  = AS_BYTES(args[1]);
    z_obj_bytes *ct_obj  = AS_BYTES(args[2]);

    if (key_obj->bytes.count != 32) RETURN_ERROR("chacha20_decrypt(): key must be exactly 32 bytes");
    if (iv_obj->bytes.count  != 12) RETURN_ERROR("chacha20_decrypt(): iv must be exactly 12 bytes");

    int ctl = ct_obj->bytes.count;
    if (ctl < 16) RETURN_ERROR("chacha20_decrypt(): ciphertext too short");
    int ptl = ctl - 16;

    const unsigned char *ct = (const unsigned char *)ct_obj->bytes.bytes;

    unsigned char *out = malloc(ptl > 0 ? ptl : 1);
    if (!out) RETURN_ERROR("chacha20_decrypt(): out of memory");

    EVP_CIPHER_CTX *ctx = EVP_CIPHER_CTX_new();
    int ok = ctx && EVP_DecryptInit_ex(ctx, EVP_chacha20_poly1305(), NULL, NULL, NULL);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_IVLEN, 12, NULL);
    ok = ok && EVP_DecryptInit_ex(ctx, NULL, NULL,
                    (const unsigned char *)key_obj->bytes.bytes,
                    (const unsigned char *)iv_obj->bytes.bytes);

    if (!ok) {
        if (ctx) EVP_CIPHER_CTX_free(ctx);
        free(out);
        RETURN_ERROR("chacha20_decrypt(): init failed");
    }

    if (arg_count == 4 && IS_BYTES(args[3])) {
        z_obj_bytes *aad_obj = AS_BYTES(args[3]);
        int dummy;
        EVP_DecryptUpdate(ctx, NULL, &dummy,
            (const unsigned char *)aad_obj->bytes.bytes, aad_obj->bytes.count);
    }

    int out_len = 0, final_len = 0;
    ok = EVP_DecryptUpdate(ctx, out, &out_len, ct, ptl);

    unsigned char tag[16];
    memcpy(tag, ct + ptl, 16);
    ok = ok && EVP_CIPHER_CTX_ctrl(ctx, EVP_CTRL_AEAD_SET_TAG, 16, tag);
    int auth_ok = ok ? EVP_DecryptFinal_ex(ctx, out + out_len, &final_len) : 0;
    EVP_CIPHER_CTX_free(ctx);

    if (!auth_ok) {
        free(out);
        RETURN_ERROR("chacha20_decrypt(): authentication failed — data is corrupt or tampered");
    }
    RETURN_BYTES(out, out_len + final_len);
}


/* =========================================================================
 * Section 5 — RSA key generation, encryption (OAEP), signing (PSS)
 * ====================================================================== */

/*
 * crypto_rsa_generate(bits: number) -> dict{private_pem, public_pem}
 *
 * Generates an RSA key pair.  bits must be 2048 or 4096.
 * Returns a dictionary with PEM-encoded private_pem and public_pem strings.
 */
DECLARE_MODULE_METHOD(crypto_rsa_generate) {
    ENFORCE_ARG_COUNT(rsa_generate, 1);
    ENFORCE_ARG_TYPE(rsa_generate, 0, IS_NUMBER);

    int bits = (int)AS_NUMBER(args[0]);
    if (bits != 2048 && bits != 4096)
        RETURN_ERROR("rsa_generate(): bits must be 2048 or 4096");

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_RSA, NULL);
    if (!ctx) RETURN_ERROR("rsa_generate(): context failed");

    EVP_PKEY *pkey = NULL;
    int ok = EVP_PKEY_keygen_init(ctx) > 0
          && EVP_PKEY_CTX_set_rsa_keygen_bits(ctx, bits) > 0
          && EVP_PKEY_keygen(ctx, &pkey) > 0;
    EVP_PKEY_CTX_free(ctx);

    if (!ok || !pkey) {
        if (pkey) EVP_PKEY_free(pkey);
        RETURN_ERROR("rsa_generate(): key generation failed");
    }

    char *priv = evp_key_to_pem(pkey, 1);
    char *pub  = evp_key_to_pem(pkey, 0);
    EVP_PKEY_free(pkey);

    if (!priv || !pub) {
        free(priv); free(pub);
        RETURN_ERROR("rsa_generate(): PEM serialization failed");
    }

    z_obj_dict *result = (z_obj_dict *)GC(new_dict(vm));
    dict_add_entry(vm, result, GC_STRING("private_pem"), GC_STRING(priv));
    dict_add_entry(vm, result, GC_STRING("public_pem"),  GC_STRING(pub));
    free(priv); free(pub);
    RETURN_OBJ(result);
}

/*
 * crypto_rsa_encrypt(public_pem: string, plaintext: bytes) -> bytes
 *
 * Encrypts plaintext with RSA-OAEP (SHA-256 hash, MGF1-SHA-256 mask).
 */
DECLARE_MODULE_METHOD(crypto_rsa_encrypt) {
    ENFORCE_ARG_COUNT(rsa_encrypt, 2);
    ENFORCE_ARG_TYPE(rsa_encrypt, 0, IS_STRING);
    ENFORCE_ARG_TYPE(rsa_encrypt, 1, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *pt_obj  = AS_BYTES(args[1]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("rsa_encrypt(): failed to parse public key PEM");

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new(pkey, NULL);
    EVP_PKEY_free(pkey);
    if (!ctx) RETURN_ERROR("rsa_encrypt(): context failed");

    int ok = EVP_PKEY_encrypt_init(ctx) > 0
          && EVP_PKEY_CTX_set_rsa_padding(ctx, RSA_PKCS1_OAEP_PADDING) > 0
          && EVP_PKEY_CTX_set_rsa_oaep_md(ctx, EVP_sha256()) > 0
          && EVP_PKEY_CTX_set_rsa_mgf1_md(ctx, EVP_sha256()) > 0;

    if (!ok) { EVP_PKEY_CTX_free(ctx); RETURN_ERROR("rsa_encrypt(): padding configuration failed"); }

    size_t out_len = 0;
    const unsigned char *pt = (const unsigned char *)pt_obj->bytes.bytes;
    int ptl = pt_obj->bytes.count;

    ok = EVP_PKEY_encrypt(ctx, NULL, &out_len, pt, ptl) > 0;
    unsigned char *out = ok ? malloc(out_len) : NULL;
    ok = out && EVP_PKEY_encrypt(ctx, out, &out_len, pt, ptl) > 0;
    EVP_PKEY_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("rsa_encrypt(): encryption failed"); }
    RETURN_BYTES(out, (int)out_len);
}

/*
 * crypto_rsa_decrypt(private_pem: string, ciphertext: bytes) -> bytes
 *
 * Decrypts RSA-OAEP ciphertext.
 */
DECLARE_MODULE_METHOD(crypto_rsa_decrypt) {
    ENFORCE_ARG_COUNT(rsa_decrypt, 2);
    ENFORCE_ARG_TYPE(rsa_encrypt, 0, IS_STRING);
    ENFORCE_ARG_TYPE(rsa_decrypt, 1, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *ct_obj  = AS_BYTES(args[1]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("rsa_decrypt(): failed to parse private key PEM");

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new(pkey, NULL);
    EVP_PKEY_free(pkey);
    if (!ctx) RETURN_ERROR("rsa_decrypt(): context failed");

    int ok = EVP_PKEY_decrypt_init(ctx) > 0
          && EVP_PKEY_CTX_set_rsa_padding(ctx, RSA_PKCS1_OAEP_PADDING) > 0
          && EVP_PKEY_CTX_set_rsa_oaep_md(ctx, EVP_sha256()) > 0
          && EVP_PKEY_CTX_set_rsa_mgf1_md(ctx, EVP_sha256()) > 0;

    if (!ok) { EVP_PKEY_CTX_free(ctx); RETURN_ERROR("rsa_decrypt(): padding configuration failed"); }

    const unsigned char *ct = (const unsigned char *)ct_obj->bytes.bytes;
    int ctl = ct_obj->bytes.count;
    size_t out_len = 0;

    ok = EVP_PKEY_decrypt(ctx, NULL, &out_len, ct, ctl) > 0;
    unsigned char *out = ok ? malloc(out_len) : NULL;
    ok = out && EVP_PKEY_decrypt(ctx, out, &out_len, ct, ctl) > 0;
    EVP_PKEY_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("rsa_decrypt(): decryption failed"); }
    RETURN_BYTES(out, (int)out_len);
}

/*
 * crypto_rsa_sign(private_pem: string, message: bytes) -> bytes
 *
 * Signs message with RSA-PSS (SHA-256 digest, MGF1-SHA-256, salt len = digest).
 * Returns the DER-encoded signature.
 */
DECLARE_MODULE_METHOD(crypto_rsa_sign) {
    ENFORCE_ARG_COUNT(rsa_sign, 2);
    ENFORCE_ARG_TYPE(rsa_sign, 0, IS_STRING);
    ENFORCE_ARG_TYPE(rsa_sign, 1, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("rsa_sign(): failed to parse private key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    EVP_PKEY_CTX *pctx = NULL;

    int ok = mctx && EVP_DigestSignInit(mctx, &pctx, EVP_sha256(), NULL, pkey) > 0
          && EVP_PKEY_CTX_set_rsa_padding(pctx, RSA_PKCS1_PSS_PADDING) > 0
          && EVP_PKEY_CTX_set_rsa_pss_saltlen(pctx, RSA_PSS_SALTLEN_DIGEST) > 0
          && EVP_PKEY_CTX_set_rsa_mgf1_md(pctx, EVP_sha256()) > 0;

    EVP_PKEY_free(pkey);

    size_t sig_len = 0;
    ok = ok && EVP_DigestSign(mctx,
                    NULL, &sig_len,
                    (const unsigned char *)msg_obj->bytes.bytes,
                    msg_obj->bytes.count) > 0;

    unsigned char *sig = ok ? malloc(sig_len) : NULL;
    ok = sig && EVP_DigestSign(mctx, sig, &sig_len,
                    (const unsigned char *)msg_obj->bytes.bytes,
                    msg_obj->bytes.count) > 0;

    if (mctx) EVP_MD_CTX_free(mctx);
    if (!ok) { free(sig); RETURN_ERROR("rsa_sign(): signing failed"); }
    RETURN_BYTES(sig, (int)sig_len);
}

/*
 * crypto_rsa_verify(public_pem: string, message: bytes, signature: bytes) -> bool
 *
 * Verifies an RSA-PSS signature.  Returns true on success, false on
 * failure.  Never raises an error for a bad signature — only for invalid
 * arguments or a malformed key.
 */
DECLARE_MODULE_METHOD(crypto_rsa_verify) {
    ENFORCE_ARG_COUNT(rsa_verify, 3);
    ENFORCE_ARG_TYPE(rsa_verify, 0, IS_STRING);
    ENFORCE_ARG_TYPE(rsa_verify, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(rsa_verify, 2, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);
    z_obj_bytes  *sig_obj = AS_BYTES(args[2]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("rsa_verify(): failed to parse public key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    EVP_PKEY_CTX *pctx = NULL;

    int ok = mctx && EVP_DigestVerifyInit(mctx, &pctx, EVP_sha256(), NULL, pkey) > 0
          && EVP_PKEY_CTX_set_rsa_padding(pctx, RSA_PKCS1_PSS_PADDING) > 0
          && EVP_PKEY_CTX_set_rsa_pss_saltlen(pctx, RSA_PSS_SALTLEN_DIGEST) > 0
          && EVP_PKEY_CTX_set_rsa_mgf1_md(pctx, EVP_sha256()) > 0;

    EVP_PKEY_free(pkey);

    int verified = ok
        ? EVP_DigestVerify(mctx,
              (const unsigned char *)sig_obj->bytes.bytes, sig_obj->bytes.count,
              (const unsigned char *)msg_obj->bytes.bytes, msg_obj->bytes.count)
        : -1;

    if (mctx) EVP_MD_CTX_free(mctx);
    ERR_clear_error();
    RETURN_BOOL(verified == 1);
}


/* =========================================================================
 * Section 6 — ECDSA (P-256 / P-384)
 * ====================================================================== */

static EVP_PKEY *ecdsa_generate_key(int nid) {
    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_EC, NULL);
    if (!ctx) return NULL;
    EVP_PKEY *pkey = NULL;
    if (EVP_PKEY_keygen_init(ctx) <= 0
     || EVP_PKEY_CTX_set_ec_paramgen_curve_nid(ctx, nid) <= 0
     || EVP_PKEY_keygen(ctx, &pkey) <= 0) {
        EVP_PKEY_CTX_free(ctx);
        return NULL;
    }
    EVP_PKEY_CTX_free(ctx);
    return pkey;
}

/*
 * crypto_ecdsa_generate(curve: string) -> dict{private_pem, public_pem}
 *
 * Generates an ECDSA key pair.  curve must be "P-256" or "P-384".
 */
DECLARE_MODULE_METHOD(crypto_ecdsa_generate) {
    ENFORCE_ARG_COUNT(ecdsa_generate, 1);
    ENFORCE_ARG_TYPE(ecdsa_generate, 0, IS_STRING);

    z_obj_string *curve = AS_STRING(args[0]);

    int nid;
    if      (strcmp(curve->chars, "P-256") == 0) nid = NID_X9_62_prime256v1;
    else if (strcmp(curve->chars, "P-384") == 0) nid = NID_secp384r1;
    else RETURN_ERROR("ecdsa_generate(): curve must be \"P-256\" or \"P-384\"");

    EVP_PKEY *pkey = ecdsa_generate_key(nid);
    if (!pkey) RETURN_ERROR("ecdsa_generate(): key generation failed");

    char *priv = evp_key_to_pem(pkey, 1);
    char *pub  = evp_key_to_pem(pkey, 0);
    EVP_PKEY_free(pkey);

    if (!priv || !pub) { free(priv); free(pub); RETURN_ERROR("ecdsa_generate(): PEM export failed"); }

    z_obj_dict *result = (z_obj_dict *)GC(new_dict(vm));
    dict_add_entry(vm, result, GC_STRING("private_pem"), GC_STRING(priv));
    dict_add_entry(vm, result, GC_STRING("public_pem"),  GC_STRING(pub));
    free(priv); free(pub);
    RETURN_OBJ(result);
}

/*
 * crypto_ecdsa_sign(private_pem: string, message: bytes) -> bytes
 *
 * Signs message with ECDSA-SHA256.  Returns the DER-encoded signature.
 */
DECLARE_MODULE_METHOD(crypto_ecdsa_sign) {
    ENFORCE_ARG_COUNT(ecdsa_sign, 2);
    ENFORCE_ARG_TYPE(ecdsa_sign, 0, IS_STRING);
    ENFORCE_ARG_TYPE(ecdsa_sign, 1, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("ecdsa_sign(): failed to parse private key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    size_t sig_len = 0;
    int ok = mctx
        && EVP_DigestSignInit(mctx, NULL, EVP_sha256(), NULL, pkey) > 0
        && EVP_DigestSign(mctx, NULL, &sig_len,
               (const unsigned char *)msg_obj->bytes.bytes,
               msg_obj->bytes.count) > 0;

    unsigned char *sig = ok ? malloc(sig_len) : NULL;
    ok = sig && EVP_DigestSign(mctx, sig, &sig_len,
                    (const unsigned char *)msg_obj->bytes.bytes,
                    msg_obj->bytes.count) > 0;

    EVP_PKEY_free(pkey);
    if (mctx) EVP_MD_CTX_free(mctx);
    if (!ok) { free(sig); RETURN_ERROR("ecdsa_sign(): signing failed"); }
    RETURN_BYTES(sig, (int)sig_len);
}

/*
 * crypto_ecdsa_verify(public_pem: string, message: bytes, signature: bytes) -> bool
 *
 * Verifies an ECDSA-SHA256 DER signature.
 */
DECLARE_MODULE_METHOD(crypto_ecdsa_verify) {
    ENFORCE_ARG_COUNT(ecdsa_verify, 3);
    ENFORCE_ARG_TYPE(ecdsa_verify, 0, IS_STRING);
    ENFORCE_ARG_TYPE(ecdsa_verify, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(ecdsa_verify, 2, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);
    z_obj_bytes  *sig_obj = AS_BYTES(args[2]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("ecdsa_verify(): failed to parse public key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    int ok = mctx && EVP_DigestVerifyInit(mctx, NULL, EVP_sha256(), NULL, pkey) > 0;

    int verified = ok
        ? EVP_DigestVerify(mctx,
              (const unsigned char *)sig_obj->bytes.bytes, sig_obj->bytes.count,
              (const unsigned char *)msg_obj->bytes.bytes, msg_obj->bytes.count)
        : -1;

    EVP_PKEY_free(pkey);
    if (mctx) EVP_MD_CTX_free(mctx);
    ERR_clear_error();
    RETURN_BOOL(verified == 1);
}


/* =========================================================================
 * Section 7 — Ed25519 (EdDSA)
 * ====================================================================== */

/*
 * crypto_ed25519_generate() -> dict{private_pem, public_pem}
 *
 * Generates an Ed25519 key pair.
 */
DECLARE_MODULE_METHOD(crypto_ed25519_generate) {
    ENFORCE_ARG_COUNT(ed25519_generate, 0);

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_ED25519, NULL);
    if (!ctx) RETURN_ERROR("ed25519_generate(): context failed");

    EVP_PKEY *pkey = NULL;
    int ok = EVP_PKEY_keygen_init(ctx) > 0 && EVP_PKEY_keygen(ctx, &pkey) > 0;
    EVP_PKEY_CTX_free(ctx);

    if (!ok || !pkey) { if (pkey) EVP_PKEY_free(pkey); RETURN_ERROR("ed25519_generate(): keygen failed"); }

    char *priv = evp_key_to_pem(pkey, 1);
    char *pub  = evp_key_to_pem(pkey, 0);
    EVP_PKEY_free(pkey);

    if (!priv || !pub) { free(priv); free(pub); RETURN_ERROR("ed25519_generate(): PEM export failed"); }

    z_obj_dict *result = (z_obj_dict *)GC(new_dict(vm));
    dict_add_entry(vm, result, GC_STRING("private_pem"), GC_STRING(priv));
    dict_add_entry(vm, result, GC_STRING("public_pem"),  GC_STRING(pub));
    free(priv); free(pub);
    RETURN_OBJ(result);
}

/*
 * crypto_ed25519_sign(private_pem: string, message: bytes) -> bytes
 *
 * Signs message with Ed25519.  Returns the 64-byte raw signature.
 */
DECLARE_MODULE_METHOD(crypto_ed25519_sign) {
    ENFORCE_ARG_COUNT(ed25519_sign, 2);
    ENFORCE_ARG_TYPE(ed25519_sign, 0, IS_STRING);
    ENFORCE_ARG_TYPE(ed25519_sign, 1, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("ed25519_sign(): failed to parse private key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    size_t sig_len = 64;
    unsigned char *sig = malloc(sig_len);

    int ok = sig && mctx
        && EVP_DigestSignInit(mctx, NULL, NULL, NULL, pkey) > 0
        && EVP_DigestSign(mctx, sig, &sig_len,
               (const unsigned char *)msg_obj->bytes.bytes,
               msg_obj->bytes.count) > 0;

    EVP_PKEY_free(pkey);
    if (mctx) EVP_MD_CTX_free(mctx);
    if (!ok) { free(sig); RETURN_ERROR("ed25519_sign(): signing failed"); }
    RETURN_BYTES(sig, (int)sig_len);
}

/*
 * crypto_ed25519_verify(public_pem: string, message: bytes, signature: bytes) -> bool
 *
 * Verifies an Ed25519 signature.
 */
DECLARE_MODULE_METHOD(crypto_ed25519_verify) {
    ENFORCE_ARG_COUNT(ed25519_verify, 3);
    ENFORCE_ARG_TYPE(ed25519_verify, 0, IS_STRING);
    ENFORCE_ARG_TYPE(ed25519_verify, 1, IS_BYTES);
    ENFORCE_ARG_TYPE(ed25519_verify, 2, IS_BYTES);

    z_obj_string *pem_str = AS_STRING(args[0]);
    z_obj_bytes  *msg_obj = AS_BYTES(args[1]);
    z_obj_bytes  *sig_obj = AS_BYTES(args[2]);

    EVP_PKEY *pkey = pem_to_evp_key(pem_str->chars, pem_str->length);
    if (!pkey) RETURN_ERROR("ed25519_verify(): failed to parse public key PEM");

    EVP_MD_CTX *mctx = EVP_MD_CTX_new();
    int ok = mctx && EVP_DigestVerifyInit(mctx, NULL, NULL, NULL, pkey) > 0;

    int verified = ok
        ? EVP_DigestVerify(mctx,
              (const unsigned char *)sig_obj->bytes.bytes, sig_obj->bytes.count,
              (const unsigned char *)msg_obj->bytes.bytes, msg_obj->bytes.count)
        : -1;

    EVP_PKEY_free(pkey);
    if (mctx) EVP_MD_CTX_free(mctx);
    ERR_clear_error();
    RETURN_BOOL(verified == 1);
}


/* =========================================================================
 * Section 8 — X25519 (Diffie-Hellman key exchange)
 * ====================================================================== */

/*
 * crypto_x25519_generate() -> dict{private_pem, public_pem}
 *
 * Generates an X25519 key pair for use in Diffie-Hellman key exchange.
 */
DECLARE_MODULE_METHOD(crypto_x25519_generate) {
    ENFORCE_ARG_COUNT(x25519_generate, 0);

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_X25519, NULL);
    if (!ctx) RETURN_ERROR("x25519_generate(): context failed");

    EVP_PKEY *pkey = NULL;
    int ok = EVP_PKEY_keygen_init(ctx) > 0 && EVP_PKEY_keygen(ctx, &pkey) > 0;
    EVP_PKEY_CTX_free(ctx);

    if (!ok || !pkey) { if (pkey) EVP_PKEY_free(pkey); RETURN_ERROR("x25519_generate(): keygen failed"); }

    char *priv = evp_key_to_pem(pkey, 1);
    char *pub  = evp_key_to_pem(pkey, 0);
    EVP_PKEY_free(pkey);

    if (!priv || !pub) { free(priv); free(pub); RETURN_ERROR("x25519_generate(): PEM export failed"); }

    z_obj_dict *result = (z_obj_dict *)GC(new_dict(vm));
    dict_add_entry(vm, result, GC_STRING("private_pem"), GC_STRING(priv));
    dict_add_entry(vm, result, GC_STRING("public_pem"),  GC_STRING(pub));
    free(priv); free(pub);
    RETURN_OBJ(result);
}

/*
 * crypto_x25519_exchange(private_pem: string, peer_public_pem: string) -> bytes
 *
 * Performs X25519 Diffie-Hellman and returns the 32-byte shared secret.
 * Both parties must hash the shared secret before using it as a key.
 */
DECLARE_MODULE_METHOD(crypto_x25519_exchange) {
    ENFORCE_ARG_COUNT(x25519_exchange, 2);
    ENFORCE_ARG_TYPE(x25519_exchange, 0, IS_STRING);
    ENFORCE_ARG_TYPE(x25519_exchange, 1, IS_STRING);

    z_obj_string *priv_str = AS_STRING(args[0]);
    z_obj_string *pub_str  = AS_STRING(args[1]);

    EVP_PKEY *priv_key = pem_to_evp_key(priv_str->chars, priv_str->length);
    if (!priv_key) RETURN_ERROR("x25519_exchange(): failed to parse private key PEM");

    EVP_PKEY *pub_key = pem_to_evp_key(pub_str->chars, pub_str->length);
    if (!pub_key) { EVP_PKEY_free(priv_key); RETURN_ERROR("x25519_exchange(): failed to parse peer public key PEM"); }

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new(priv_key, NULL);
    EVP_PKEY_free(priv_key);

    size_t secret_len = 32;
    unsigned char *secret = malloc(secret_len);

    int ok = secret && ctx
        && EVP_PKEY_derive_init(ctx) > 0
        && EVP_PKEY_derive_set_peer(ctx, pub_key) > 0
        && EVP_PKEY_derive(ctx, secret, &secret_len) > 0;

    EVP_PKEY_free(pub_key);
    if (ctx) EVP_PKEY_CTX_free(ctx);

    if (!ok) { free(secret); RETURN_ERROR("x25519_exchange(): key exchange failed"); }
    RETURN_BYTES(secret, (int)secret_len);
}


/* =========================================================================
 * Section 9 — Argon2
 * ====================================================================== */

/*
 * crypto_argon2_hash(password: string, salt: bytes, options: dict?) -> string
 *
 * Derives a password hash using Argon2id.  Returns an encoded string
 * compatible with argon2_verify().
 *
 * options keys (all optional):
 *   t_cost  — number of iterations   (default 3)
 *   m_cost  — memory in KiB          (default 65536 = 64 MiB)
 *   threads — parallelism factor      (default 4)
 *   hash_len — raw hash length bytes  (default 32)
 */
DECLARE_MODULE_METHOD(crypto_argon2_hash) {
    ENFORCE_ARG_RANGE(argon2_hash, 2, 4);
    ENFORCE_ARG_TYPE(argon2_hash, 0, IS_STRING);
    ENFORCE_ARG_TYPE(argon2_hash, 1, IS_BYTES);

    z_obj_string *pwd_obj  = AS_STRING(args[0]);
    z_obj_bytes  *salt_obj = AS_BYTES(args[1]);

    if (salt_obj->bytes.count < 8)
        RETURN_ERROR("argon2_hash(): salt must be at least 8 bytes");

    uint32_t t_cost   = 3;
    uint32_t m_cost   = 65536;
    uint32_t threads  = 4;
    uint32_t hash_len = 32;

    if (arg_count >= 3 && IS_DICT(args[2])) {
        z_obj_dict *opts = AS_DICT(args[2]);
        z_value v;
        if (dict_get_entry(opts, GC_STRING("t_cost"),  &v) && IS_NUMBER(v)) t_cost   = (uint32_t)AS_NUMBER(v);
        if (dict_get_entry(opts, GC_STRING("m_cost"),  &v) && IS_NUMBER(v)) m_cost   = (uint32_t)AS_NUMBER(v);
        if (dict_get_entry(opts, GC_STRING("threads"), &v) && IS_NUMBER(v)) threads  = (uint32_t)AS_NUMBER(v);
        if (dict_get_entry(opts, GC_STRING("hash_len"),&v) && IS_NUMBER(v)) hash_len = (uint32_t)AS_NUMBER(v);
    }

    bool return_bytes = false;
    if(arg_count == 4) {
        ENFORCE_ARG_TYPE(argon2_hash, 3, IS_BOOL);
        return_bytes = AS_BOOL(args[3]);
    }

    if(return_bytes) {
        z_obj_bytes *bytes = (z_obj_bytes *)GC(new_bytes(vm, hash_len));

        int rc = argon2id_hash_raw(
            t_cost, m_cost, threads,
            pwd_obj->chars, pwd_obj->length,
            salt_obj->bytes.bytes, salt_obj->bytes.count,
            bytes->bytes.bytes, hash_len
        );

        if (rc != ARGON2_OK) {
            const char *msg = argon2_error_message(rc);
            RETURN_ERROR(msg ? msg : "argon2_hash(): unknown error");
        }

        RETURN_OBJ(bytes);
    } else {
        size_t encoded_len = argon2_encodedlen(t_cost, m_cost, threads,
                                salt_obj->bytes.count, hash_len, Argon2_id);
        char *encoded = malloc(encoded_len);
        if (!encoded) RETURN_ERROR("argon2_hash(): out of memory");

        int rc = argon2id_hash_encoded(
            t_cost, m_cost, threads,
            pwd_obj->chars, pwd_obj->length,
            salt_obj->bytes.bytes, salt_obj->bytes.count,
            hash_len, encoded, encoded_len
        );

        if (rc != ARGON2_OK) {
            const char *msg = argon2_error_message(rc);
            free(encoded);
            RETURN_ERROR(msg ? msg : "argon2_hash(): unknown error");
        }

        RETURN_T_STRING(encoded, encoded_len);
    }
}

/*
 * crypto_argon2_verify(encoded: string, password: string) -> bool
 *
 * Verifies a password against an Argon2id encoded hash string.
 * Returns true on match.
 */
DECLARE_MODULE_METHOD(crypto_argon2_verify) {
    ENFORCE_ARG_COUNT(argon2_verify, 2);
    ENFORCE_ARG_TYPE(argon2_verify, 0, IS_STRING);
    ENFORCE_ARG_TYPE(argon2_verify, 1, IS_STRING);

    z_obj_string *enc_obj = AS_STRING(args[0]);
    z_obj_string *pwd_obj = AS_STRING(args[1]);

    int rc = argon2id_verify(enc_obj->chars, pwd_obj->chars, pwd_obj->length);
    RETURN_BOOL(rc == ARGON2_OK);
}


/* =========================================================================
 * Section 10 — HKDF (key derivation from shared secrets)
 * ====================================================================== */

/*
 * crypto_hkdf(ikm: bytes, salt: bytes, info: bytes, length: number) -> bytes
 *
 * Derives length bytes of keying material from the input key material
 * using HKDF-SHA256 (RFC 5869).  Suitable for expanding X25519 shared
 * secrets into symmetric keys.
 */
DECLARE_MODULE_METHOD(crypto_hkdf) {
    ENFORCE_ARG_COUNT(hkdf, 4);
    if (!IS_BYTES(args[0]))   RETURN_ERROR("hkdf(): ikm must be bytes");
    if (!IS_BYTES(args[1]))   RETURN_ERROR("hkdf(): salt must be bytes");
    if (!IS_BYTES(args[2]))   RETURN_ERROR("hkdf(): info must be bytes");
    if (!IS_NUMBER(args[3]))  RETURN_ERROR("hkdf(): length must be a number");

    z_obj_bytes *ikm_obj  = AS_BYTES(args[0]);
    z_obj_bytes *salt_obj = AS_BYTES(args[1]);
    z_obj_bytes *info_obj = AS_BYTES(args[2]);
    int length = (int)AS_NUMBER(args[3]);

    if (length <= 0 || length > 8160) RETURN_ERROR("hkdf(): length must be 1..8160");

    unsigned char *out = malloc(length);
    if (!out) RETURN_ERROR("hkdf(): out of memory");

    EVP_PKEY_CTX *ctx = EVP_PKEY_CTX_new_id(EVP_PKEY_HKDF, NULL);
    int ok = ctx
        && EVP_PKEY_derive_init(ctx) > 0
        && EVP_PKEY_CTX_set_hkdf_md(ctx, EVP_sha256()) > 0
        && EVP_PKEY_CTX_set1_hkdf_salt(ctx,
               (const unsigned char *)salt_obj->bytes.bytes, salt_obj->bytes.count) > 0
        && EVP_PKEY_CTX_set1_hkdf_key(ctx,
               (const unsigned char *)ikm_obj->bytes.bytes, ikm_obj->bytes.count) > 0
        && EVP_PKEY_CTX_add1_hkdf_info(ctx,
               (const unsigned char *)info_obj->bytes.bytes, info_obj->bytes.count) > 0;

    size_t out_len = length;
    ok = ok && EVP_PKEY_derive(ctx, out, &out_len) > 0;
    if (ctx) EVP_PKEY_CTX_free(ctx);

    if (!ok) { free(out); RETURN_ERROR("hkdf(): derivation failed"); }
    RETURN_BYTES(out, length);
}


/* =========================================================================
 * Module registration
 * ====================================================================== */

CREATE_MODULE_LOADER(crypto) {
    static z_func_reg functions[] = {
        /* Random */
        {"random_bytes",      false, GET_MODULE_METHOD(crypto_random_bytes)},
        /* AES-GCM */
        {"aes_gcm_encrypt",   false, GET_MODULE_METHOD(crypto_aes_gcm_encrypt)},
        {"aes_gcm_decrypt",   false, GET_MODULE_METHOD(crypto_aes_gcm_decrypt)},
        /* AES-CBC */
        {"aes_cbc_encrypt",   false, GET_MODULE_METHOD(crypto_aes_cbc_encrypt)},
        {"aes_cbc_decrypt",   false, GET_MODULE_METHOD(crypto_aes_cbc_decrypt)},
        /* ChaCha20-Poly1305 */
        {"chacha20_encrypt",  false, GET_MODULE_METHOD(crypto_chacha20_encrypt)},
        {"chacha20_decrypt",  false, GET_MODULE_METHOD(crypto_chacha20_decrypt)},
        /* RSA */
        {"rsa_generate",      false, GET_MODULE_METHOD(crypto_rsa_generate)},
        {"rsa_encrypt",       false, GET_MODULE_METHOD(crypto_rsa_encrypt)},
        {"rsa_decrypt",       false, GET_MODULE_METHOD(crypto_rsa_decrypt)},
        {"rsa_sign",          false, GET_MODULE_METHOD(crypto_rsa_sign)},
        {"rsa_verify",        false, GET_MODULE_METHOD(crypto_rsa_verify)},
        /* ECDSA */
        {"ecdsa_generate",    false, GET_MODULE_METHOD(crypto_ecdsa_generate)},
        {"ecdsa_sign",        false, GET_MODULE_METHOD(crypto_ecdsa_sign)},
        {"ecdsa_verify",      false, GET_MODULE_METHOD(crypto_ecdsa_verify)},
        /* Ed25519 */
        {"ed25519_generate",  false, GET_MODULE_METHOD(crypto_ed25519_generate)},
        {"ed25519_sign",      false, GET_MODULE_METHOD(crypto_ed25519_sign)},
        {"ed25519_verify",    false, GET_MODULE_METHOD(crypto_ed25519_verify)},
        /* X25519 */
        {"x25519_generate",   false, GET_MODULE_METHOD(crypto_x25519_generate)},
        {"x25519_exchange",   false, GET_MODULE_METHOD(crypto_x25519_exchange)},
        /* Argon2 */
        {"argon2_hash",       false, GET_MODULE_METHOD(crypto_argon2_hash)},
        {"argon2_verify",     false, GET_MODULE_METHOD(crypto_argon2_verify)},
        /* HKDF */
        {"hkdf",              false, GET_MODULE_METHOD(crypto_hkdf)},
        {NULL, false, NULL},
    };

    static z_module_reg module = {
        .name      = "_crypto",
        .fields    = NULL,
        .functions = functions,
        .classes   = NULL,
        .preloader = NULL,
        .unloader  = NULL,
    };

    return &module;
}
