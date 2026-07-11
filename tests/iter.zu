iter var i = 0; i < 10; i++ echo i

iter var i = 0; i < 10; i++ {
  echo '${i} iteration'
}

var j = 20

iter ; j >= 1; j-- {
  echo 'J = ${j}'
}

iter var i = 1; 
  i <= 10; 
  i++ 
{
  echo 'hi = ${i}'
  iter var j = 10; j >= 1; j-- {
    if j > 4 and j < 7 {
      continue
    }
    echo 'jay = ${j}'

    iter var k = 1; k <= 10; k++ {
      echo 'kay = ${k}'
    }
  }
}

iter var x = 0; x < 10; x++ {
  if x == 3 break
  echo 'The new x = ${x}'
}

iter ; j++ <= 10; {
  echo j
}
