/* The Computer Language Benchmarks Game
 * https://salsa.debian.org/benchmarksgame-team/benchmarksgame/
 *
 * binary-trees Zuri #1 program
 *
 * contributed by Richard Ore
 * *reset*
*/

class TreeNode {
  TreeNode(left, right) {
    self.left = left
    self.right = right
  }

  count() {
    if self.left == nil return 1
    return 1 + self.left.count() + self.right.count()
  }
}

def tree_with(depth) {
  if depth > 0 {
    return TreeNode(tree_with(depth - 1), tree_with(depth - 1))
  }
  return TreeNode(nil, nil)
}

def main(n) {
  var min_depth = 4
  var max_depth = max(min_depth + 2, n)
  var stretch_depth = max_depth + 1

  echo 'stretch tree of depth ${stretch_depth}\t check: ${tree_with(stretch_depth).count()}'

  var long_lived_tree = tree_with(max_depth)

  iter var depth = min_depth; depth <= max_depth; depth += 2 {
    var sum = 0
    var iterations = 1 << (max_depth - depth + min_depth)

    iter var i = 1; i <= iterations; i++ {
      sum += tree_with(depth).count()
    }

    echo '${iterations}\t trees of depth ${depth}\t check: ${sum}'
  }

  echo 'long lived tree of depth ${max_depth}\t check: ${long_lived_tree.count()}'
}

var start = time()
main(21)
echo '\n\nTime taken: ${time() - start}'
