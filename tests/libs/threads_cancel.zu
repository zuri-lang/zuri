import thread
import os

var th = thread(@{
    while true {}
})
th.start()

os.sleep(2)

th.cancel()

echo 'Done!'