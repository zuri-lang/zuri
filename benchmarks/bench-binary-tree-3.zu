import thread

var start = time()

var N = 21

def make_tree(depth) {
  if depth <= 0 return [nil, nil]
  depth -= 1
  return [make_tree(depth), make_tree(depth)]
}

def check_tree(node) {
  var left = node[0], right = node[1]
  if !left return 1
  return 1 + check_tree(left) + check_tree(right)
}

var min_depth = 4
var max_depth = max(min_depth + 2, N)
var stretch_depth = max_depth + 1

echo 'stretch tree of depth ${stretch_depth}\t check: ${check_tree(make_tree(stretch_depth))}'

var long_lived_tree = make_tree(max_depth)

var iterations = 2 ** max_depth

var thrds = []
var lines = []
iter var depth = min_depth; depth < stretch_depth; depth += 2 {
  lines.append(nil)

  thrds.append(thread.start(@(_, lines, i, iterations, depth){
    var check = 0
    for i in 1..(iterations + 1) {
      check += check_tree(make_tree(depth))
    }

    lines[i] = '${iterations}\t trees of depth ${depth}\t check: ${check}'
  }, [lines, lines.length() - 1, iterations, depth]))

  iterations //= 4
}

iter var i = 0; i < thrds.length(); i++ {
  thrds[i].await()
  echo lines[i]
}

echo 'long lived tree of depth ${max_depth}\t check: ${check_tree(long_lived_tree)}'
echo 'Total time taken: ${time() - start}'
