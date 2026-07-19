import .functions

# importing it again should not re-execute the module
import .functions

import .fibonacci {
  generate_fibonacci
}

# None should be re-executed but generate_fibonacci should stay the same
import .fibonacci {
  generate_fibonacci
}

echo generate_fibonacci(25)

import .closure as cls
echo cls.outer

import .closure as cls_new

# cls_new.out should be the same as cls.outer since the closure module
# should not be re-executed
echo cls_new.outer