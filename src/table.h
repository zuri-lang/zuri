#ifndef ZURI_TABLE_H
#define ZURI_TABLE_H

#include "common.h"
#include "value.h"

typedef struct {
  z_value key;
  z_value value;
} z_entry;

typedef struct {
  int count;
  int capacity;
  z_entry *entries;
} z_table;

void init_table(z_table *table);

void free_table(z_vm *vm, z_table *table);

bool table_set(z_vm *vm, z_table *table, z_value key, z_value value);

bool table_get(z_table *table, z_value key, z_value *value);

bool table_delete(z_table *table, z_value key);

void table_add_all(z_vm *vm, z_table *from, z_table *to);
void table_copy_extensions(z_vm *vm, z_table *from, z_table *to);
void table_copy(z_vm *vm, z_table *from, z_table *to);

z_obj_string *table_find_string(z_table *table, const char *chars, int length,
                                uint32_t hash);

z_value table_find_key(z_table *table, z_value value);
z_obj_list *table_get_keys(z_vm *vm, z_table *table);

void table_print(z_table *table);

void mark_table(z_vm *vm, z_table *table);

void table_remove_whites(z_vm *vm, z_table *table);

void table_import_all(z_vm *vm, z_table *from, z_table *to);

#endif