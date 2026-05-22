#ifndef ZURI_UTIL_H
#define ZURI_UTIL_H

#include "common.h"

char *append_strings(char *old, char *new_str);
char *append_strings_n(char *old, char *new_str, size_t new_len);
char *read_file(const char *path);
size_t *parse_size(char *input, size_t *target);
char *format_size(size_t size);

#endif