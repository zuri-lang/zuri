#ifndef ZURI_DEBUG_H
#define ZURI_DEBUG_H

#include "blob.h"

void disassemble_blob(z_blob *blob, const char *name);

int disassemble_instruction(z_blob *blob, int offset);

#endif