def fib(n) {
    if n < 2 {
        return 1
    }

    return fib(n - 1) + fib(n - 2)
}

var start = microtime()
echo fib(45)

echo '\nTotal time taken: ${(microtime() - start)/1_000_000}s'