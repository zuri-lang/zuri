import zlib

var d = zlib.compress('Hello, World')
echo d

var f = zlib.uncompress(d)
echo f.to_string()

var start = time()

for i in 1..101 {
  zlib.adler32('hello, world')
}

echo '1 hundred adler32 in ${time() - start}s'
start = time()

for i in 1..101 {
  zlib.crc32('hello, world')
}

echo '1 hundred crc32 in ${time() - start}s'
