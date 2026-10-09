// The landing page's behaviour: Zuri highlighting for the code samples,
// the tab sets, the copy buttons, and the terminal that plays a project
// from `zuri init` to a shipped executable. Everything here is an
// enhancement; without it the page shows every sample and every step.

(function () {
  'use strict';

  var reducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;

  // Highlighting

  var KEYWORDS = new Set([
    'and', 'as', 'break', 'catch', 'class', 'const', 'continue', 'def', 'do',
    'echo', 'else', 'false', 'for', 'if', 'import', 'in', 'iter', 'nil', 'or',
    'parent', 'raise', 'return', 'self', 'static', 'true', 'using', 'var',
    'when', 'while',
  ]);

  var TYPES = new Set([
    'bool', 'bytes', 'dict', 'file', 'function', 'list', 'number', 'range',
    'string',
  ]);

  function escape(text) {
    return text.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  function span(kind, text) {
    return '<span class="' + kind + '">' + escape(text) + '</span>';
  }

  // A string literal, with each `${...}` inside it highlighted as code.
  function stringLiteral(source, start) {
    var quote = source[start];
    var i = start + 1;
    var out = '';
    var run = quote;

    while (i < source.length && source[i] !== quote) {
      if (source[i] === '\\') {
        run += source.slice(i, i + 2);
        i += 2;
        continue;
      }

      if (source[i] === '$' && source[i + 1] === '{') {
        var depth = 1;
        var j = i + 2;

        while (j < source.length && depth > 0) {
          if (source[j] === '{') {
            depth++;
          } else if (source[j] === '}') {
            depth--;
          }

          j++;
        }

        out += span('zs', run) + span('zk', '${') + highlight(source.slice(i + 2, j - 1)) + span('zk', '}');
        run = '';
        i = j;
        continue;
      }

      run += source[i];
      i++;
    }

    run += source[i] || '';

    return { html: out + span('zs', run), end: i + 1 };
  }

  function highlight(source) {
    var out = '';
    var i = 0;

    while (i < source.length) {
      var c = source[i];
      var rest = source.slice(i);
      var m;

      if (c === '#') {
        var end = source.indexOf('\n', i);
        end = end < 0 ? source.length : end;
        out += span('zc', source.slice(i, end));
        i = end;
      } else if (c === "'" || c === '"') {
        var literal = stringLiteral(source, i);
        out += literal.html;
        i = literal.end;
      } else if ((m = /^@[A-Za-z_]\w*/.exec(rest))) {
        out += span('zd', m[0]);
        i += m[0].length;
      } else if (c === '@') {
        out += span('zk', c);
        i++;
      } else if ((m = /^(0x[0-9a-fA-F]+|\d+(\.\d+)?)/.exec(rest)) && !/\w/.test(source[i - 1] || '')) {
        out += span('zn', m[0]);
        i += m[0].length;
      } else if ((m = /^[A-Za-z_]\w*/.exec(rest))) {
        var word = m[0];
        var next = source.slice(i + word.length).match(/^\s*\(/);

        if (KEYWORDS.has(word)) {
          out += span('zk', word);
        } else if (next) {
          out += span(/^[A-Z]/.test(word) ? 'zt' : 'zf', word);
        } else if (/^[A-Z]/.test(word) || TYPES.has(word)) {
          out += span('zt', word);
        } else {
          out += escape(word);
        }

        i += word.length;
      } else {
        out += escape(c);
        i++;
      }
    }

    return out;
  }

  document.querySelectorAll('code.zu').forEach(function (block) {
    block.innerHTML = highlight(block.textContent);
  });

  // The top bar gains its rule once the page scrolls under it.

  var topbar = document.querySelector('.topbar');

  function markScrolled() {
    topbar.classList.toggle('scrolled', window.scrollY > 8);
  }

  if (topbar) {
    markScrolled();
    window.addEventListener('scroll', markScrolled, { passive: true });
  }

  // Tabs

  function select(tab) {
    var list = tab.closest('[role="tablist"]');
    var group = tab.closest('[data-tabs]');

    list.querySelectorAll('[role="tab"]').forEach(function (other) {
      var on = other === tab;
      var panel = document.getElementById(other.getAttribute('aria-controls'));

      other.setAttribute('aria-selected', on ? 'true' : 'false');
      other.tabIndex = on ? 0 : -1;

      if (panel) {
        panel.hidden = !on;
      }

      group.querySelectorAll('[data-for="' + other.getAttribute('aria-controls') + '"]').forEach(function (note) {
        note.hidden = !on;
      });
    });
  }

  document.querySelectorAll('[data-tabs] [role="tablist"]').forEach(function (list) {
    var tabs = Array.prototype.slice.call(list.querySelectorAll('[role="tab"]'));

    tabs.forEach(function (tab, index) {
      tab.addEventListener('click', function () {
        select(tab);
      });

      tab.addEventListener('keydown', function (event) {
        var step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[event.key];

        if (event.key === 'Home' || event.key === 'End') {
          step = 0;
        }

        if (step === undefined) {
          return;
        }

        event.preventDefault();

        var target = event.key === 'Home' ? tabs[0]
          : event.key === 'End' ? tabs[tabs.length - 1]
          : tabs[(index + step + tabs.length) % tabs.length];

        target.focus();
        select(target);
      });
    });
  });

  // Visitors on Windows see the PowerShell line first.
  if (/Windows/i.test(navigator.userAgent)) {
    document.querySelectorAll('.install [role="tab"][id$="-win-tab"]').forEach(select);
  }

  // Copy buttons

  function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text);
    }

    return new Promise(function (resolve, reject) {
      var area = document.createElement('textarea');

      area.value = text;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      document.body.appendChild(area);
      area.select();

      try {
        document.execCommand('copy') ? resolve() : reject(new Error('copy refused'));
      } catch (error) {
        reject(error);
      } finally {
        document.body.removeChild(area);
      }
    });
  }

  document.querySelectorAll('.copy').forEach(function (button) {
    button.addEventListener('click', function () {
      var code = button.parentElement.querySelector('code').cloneNode(true);

      code.querySelectorAll('.p').forEach(function (prompt) {
        prompt.remove();
      });

      copyText(code.textContent.trim()).then(function () {
        button.classList.add('copied');
        button.setAttribute('aria-label', 'Copied');

        setTimeout(function () {
          button.classList.remove('copied');
          button.setAttribute('aria-label', 'Copy the install command');
        }, 1600);
      });
    });
  });

  // The lifecycle terminal

  document.querySelectorAll('[data-player]').forEach(function (player) {
    var scenes = Array.prototype.slice.call(player.querySelectorAll('.scene'));
    var steps = Array.prototype.slice.call(player.querySelectorAll('.step'));
    var current = 0;
    var timer = null;
    var visible = false;
    var paused = false;

    scenes.forEach(function (scene) {
      var command = scene.querySelector('[data-type]');
      command.dataset.full = command.textContent;
    });

    player.classList.add('live');

    function show(index, typed) {
      clearTimeout(timer);
      current = index;

      player.querySelectorAll('.caret').forEach(function (caret) {
        caret.remove();
      });

      scenes.forEach(function (scene, i) {
        scene.classList.toggle('on', i === index);
      });

      steps.forEach(function (step, i) {
        step.setAttribute('aria-pressed', i === index ? 'true' : 'false');
      });

      var scene = scenes[index];
      var command = scene.querySelector('[data-type]');
      var output = scene.querySelector('[data-out]');
      var full = command.dataset.full;

      if (!typed) {
        command.textContent = full;
        output.style.visibility = 'visible';
        queueNext(scene);
        return;
      }

      var caret = document.createElement('span');
      var shown = 0;

      caret.className = 'caret';
      command.textContent = '';
      command.after(caret);
      output.style.visibility = 'hidden';

      (function type() {
        shown++;
        command.textContent = full.slice(0, shown);

        if (shown < full.length) {
          timer = setTimeout(type, 26 + Math.random() * 40);
          return;
        }

        timer = setTimeout(function () {
          caret.remove();
          output.style.visibility = 'visible';
          queueNext(scene);
        }, 380);
      })();
    }

    // Longer output stays up longer, so it can be read.
    function queueNext(scene) {
      if (reducedMotion || paused) {
        return;
      }

      var lines = scene.querySelector('[data-out]').textContent.split('\n').length;

      timer = setTimeout(function () {
        if (visible) {
          show((current + 1) % scenes.length, true);
        } else {
          paused = true;
        }
      }, 1600 + lines * 260);
    }

    steps.forEach(function (step, index) {
      step.addEventListener('click', function () {
        paused = false;
        show(index, false);
      });
    });

    if (reducedMotion || !('IntersectionObserver' in window)) {
      show(0, false);
      return;
    }

    new IntersectionObserver(function (entries) {
      visible = entries[0].isIntersecting;

      if (visible && (paused || timer === null)) {
        paused = false;
        show(timer === null ? 0 : current, timer === null);
      }
    }, { threshold: 0.35 }).observe(player);
  });
})();
