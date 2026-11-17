/*
  The script every page of a zuri doc site shares.

  Everything a page needs to be read is already in its HTML, coloured
  code included; this adds what needs a script: the appearance menu,
  the contents as a drawer and its folding, the outline following the
  scroll, copying code, keys to turn the page, searching, and reloading
  while `zuri doc serve` is running.
*/
(function () {
  'use strict';

  var doc = document;
  var root = doc.documentElement;
  var body = doc.body;
  var base = body.getAttribute('data-root') || '';

  function store(key, value) {
    try {
      if (value === null) {
        localStorage.removeItem(key);
      } else {
        localStorage.setItem(key, value);
      }
    } catch (e) {}
  }

  function recall(key) {
    try {
      return localStorage.getItem(key);
    } catch (e) {
      return null;
    }
  }

  function typing(event) {
    var target = event.target;
    var name = target && target.tagName;

    return name === 'INPUT' || name === 'TEXTAREA' || name === 'SELECT' ||
      (target && target.isContentEditable);
  }

  var ICONS = {
    copy: '<svg viewBox="0 0 24 24" aria-hidden="true"><rect x="9" y="9" width="11" height="11" rx="2"/><path d="M5 15V6a2 2 0 0 1 2-2h9"/></svg>',
    check: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="m5 12.5 4.5 4.5L19 7"/></svg>',
    close: '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6 6 18"/></svg>'
  };

  /* Appearance */

  var dark = window.matchMedia('(prefers-color-scheme: dark)');

  function applyTheme(choice) {
    var isDark = choice === 'dark' || (choice === 'system' && dark.matches);

    root.setAttribute('data-theme', isDark ? 'dark' : 'light');
    root.setAttribute('data-theme-choice', choice);

    var items = doc.querySelectorAll('.theme-options [data-choice]');

    for (var i = 0; i < items.length; i++) {
      items[i].setAttribute('aria-checked', String(items[i].getAttribute('data-choice') === choice));
    }
  }

  function setupTheme() {
    var toggle = doc.querySelector('.theme-toggle');
    var menu = doc.querySelector('.theme-options');

    applyTheme(root.getAttribute('data-theme-choice') || 'system');

    var follow = function () {
      if (root.getAttribute('data-theme-choice') === 'system') {
        applyTheme('system');
      }
    };

    if (dark.addEventListener) {
      dark.addEventListener('change', follow);
    } else if (dark.addListener) {
      dark.addListener(follow);
    }

    if (!toggle || !menu) {
      return;
    }

    function close() {
      menu.hidden = true;
      toggle.setAttribute('aria-expanded', 'false');
    }

    toggle.addEventListener('click', function (event) {
      event.stopPropagation();
      menu.hidden = !menu.hidden;
      toggle.setAttribute('aria-expanded', String(!menu.hidden));

      if (!menu.hidden) {
        var chosen = menu.querySelector('[aria-checked="true"]') || menu.querySelector('button');

        chosen.focus();
      }
    });

    menu.addEventListener('click', function (event) {
      var item = event.target.closest('[data-choice]');

      if (!item) {
        return;
      }

      var choice = item.getAttribute('data-choice');

      store('zuri-doc-theme', choice);
      applyTheme(choice);
      close();
      toggle.focus();
    });

    doc.addEventListener('click', function (event) {
      if (!menu.hidden && !event.target.closest('.theme-menu')) {
        close();
      }
    });

    menu.addEventListener('keydown', function (event) {
      var items = Array.prototype.slice.call(menu.querySelectorAll('button'));
      var at = items.indexOf(doc.activeElement);

      if (event.key === 'Escape') {
        close();
        toggle.focus();
      } else if (event.key === 'ArrowDown') {
        event.preventDefault();
        items[(at + 1) % items.length].focus();
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        items[(at + items.length - 1) % items.length].focus();
      }
    });
  }

  /* The contents */

  var narrow = window.matchMedia('(max-width: 900px)');

  function drawerOpen() {
    return root.getAttribute('data-drawer') === 'open';
  }

  function setDrawer(open) {
    if (open) {
      root.setAttribute('data-drawer', 'open');
    } else {
      root.removeAttribute('data-drawer');
    }
  }

  function setupSidebar() {
    var sidebar = doc.getElementById('sidebar');
    var toggle = doc.querySelector('.sidebar-toggle');
    var scroller = doc.querySelector('.sidebar-scroll');

    if (!sidebar) {
      return;
    }

    if (toggle) {
      toggle.addEventListener('click', function () {
        if (narrow.matches) {
          setDrawer(!drawerOpen());
          return;
        }

        var hidden = root.getAttribute('data-sidebar') === 'hidden';

        if (hidden) {
          root.removeAttribute('data-sidebar');
          store('zuri-doc-sidebar', null);
        } else {
          root.setAttribute('data-sidebar', 'hidden');
          store('zuri-doc-sidebar', 'hidden');
        }
      });
    }

    var scrim = doc.querySelector('.scrim');

    if (scrim) {
      scrim.addEventListener('click', function () {
        setDrawer(false);
      });
    }

    sidebar.addEventListener('click', function (event) {
      var fold = event.target.closest('.toc-fold');

      if (fold) {
        var item = fold.closest('.toc-item');
        var open = item.classList.toggle('open');

        fold.setAttribute('aria-expanded', String(open));
        return;
      }

      if (event.target.closest('a') && narrow.matches) {
        setDrawer(false);
      }
    });

    // The contents keep their place from page to page; where the page
    // being read is out of view, it is brought into it.
    var current = sidebar.querySelector('[aria-current="page"]');
    var saved = null;

    try {
      saved = sessionStorage.getItem('zuri-doc-toc-scroll');
    } catch (e) {}

    if (saved !== null) {
      scroller.scrollTop = parseInt(saved, 10) || 0;
    }

    if (current) {
      var box = scroller.getBoundingClientRect();
      var spot = current.getBoundingClientRect();

      if (spot.top < box.top + 40 || spot.bottom > box.bottom - 40) {
        scroller.scrollTop += spot.top - box.top - box.height / 3;
      }
    }

    window.addEventListener('pagehide', function () {
      try {
        sessionStorage.setItem('zuri-doc-toc-scroll', String(scroller.scrollTop));
      } catch (e) {}
    });

    // Dragging the edge of the contents widens or narrows them.
    var grip = doc.querySelector('.sidebar-grip');

    if (!grip) {
      return;
    }

    grip.addEventListener('pointerdown', function (event) {
      event.preventDefault();
      grip.setPointerCapture(event.pointerId);
      body.classList.add('resizing');

      function move(moved) {
        var width = Math.max(220, Math.min(520, moved.clientX));

        root.style.setProperty('--sidebar-width', width + 'px');
      }

      function stop(stopped) {
        grip.releasePointerCapture(stopped.pointerId);
        grip.removeEventListener('pointermove', move);
        grip.removeEventListener('pointerup', stop);
        body.classList.remove('resizing');
        store('zuri-doc-sidebar-width', String(Math.round(sidebar.getBoundingClientRect().width)));
      }

      grip.addEventListener('pointermove', move);
      grip.addEventListener('pointerup', stop);
    });

    grip.addEventListener('dblclick', function () {
      root.style.removeProperty('--sidebar-width');
      store('zuri-doc-sidebar-width', null);
    });
  }

  /* On this page */

  function setupOutline() {
    var links = doc.querySelectorAll('.outline a');

    if (!links.length || !('IntersectionObserver' in window)) {
      return;
    }

    var byId = {};
    var headings = [];

    for (var i = 0; i < links.length; i++) {
      var id = decodeURIComponent(links[i].getAttribute('href').slice(1));
      var heading = doc.getElementById(id);

      if (heading) {
        byId[id] = links[i];
        headings.push(heading);
      }
    }

    var visible = {};

    function mark() {
      var chosen = null;

      for (var i = 0; i < headings.length; i++) {
        if (visible[headings[i].id]) {
          chosen = headings[i];
          break;
        }
      }

      if (!chosen) {
        // Between headings: the last one above the top of the window.
        for (var j = headings.length - 1; j >= 0; j--) {
          if (headings[j].getBoundingClientRect().top < 120) {
            chosen = headings[j];
            break;
          }
        }
      }

      for (var key in byId) {
        byId[key].classList.toggle('current', chosen !== null && key === chosen.id);
      }
    }

    var observer = new IntersectionObserver(function (entries) {
      entries.forEach(function (entry) {
        visible[entry.target.id] = entry.isIntersecting;
      });
      mark();
    }, { rootMargin: '-64px 0px -55% 0px' });

    headings.forEach(function (heading) {
      observer.observe(heading);
    });
  }

  /* Copying code */

  function copyText(text) {
    if (navigator.clipboard && window.isSecureContext) {
      return navigator.clipboard.writeText(text);
    }

    return new Promise(function (resolve, reject) {
      var area = doc.createElement('textarea');

      area.value = text;
      area.setAttribute('readonly', '');
      area.style.position = 'fixed';
      area.style.opacity = '0';
      body.appendChild(area);
      area.select();

      try {
        doc.execCommand('copy') ? resolve() : reject();
      } catch (e) {
        reject(e);
      }

      body.removeChild(area);
    });
  }

  // A terminal session copies its commands, not its prompts or output.
  function codeText(pre) {
    if (!pre.querySelector('.prompt')) {
      return pre.textContent;
    }

    var lines = [];
    var current = null;

    pre.querySelector('code').childNodes.forEach(function (node) {
      if (node.nodeType === 1 && node.classList.contains('prompt')) {
        current = [];
        lines.push(current);
        return;
      }

      var text = node.textContent;
      var parts = text.split('\n');

      for (var i = 0; i < parts.length; i++) {
        if (i > 0) {
          current = null;
        }

        if (current) {
          current.push(parts[i]);
        }
      }
    });

    return lines.map(function (line) {
      return line.join('');
    }).join('\n');
  }

  function setupCopy() {
    var blocks = doc.querySelectorAll('figure.code');

    for (var i = 0; i < blocks.length; i++) {
      var figure = blocks[i];

      if (figure.classList.contains('output')) {
        continue;
      }

      var button = doc.createElement('button');

      button.type = 'button';
      button.className = 'code-copy';
      button.setAttribute('aria-label', 'Copy');
      button.title = 'Copy';
      button.innerHTML = ICONS.copy;
      figure.appendChild(button);
    }

    doc.addEventListener('click', function (event) {
      var button = event.target.closest('.code-copy');

      if (!button) {
        return;
      }

      var pre = button.parentNode.querySelector('pre');

      copyText(codeText(pre).replace(/\n$/, '')).then(function () {
        button.classList.add('copied');
        button.innerHTML = ICONS.check;
        button.setAttribute('aria-label', 'Copied');

        setTimeout(function () {
          button.classList.remove('copied');
          button.innerHTML = ICONS.copy;
          button.setAttribute('aria-label', 'Copy');
        }, 1600);
      }, function () {});
    });
  }

  /* Search */

  var search = {
    dialog: null,
    input: null,
    results: null,
    status: null,
    data: null,
    index: null,
    vocabulary: null,
    hits: [],
    selected: -1,
    loading: false,
    timer: 0
  };

  function words(text) {
    return text.toLowerCase().match(/[\p{L}\p{N}_]+/gu) || [];
  }

  // The words of _text_ as the index holds them: each name whole, and
  // each part of a `snake_case` name as well, so `env` finds `get_env`.
  function indexWords(text) {
    var found = words(text);
    var count = found.length;

    for (var i = 0; i < count; i++) {
      if (found[i].indexOf('_') > 0) {
        found[i].split('_').forEach(function (part) {
          if (part.length > 1) {
            found.push(part);
          }
        });
      }
    }

    return found;
  }

  function escapeHtml(text) {
    return text.replace(/[&<>"]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }

  // Builds the word index once the sections have arrived: each word to
  // the sections holding it, weighted by where in them it appears.
  function buildIndex() {
    var data = window.zuriDocIndex;
    var index = new Map();

    function add(word, section, weight) {
      var entry = index.get(word);

      if (!entry) {
        entry = new Map();
        index.set(word, entry);
      }

      entry.set(section, (entry.get(section) || 0) + weight);
    }

    data.sections.forEach(function (section, s) {
      var page = data.pages[section[0]];
      var seen = {};

      indexWords(section[2]).forEach(function (word) {
        add(word, s, 10);
      });

      if (section[1] === '') {
        indexWords(page[1]).forEach(function (word) {
          add(word, s, 6);
        });
      }

      indexWords(section[3]).forEach(function (word) {
        seen[word] = (seen[word] || 0) + 1;
      });

      for (var word in seen) {
        add(word, s, Math.min(1 + Math.log(seen[word]), 4));
      }
    });

    search.data = data;
    search.index = index;
    search.vocabulary = Array.from(index.keys()).sort();
  }

  // Every indexed word starting with _prefix_.
  function startingWith(prefix) {
    var list = search.vocabulary;
    var low = 0;
    var high = list.length;

    while (low < high) {
      var mid = (low + high) >> 1;

      if (list[mid] < prefix) {
        low = mid + 1;
      } else {
        high = mid;
      }
    }

    var found = [];

    for (var i = low; i < list.length && found.length < 80; i++) {
      if (list[i].lastIndexOf(prefix, 0) !== 0) {
        break;
      }

      found.push(list[i]);
    }

    return found;
  }

  function runQuery(query) {
    var terms = words(query);

    if (!terms.length) {
      return [];
    }

    var total = search.data.sections.length;
    var scores = null;

    for (var t = 0; t < terms.length; t++) {
      var term = terms[t];
      var matches = new Map();
      var candidates = startingWith(term);

      for (var c = 0; c < candidates.length; c++) {
        var word = candidates[c];
        var postings = search.index.get(word);
        var exact = word === term ? 1 : 0.55 * term.length / word.length;
        var rarity = Math.log(1 + total / postings.size);

        postings.forEach(function (weight, section) {
          var score = weight * rarity * exact;

          if (score > (matches.get(section) || 0)) {
            matches.set(section, score);
          }
        });
      }

      if (scores === null) {
        scores = matches;
      } else {
        var both = new Map();

        scores.forEach(function (score, section) {
          if (matches.has(section)) {
            both.set(section, score + matches.get(section));
          }
        });

        scores = both;
      }

      if (!scores.size) {
        return [];
      }
    }

    var phrase = query.trim().toLowerCase();
    var ranked = [];

    scores.forEach(function (score, s) {
      var section = search.data.sections[s];

      if (terms.length > 1) {
        if (section[2].toLowerCase().indexOf(phrase) >= 0) {
          score *= 2.5;
        } else if (section[3].toLowerCase().indexOf(phrase) >= 0) {
          score *= 1.6;
        }
      }

      ranked.push([s, score]);
    });

    ranked.sort(function (a, b) {
      return b[1] - a[1];
    });

    return ranked.slice(0, 40).map(function (pair) {
      return pair[0];
    });
  }

  function markTerms(text, terms) {
    var html = escapeHtml(text);

    if (!terms.length) {
      return html;
    }

    var pattern = new RegExp('(' + terms.map(function (term) {
      return term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }).join('|') + ')', 'giu');

    return html.replace(pattern, '<mark>$1</mark>');
  }

  // A stretch of the section's text around the first term found in it.
  function teaser(text, terms) {
    var lower = text.toLowerCase();
    var at = -1;

    for (var i = 0; i < terms.length && at < 0; i++) {
      at = lower.indexOf(terms[i]);
    }

    var start = Math.max(0, at < 0 ? 0 : at - 60);
    var end = Math.min(text.length, start + 190);
    var piece = text.slice(start, end);

    if (start > 0) {
      piece = '… ' + piece.replace(/^\S*\s/, '');
    }

    if (end < text.length) {
      piece = piece.replace(/\s\S*$/, '') + ' …';
    }

    return markTerms(piece, terms);
  }

  function select(at) {
    var items = search.results.children;

    if (!items.length) {
      search.selected = -1;
      return;
    }

    search.selected = (at + items.length) % items.length;

    for (var i = 0; i < items.length; i++) {
      items[i].setAttribute('aria-selected', String(i === search.selected));
    }

    items[search.selected].scrollIntoView({ block: 'nearest' });
  }

  function show(query) {
    var terms = words(query);

    search.results.innerHTML = '';
    search.status.textContent = '';
    search.selected = -1;

    if (!terms.length) {
      return;
    }

    var found = runQuery(query);

    if (!found.length) {
      search.status.textContent = 'Nothing matches “' + query.trim() + '”.';
      return;
    }

    var highlight = encodeURIComponent(terms.join(' '));
    var html = '';

    found.forEach(function (s, n) {
      var section = search.data.sections[s];
      var page = search.data.pages[section[0]];
      var url = base + page[0] + '?highlight=' + highlight + (section[1] ? '#' + section[1] : '');
      var where = page[2] ? page[2] : page[1];

      html += '<li role="option" id="search-hit-' + n + '" aria-selected="false">' +
        '<a href="' + escapeHtml(url) + '">' +
        '<span class="search-where">' + escapeHtml(where) + '</span>' +
        '<span class="search-title">' + markTerms(section[2], terms) + '</span>' +
        '<span class="search-teaser">' + teaser(section[3], terms) + '</span>' +
        '</a></li>';
    });

    search.results.innerHTML = html;
    select(0);
  }

  function loadIndex(then) {
    if (search.index) {
      then();
      return;
    }

    if (search.loading) {
      return;
    }

    search.loading = true;
    search.status.textContent = 'Loading the index…';

    var script = doc.createElement('script');

    script.src = base + 'search-index.js';
    script.onload = function () {
      buildIndex();
      search.status.textContent = '';
      then();
    };
    script.onerror = function () {
      search.loading = false;
      search.status.textContent = 'The search index could not be loaded.';
    };
    doc.head.appendChild(script);
  }

  function openSearch() {
    if (!search.dialog) {
      return;
    }

    search.dialog.hidden = false;
    body.style.overflow = 'hidden';
    search.input.focus();
    search.input.select();
    loadIndex(function () {
      show(search.input.value);
    });
  }

  function closeSearch() {
    if (!search.dialog || search.dialog.hidden) {
      return;
    }

    search.dialog.hidden = true;
    body.style.overflow = '';
  }

  function setupSearch() {
    search.dialog = doc.querySelector('.search');

    if (!search.dialog) {
      return;
    }

    search.input = search.dialog.querySelector('input');
    search.results = search.dialog.querySelector('.search-results');
    search.status = search.dialog.querySelector('.search-status');

    var opener = doc.querySelector('.search-open');

    if (opener) {
      opener.addEventListener('click', openSearch);
    }

    search.dialog.addEventListener('click', function (event) {
      if (event.target === search.dialog) {
        closeSearch();
      }
    });

    search.input.addEventListener('input', function () {
      clearTimeout(search.timer);
      search.timer = setTimeout(function () {
        if (search.index) {
          show(search.input.value);
        }
      }, 60);
    });

    search.input.addEventListener('keydown', function (event) {
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        select(search.selected + 1);
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        select(search.selected - 1);
      } else if (event.key === 'Enter') {
        var item = search.results.children[search.selected];

        if (item) {
          event.preventDefault();
          window.location.href = item.querySelector('a').href;
          closeSearch();
        }
      }
    });
  }

  // Marks the words a search found on the page it led to.
  function markHits() {
    var wanted = new URLSearchParams(window.location.search).get('highlight');
    var content = doc.querySelector('.content');

    if (!wanted || !content) {
      return;
    }

    var terms = words(wanted).filter(function (term) {
      return term.length > 1;
    });

    if (!terms.length) {
      return;
    }

    var pattern = new RegExp(terms.map(function (term) {
      return term.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }).join('|'), 'giu');
    var walker = doc.createTreeWalker(content, NodeFilter.SHOW_TEXT);
    var nodes = [];

    while (walker.nextNode()) {
      nodes.push(walker.currentNode);
    }

    var marks = [];

    nodes.forEach(function (node) {
      var text = node.nodeValue;

      pattern.lastIndex = 0;

      if (!pattern.test(text)) {
        return;
      }

      pattern.lastIndex = 0;

      var piece = doc.createDocumentFragment();
      var at = 0;
      var found;

      while ((found = pattern.exec(text)) !== null) {
        piece.appendChild(doc.createTextNode(text.slice(at, found.index)));

        var mark = doc.createElement('mark');

        mark.className = 'search-hit';
        mark.textContent = found[0];
        piece.appendChild(mark);
        marks.push(mark);
        at = found.index + found[0].length;
      }

      piece.appendChild(doc.createTextNode(text.slice(at)));
      node.parentNode.replaceChild(piece, node);
    });

    if (!marks.length) {
      return;
    }

    var bar = doc.createElement('div');

    bar.className = 'hit-bar';
    bar.innerHTML = '<span>Showing “' + escapeHtml(terms.join(' ')) + '”</span>' +
      '<button type="button" aria-label="Clear the highlighting">' + ICONS.close + '</button>';
    body.appendChild(bar);

    function clear() {
      marks.forEach(function (mark) {
        mark.replaceWith(doc.createTextNode(mark.textContent));
      });
      content.normalize();
      bar.remove();

      var url = new URL(window.location.href);

      url.searchParams.delete('highlight');
      history.replaceState(null, '', url.toString());
    }

    bar.querySelector('button').addEventListener('click', clear);

    if (!window.location.hash) {
      marks[0].scrollIntoView({ block: 'center' });
    }
  }

  /* Keys */

  function setupKeys() {
    doc.addEventListener('keydown', function (event) {
      if (event.key === 'Escape') {
        if (search.dialog && !search.dialog.hidden) {
          closeSearch();
          return;
        }

        if (drawerOpen()) {
          setDrawer(false);
          return;
        }
      }

      if ((event.key === 'k' || event.key === 'K') && (event.metaKey || event.ctrlKey)) {
        event.preventDefault();
        openSearch();
        return;
      }

      if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey || typing(event)) {
        return;
      }

      if (search.dialog && !search.dialog.hidden) {
        return;
      }

      if (event.key === '/' || event.key === 's') {
        event.preventDefault();
        openSearch();
      } else if (event.key === 'ArrowLeft') {
        var previous = doc.querySelector('a[rel="prev"]');

        if (previous) {
          window.location.href = previous.href;
        }
      } else if (event.key === 'ArrowRight') {
        var next = doc.querySelector('a[rel="next"]');

        if (next) {
          window.location.href = next.href;
        }
      }
    });
  }

  /* While `zuri doc serve` is running */

  function setupLive() {
    if (!body.hasAttribute('data-live')) {
      return;
    }

    var known = null;

    function check() {
      fetch(base + '__zuri_doc/version', { cache: 'no-store' })
        .then(function (response) {
          return response.text();
        })
        .then(function (version) {
          if (known === null) {
            known = version;
          } else if (version !== known) {
            window.location.reload();
            return;
          }

          setTimeout(check, 700);
        }, function () {
          // The server stopped; look again less often until it is back.
          setTimeout(check, 2500);
        });
    }

    check();
  }

  /* Printing */

  function setupPrint() {
    if (!body.hasAttribute('data-print') || !/[?&]print\b/.test(window.location.search)) {
      return;
    }

    var ready = doc.fonts && doc.fonts.ready ? doc.fonts.ready : Promise.resolve();

    ready.then(function () {
      setTimeout(function () {
        window.print();
      }, 150);
    });
  }

  setupTheme();
  setupSidebar();
  setupOutline();
  setupCopy();
  setupSearch();
  setupKeys();
  markHits();
  setupLive();
  setupPrint();
})();
