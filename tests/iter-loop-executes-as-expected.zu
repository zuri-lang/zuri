def fib(n) {
    if n < 2 {
        return n
    }
    var a = 0, b = 1
    iter var i = 2; i <= n; i = i + 1 {
        const f = a + b
        a = b
        b = f
    }
    return b
}
echo fib(6)