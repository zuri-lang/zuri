
echo 'Copying file me.png to me2.png'
var start = time()

var f2 = file('image.png', 'wb')
f2.write(bytes([1, 2, 3, 4, 5]))

var f = file('image.png', 'rb')
var data = f.read()

echo 'Successfully copied file me.png to me2.png in ${time() - start}s'
