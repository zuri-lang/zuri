#include "blob.h"
#include "memory.h"
#include "vm.h"

#include <stdlib.h>

void init_blob(z_blob *blob) {
  blob->count = 0;
  blob->capacity = 0;
  blob->code = NULL;
  blob->lines = NULL;
  init_value_arr(&blob->constants);
}

void write_blob(z_vm *vm, z_blob *blob, uint8_t byte, int line) {
  if (blob->capacity < blob->count + 1) {
    int old_capacity = blob->capacity;
    blob->capacity = GROW_CAPACITY(old_capacity);
    blob->code = GROW_ARRAY(uint8_t, blob->code, old_capacity, blob->capacity);
    blob->lines = GROW_ARRAY(int, blob->lines, old_capacity, blob->capacity);
  }

  blob->code[blob->count] = byte;
  blob->lines[blob->count] = line;
  blob->count++;
}

void free_blob(z_vm *vm, z_blob *blob) {
  if (blob->code != NULL) {
    FREE_ARRAY(uint8_t, blob->code, blob->capacity);
  }
  if (blob->lines != NULL) {
    FREE_ARRAY(int, blob->lines, blob->capacity);
  }
  free_value_arr(vm, &blob->constants);
  init_blob(blob);
}

int add_constant(z_vm *vm, z_blob *blob, z_value value) {
  push(vm, value); // fixing gc corruption
  write_value_arr(vm, &blob->constants, value);
  pop(vm); // fixing gc corruption
  return blob->constants.count - 1;
}
