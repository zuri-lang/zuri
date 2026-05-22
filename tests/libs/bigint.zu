import bigint

var _hex = '1';
for i in 1..129 {
  var n = bigint(_hex, 16)
  assert n.toString(16).length() == i
  _hex = _hex + '0'
}

assert bigint(0).toString(2, 256).length() == 256

assert bigint(
  'ffb96ff654e61130ba8422f0debca77a0ea74ae5ea8bca9b54ab64aabf01003',
  16
).toString('hex', 2).length() == 64

assert bigint(-1).isNeg() == true
assert bigint(1).isNeg() == false
assert bigint(0).isNeg() == false
assert bigint('-0', 10).isNeg() == false

assert bigint(0).isOdd() == false
assert bigint(1).isOdd() == true
assert bigint(2).isOdd() == false
assert bigint('-0', 10).isOdd() == false
assert bigint('-1', 10).isOdd() == true
assert bigint('-2', 10).isOdd() == false

assert bigint(0).isEven() == true
assert bigint(1).isEven() == false
assert bigint(2).isEven() == true
assert bigint('-0', 10).isEven() == true
assert bigint('-1', 10).isEven() == false
assert bigint('-2', 10).isEven() == true

assert bigint(0).isZero() == true
assert bigint(1).isZero() == false
assert bigint(0xffffffff).isZero() == false


assert bigint(0x123456).toNumber() == 0x123456
assert bigint(0x3ffffff).toNumber() == 0x3ffffff
assert bigint(0x4000000).toNumber() == 0x4000000
assert bigint(0x10000000000000).toNumber() == 0x10000000000000
assert bigint(0x10040004004000).toNumber() == 0x10040004004000
assert bigint(-0x123456).toNumber() == -0x123456
assert bigint(-0x3ffffff).toNumber() == -0x3ffffff
assert bigint(-0x4000000).toNumber() == -0x4000000
assert bigint(-0x10000000000000).toNumber() == -0x10000000000000
assert bigint(-0x10040004004000).toNumber() == -0x10040004004000

assert bigint(
  '-578960446186580977117854925043439539266' +
  '34992332820282019728792003956564819968', 10
).toTwos(256).toString(16) ==
'8000000000000000000000000000000000000000000000000000000000000000'

@{
  var n = bigint(
    '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',
    16
  )
  assert (n * n).toString(16) ==
    '39e58a8055b6fb264b75ec8c646509784204ac15a8c24e05babc9729ab9' +
    'b055c3a9458e4ce3289560a38e08ba8175a9446ce14e608245ab3a9' +
    '978a8bd8acaa40'
  assert ((n * n) * n).toString(16) ==
    '1b888e01a06e974017a28a5b4da436169761c9730b7aeedf75fc60f687b' +
    '46e0cf2cb11667f795d5569482640fe5f628939467a01a612b02350' +
    '0d0161e9730279a7561043af6197798e41b7432458463e64fa81158' +
    '907322dc330562697d0d600'
}()


assert bigint('1222222225255589') / bigint('611111124969028') == bigint(1)

@{
  var b = bigint(
    '39e58a8055b6fb264b75ec8c646509784204ac15a8c24e05babc9729ab9' +
    'b055c3a9458e4ce3289560a38e08ba8175a9446ce14e608245ab3a9' +
    '978a8bd8acaa40',
    16
  )
  var n = bigint(
    '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',
    16
  )
  assert (b /n).toString(16) == n.toString(16)
}()

@{
  var p = bigint(
    'fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f',
    16
  )
  var a = bigint(
    '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798',
    16
  )

  assert ((a ** 2) / p) == bigint('39e58a8055b6fb264b75ec8c646509784204ac15a8c24e05babc9729e58090b9', 16)
}()


@{
  var g = bigint(100)
  g += bigint(50)

  assert g == bigint(150)
}()

@{
  var p = bigint(
    'ffffffff00000001000000000000000000000000ffffffffffffffffffffffff',
    16
  )
  var a = bigint(
    'fffffffe00000003fffffffd0000000200000001fffffffe00000002ffffffff' +
    'ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
    16
  )
  assert  (a % p).toNumber() == 0
}()