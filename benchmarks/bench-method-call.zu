

class Toggle {
    Toggle(start_state) {
        self._state = start_state
    }

    value() {
        return self._state
    }

    activate() {
        self._state = !self._state
        return self
    }
}

class NthToggle < Toggle {
    NthToggle(start_state, max_counter) {
        parent(start_state)
        self._count_max = max_counter
        self._count_max_count_max = 0
    }

    activate() {
        self._count_max += 1
        if self._count_max >= self._count_max {
            parent.activate()
            self._count_max = 0
        }
        return self
    }
}

var start = time()

var n = 100000
var val = true

var toggle = Toggle(val)

for i in 0..n {
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
    val = toggle.activate().value()
}

echo val

val = true
var ntoggle = NthToggle(val, 3)

for i in 0..n {
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
    val = ntoggle.activate().value()
}

echo val

print("elapsed: ${time() - start}")

