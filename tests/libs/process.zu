import os

# not yet supported on windows
if os.platform == 'windows' {
  os.exit(0)
}

import process { * }

var paged = PagedValue()

var pr = Process(@(p, s) {
  echo 'It works!'
  echo p.id()
  s.set({name: 'Richard', age: 3.142})
}, paged)

pr.on_complete(@(){
  echo paged.get()
})

pr.start()
echo 'It works fine!'
# pr.await()  # this can be used to wait for completion.
echo 'It works fine again!'