def fannkuchredux(n)
{
    var perm = [0] * n
    var perm1 = [0] * n
    var count = [0] * n
    var maxFlipsCount = 0
    var permCount = 0
    var checksum = 0

    iter var i=0; i<n; i+=1 {
        perm1[i] = i
    }
    var r = n

    while 1 {
        while r != 1 {
            count[r-1] = r
            r -= 1
        }

        iter var i=0; i<n; i+=1 {
            perm[i] = perm1[i]
        }
        var flipsCount = 0
        var k

        while !((k = perm[0]) == 0) {
            var k2 = (k+1) >> 1
            iter var i=0; i<k2; i++ {
                var temp = perm[i]
                perm[i] = perm[k-i]
                perm[k-i] = temp
            }
            flipsCount += 1
        }

        maxFlipsCount = max(maxFlipsCount, flipsCount)
        checksum += permCount % 2 == 0 ? flipsCount : -flipsCount

        /* Use incremental change to generate another permutation */
        while 1 {
            if r == n {
                echo checksum
                return maxFlipsCount
            }

            var perm0 = perm1[0]
            var i = 0
            while i < r {
                var j = i + 1
                perm1[i] = perm1[j]
                i = j
            }
            perm1[r] = perm0
            count[r] = count[r] - 1
            if count[r] > 0 break
            r++
        }
        permCount++;
    }
}

const start = microtime()
echo 'Pfannkuchen(' +12+ ') = ' + fannkuchredux(12)

echo '\nTotal time taken: ${(microtime() - start)/1_000_000}s'
