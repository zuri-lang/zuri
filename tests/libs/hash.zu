import hash

var start = time()

for i in 1..101 {
  hash.md4('hello, world')
}

echo '1 hundred md4 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.md5('hello, world')
}

echo '1 hundred md5 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.sha1('hello, world')
}

echo '1 hundred sha1 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.sha224('hello, world')
}

echo '1 hundred sha224 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.sha256('hello, world')
}

echo '1 hundred sha256 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.sha384('hello, world')
}

echo '1 hundred sha384 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.sha512('hello, world')
}

echo '1 hundred sha512 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.fnv1('hello, world')
}

echo '1 hundred fnv1 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.fnv1a('hello, world')
}

echo '1 hundred fnv1a in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.fnv1a_64('hello, world')
}

echo '1 hundred fnv1a_64 in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.whirlpool('hello, world')
}

echo '1 hundred whirlpool in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.gost('hello, world')
}

echo '1 hundred gost in ${time() - start}s'
start = time()

for i in 1..101 {
  hash.hmac_sha256('0123456789ABCDEF', 'hello, world')
}

echo '1 hundred hmac_sha256 in ${time() - start}s'