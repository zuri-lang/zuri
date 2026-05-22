import thread

var s = []
for i in 0..60000 {
    var th = thread.start(@(t, i){
        t.set_name('thread-${i}')
        echo '${t.get_name()}, ${t.get_id()}, ${i}'
    }, [i])
    if(th) {
        s.append(th)
    }
}

for i in 0..60000 {
    s[i].await()
}
