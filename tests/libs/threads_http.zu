import thread

import http

echo '-------------- SINGLE THREAD ---------------------'

var start = microtime()
for i in 0..10 {
    echo http.get('https://example.com')
}
var normal_ends = microtime() - start


echo '--------------- MULTITHREADING -------------------'

start = microtime()

var thrds = []
for i in 0..10 {
    thrds.append(thread.start(@{
        echo http.get('https://example.com')
    }))
}

for t in thrds {
    t.await()
}

var thread_ends = microtime() - start

# --------------------- RESULT ---------------------
echo 'Normal = ${normal_ends / 1000000} seconds'
echo 'Thread = ${thread_ends / 1000000} seconds'
