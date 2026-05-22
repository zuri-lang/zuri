#include "module.h"

#include <string.h>

static const char base64_encoding_table[] = {
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M',
    'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z',
    'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm',
    'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '+', '/',
};

static const unsigned char base64_decoding_table[256] = {
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x3e, 0x00, 0x00, 0x00, 0x3f,
    0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
    0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12,
    0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x24,
    0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x31, 0x32, 0x33, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00,
};

char *base64_encode(const unsigned char *data, int input_length,
                    int *output_length) {

  if (data == NULL || input_length < 0) return NULL;

  const int mod_table[] = {0, 2, 1};

  *output_length = 4 * ((input_length + 2) / 3);

  // allocate one extra byte for NUL terminator (for safety when treating as string)
  char *encoded_data = (char *) malloc(*output_length + 1);

  if (encoded_data == NULL)
    return NULL;

  for (int i = 0, j = 0; i < input_length;) {

    uint32_t octet_a = i < input_length ? (unsigned char) data[i++] : 0;
    uint32_t octet_b = i < input_length ? (unsigned char) data[i++] : 0;
    uint32_t octet_c = i < input_length ? (unsigned char) data[i++] : 0;

    uint32_t triple = (octet_a << 0x10) + (octet_b << 0x08) + octet_c;

    encoded_data[j++] = base64_encoding_table[(triple >> 3 * 6) & 0x3F];
    encoded_data[j++] = base64_encoding_table[(triple >> 2 * 6) & 0x3F];
    encoded_data[j++] = base64_encoding_table[(triple >> 1 * 6) & 0x3F];
    encoded_data[j++] = base64_encoding_table[(triple >> 0 * 6) & 0x3F];
  }

  for (int i = 0; i < mod_table[input_length % 3]; i++)
    encoded_data[*output_length - 1 - i] = '=';

  encoded_data[*output_length] = '\0';
  return encoded_data;
}

unsigned char *base64_decode(const char *data, int input_length,
                             int *output_length) {

  if (data == NULL || input_length <= 0 || (input_length % 4) != 0)
    return NULL;

  // Validate characters (basic): allow A-Z, a-z, 0-9, '+', '/', '='
  for (int i = 0; i < input_length; i++) {
    unsigned char c = (unsigned char)data[i];
    if ((c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '+' || c == '/' || c == '=') {
      continue;
    }
    return NULL; // invalid char
  }

  *output_length = input_length / 4 * 3;

  if (data[input_length - 1] == '=')
    (*output_length)--;
  if (data[input_length - 2] == '=')
    (*output_length)--;

  unsigned char *decoded_data = (unsigned char *) malloc(*output_length);

  if (decoded_data == NULL)
    return NULL;

  for (int i = 0, j = 0; i < input_length;) {

    uint32_t sextet_a =
        data[i] == '=' ? 0 & i++ : base64_decoding_table[(int) data[i++]];
    uint32_t sextet_b =
        data[i] == '=' ? 0 & i++ : base64_decoding_table[(int) data[i++]];
    uint32_t sextet_c =
        data[i] == '=' ? 0 & i++ : base64_decoding_table[(int) data[i++]];
    uint32_t sextet_d =
        data[i] == '=' ? 0 & i++ : base64_decoding_table[(int) data[i++]];

    uint32_t triple = (sextet_a << 3 * 6) + (sextet_b << 2 * 6) +
                      (sextet_c << 1 * 6) + (sextet_d << 0 * 6);

    if (j < *output_length)
      decoded_data[j++] = (triple >> 2 * 8) & 0xFF;
    if (j < *output_length)
      decoded_data[j++] = (triple >> 1 * 8) & 0xFF;
    if (j < *output_length)
      decoded_data[j++] = (triple >> 0 * 8) & 0xFF;
  }

  return decoded_data;
}

DECLARE_MODULE_METHOD(base64__decode) {
  ENFORCE_ARG_COUNT(decode, 1);
  ENFORCE_ARG_TYPE(decode, 0, IS_STRING);

  z_obj_string *string = AS_STRING(args[0]);

  int output_length;
  unsigned char *data = base64_decode((const char *) string->chars,
                                      string->length, &output_length);

  if (data == NULL) RETURN_NIL;

  z_obj_bytes *bytes = new_bytes(vm, output_length);
  memcpy(bytes->bytes.bytes, data, output_length);
  free(data);

  RETURN_OBJ(bytes);
}

DECLARE_MODULE_METHOD(base64__encode) {
  ENFORCE_ARG_COUNT(encode, 1);
  ENFORCE_ARG_TYPE(encode, 0, IS_BYTES);

  z_obj_bytes *bytes = AS_BYTES(args[0]);

  int output_length;
  char *data = base64_encode((const unsigned char *) bytes->bytes.bytes,
                             bytes->bytes.count, &output_length);

  if (data == NULL) RETURN_NIL;

  RETURN_T_STRING(data, output_length);
}

CREATE_MODULE_LOADER(base64) {
  static z_func_reg module_functions[] = {
      {"decode", false, GET_MODULE_METHOD(base64__decode)},
      {"encode", false, GET_MODULE_METHOD(base64__encode)},
      {NULL,     false, NULL},
  };

  static z_module_reg module = {
      .name = "_base64",
      .fields = NULL,
      .functions = module_functions,
      .classes = NULL,
      .preloader = NULL,
      .unloader = NULL
  };

  return &module;
}