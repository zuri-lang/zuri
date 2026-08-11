# Notes on the new state of the Zuri programming language.

- Numbers and Booleans are now objects with methods. Numbers carry methods such as `log`, `sin`, `abs`, etc.
- All objects and classes now carry a default `to_string()` method that can be overridden by their implementations.
- Builtin functions such as `to_string`, `to_number`, etc have been made redundant new features and have been removed.
- The class constructor has been replaced by the `@new` decorated method instead of a method with the same name. 
- Classes are now partially-immutable. Once created, new fields and methods cannot be added at runtime. However, their static properties can continue to be mutated.
- Class operator override have now been replaced and no longer have their own distinct syntax. Now, they reuse the decorated methods such as `@add`, `@sub`, `@mul`, `@div`, `@lshift` etc.
- Imports are now local by default and you'll have to explicity specify that you intend to export them out of the importing module by prefixing the import path with the `@` symbol. E.g. `import @.module`, `import @.module { item }`, or `import @.module { * }` where module, item, and all items become exported by the current module respectively.
- The `zlib` module has been dropped in favor of the `compress` module.