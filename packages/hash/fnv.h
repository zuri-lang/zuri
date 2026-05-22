#ifndef ZURI_MODULE_HASH_FNV_H
#define ZURI_MODULE_HASH_FNV_H

#include <stdint.h>

#define FNV1_32_INIT ((uint32_t)0x811c9dc5)
#define FNV1_32A_INIT FNV1_32_INIT

#define FNV_32_PRIME ((uint32_t)0x01000193)

#define FNV1_64_INIT ((uint64_t)0xcbf29ce484222325ULL)
#define FNV1A_64_INIT FNV1_64_INIT

#define FNV_64_PRIME ((uint64_t)0x100000001b3ULL)

typedef struct {
  uint32_t state;
} FNV132_CTX;

typedef struct {
  uint64_t state;
} FNV164_CTX;


/*
 * fnv_32_buf - perform a 32 bit Fowler/Noll/Vo hash on a buffer
 *
 * input:
 *  buf - start of buffer to hash
 *  len - length of buffer in octets
 *  fnv	- previous hash value or 0 if first call
 *  alternate - if > 0 use the alternate version
 *
 * returns:
 *  32 bit hash as a static hash type
 */
static uint32_t fnv_32_buf(uint32_t fnv, void *buf, size_t len, int alternate) {
  unsigned char *bp = (unsigned char *) buf;   /* start of buffer */
  unsigned char *be = bp + len;     /* beyond end of buffer */

  /*
   * FNV-1 hash each octet in the buffer
   */
  if (alternate == 0) {
    while (bp < be) {
      /* multiply by the 32 bit FNV magic prime mod 2^32 */
      fnv *= FNV_32_PRIME;

      /* xor the bottom with the current octet */
      fnv ^= (uint32_t) *bp++;
    }
  } else {
    while (bp < be) {
      /* xor the bottom with the current octet */
      fnv ^= (uint32_t) *bp++;

      /* multiply by the 32 bit FNV magic prime mod 2^32 */
      fnv *= FNV_32_PRIME;
    }
  }

  /* return our new hash value */
  return fnv;
}

/*
 * fnv_64_buf - perform a 64 bit Fowler/Noll/Vo hash on a buffer
 *
 * input:
 *  buf - start of buffer to hash
 *  len - length of buffer in octets
 *  fnv	- previous hash value or 0 if first call
 *  alternate - if > 0 use the alternate version
 *
 * returns:
 *  64 bit hash as a static hash type
 */
static uint64_t fnv_64_buf(uint64_t fnv, void *buf, size_t len, int alternate) {
  unsigned char *bp = (unsigned char *) buf;   /* start of buffer */
  unsigned char *be = bp + len;     /* beyond end of buffer */

  /*
   * FNV-1 hash each octet of the buffer
   */

  if (alternate == 0) {
    while (bp < be) {
      /* multiply by the 64 bit FNV magic prime mod 2^64 */
      fnv *= FNV_64_PRIME;

      /* xor the bottom with the current octet */
      fnv ^= (uint64_t) *bp++;
    }
  } else {
    while (bp < be) {
      /* xor the bottom with the current octet */
      fnv ^= (uint64_t) *bp++;

      /* multiply by the 64 bit FNV magic prime mod 2^64 */
      fnv *= FNV_64_PRIME;
    }
  }

  /* return our new hash value */
  return fnv;
}


// 32 bit
static void FNV132Init(FNV132_CTX *context) {
  context->state = FNV1_32_INIT;
}

static void FNV132Update(FNV132_CTX *context, const unsigned char *input, size_t inputLen) {
  context->state = fnv_32_buf(context->state, (void *) input, inputLen, 0);
}

static void FNV1a32Update(FNV132_CTX *context, const unsigned char *input, size_t inputLen) {
  context->state = fnv_32_buf(context->state, (void *) input, inputLen, 1);
}

static void FNV132Final(FNV132_CTX *context, unsigned char digest[4]) {
#ifdef IS_BIG_ENDIAN
  memcpy(digest, &context->state, 4);
#else
  unsigned char *c = (unsigned char *) &context->state;
  for (int i = 0; i < 4; i++) {
    digest[i] = c[3 - i];
  }
#endif
}


// 64 bit
static void FNV164Init(FNV164_CTX *context) {
  context->state = FNV1_64_INIT;
}

static void FNV164Update(FNV164_CTX *context, const unsigned char *input, size_t inputLen) {
  context->state = fnv_64_buf(context->state, (void *) input, inputLen, 0);
}

static void FNV1a64Update(FNV164_CTX *context, const unsigned char *input, size_t inputLen) {
  context->state = fnv_64_buf(context->state, (void *) input, inputLen, 1);
}

static void FNV164Final(FNV164_CTX *context, unsigned char digest[8]) {
#ifdef IS_BIG_ENDIAN
  memcpy(digest, &context->state, 8);
#else
  unsigned char *c = (unsigned char *) &context->state;
  for (int i = 0; i < 8; i++) {
    digest[i] = c[7 - i];
  }
#endif
}

#endif //ZURI_MODULE_HASH_FNV_H
