/* Swoopdl website interactions. Content and navigation remain usable without JavaScript. */
(function () {
  'use strict';
  var root = document.documentElement;
  var scheme = window.matchMedia('(prefers-color-scheme: dark)');
  var button = document.querySelector('.theme-toggle');
  function tr(value) { return window.SwoopdlTranslate ? window.SwoopdlTranslate(value) : value; }
  function currentTheme() {
    return root.getAttribute('data-theme') || (scheme.matches ? 'dark' : 'light');
  }
  function updateThemeButton() {
    if (!button) return;
    var dark = currentTheme() === 'dark';
    button.setAttribute('aria-label', tr(dark ? 'Switch to light theme' : 'Switch to dark theme'));
    button.setAttribute('aria-pressed', String(dark));
    button.setAttribute('title', tr(dark ? 'Switch to light theme' : 'Switch to dark theme'));
    var chromeColor = document.querySelector('meta[name="theme-color"]');
    if (chromeColor) chromeColor.setAttribute('content', dark ? '#101722' : '#F8FAFC');
  }
  if (button) {
    button.hidden = false;
    button.addEventListener('click', function () {
      var next = currentTheme() === 'dark' ? 'light' : 'dark';
      root.setAttribute('data-theme', next);
      try { localStorage.setItem('swoopdl-theme', next); } catch (e) { /* Keep the current page usable when storage is unavailable. */ }
      updateThemeButton();
    });
    if (scheme.addEventListener) scheme.addEventListener('change', updateThemeButton);
    updateThemeButton();
    document.addEventListener('swoopdl:language', updateThemeButton);
  }

  var menu = document.querySelector('.mobile-menu');
  if (menu) {
    var summary = menu.querySelector('summary');
    menu.addEventListener('toggle', function () {
      summary.setAttribute('aria-label', tr(menu.open ? 'Close navigation' : 'Open navigation'));
    });
    document.addEventListener('click', function (event) {
      if (menu.open && !menu.contains(event.target)) menu.open = false;
    });
    document.addEventListener('keydown', function (event) {
      if (event.key === 'Escape' && menu.open) {
        menu.open = false;
        summary.focus();
      }
    });
    menu.querySelectorAll('a').forEach(function (link) {
      link.addEventListener('click', function () { menu.open = false; });
    });
  }

  var previews = {
    files: { icon: 'folder', label: 'Ready for the next thing', title: 'Weekend collection.zip', description: 'A big file. A little more control.', stat: '68% downloaded', meta: 'Resume supported', note: 'Your progress stays with you.' },
    torrents: { icon: 'torrent', label: 'All the pieces, together', title: 'Linux desktop.iso', description: 'Choose the files you want to keep.', stat: '68% downloaded', meta: 'Torrent download', note: 'Pick files. Follow peers and pieces.' },
    video: { icon: 'video', label: 'From a page to your collection', title: 'A little inspiration.mp4', description: 'Choose an available format and quality.', stat: '68% downloaded', meta: 'Video + audio', note: 'Supported links, with yt-dlp integration.' }
  };
  var tabGroup = document.querySelector('[data-tabs]');
  if (tabGroup) {
    var tabs = Array.from(tabGroup.querySelectorAll('[data-tab]'));
    function choose(tab) {
      var data = previews[tab.getAttribute('data-tab')];
      if (!data) return;
      tabs.forEach(function (item) { item.setAttribute('aria-pressed', String(item === tab)); });
      Object.keys(data).forEach(function (key) {
        var target = document.querySelector('[data-preview-' + key + ']');
        if (!target) return;
        if (key === 'icon') target.setAttribute('href', '/assets/icons.svg#' + data[key]);
        else {
          target.textContent = tr(data[key]);
          var fallback = document.documentElement.lang !== 'en' && window.SwoopdlHasTranslation && !window.SwoopdlHasTranslation(data[key]);
          if (fallback) {
            target.setAttribute('data-english-fallback', ''); target.lang = 'en'; target.dir = 'ltr';
          } else {
            target.removeAttribute('data-english-fallback'); target.removeAttribute('lang'); target.removeAttribute('dir');
          }
        }
      });
    }
    tabs.forEach(function (tab, index) {
      tab.addEventListener('click', function () { choose(tab); });
      tab.addEventListener('keydown', function (event) {
        var target = index;
        if (event.key === 'ArrowDown' || event.key === 'ArrowRight') target = (index + 1) % tabs.length;
        else if (event.key === 'ArrowUp' || event.key === 'ArrowLeft') target = (index + tabs.length - 1) % tabs.length;
        else if (event.key === 'Home') target = 0;
        else if (event.key === 'End') target = tabs.length - 1;
        else return;
        event.preventDefault();
        tabs[target].focus();
        choose(tabs[target]);
      });
    });
    document.addEventListener('swoopdl:language', function () {
      choose(tabGroup.querySelector('[aria-pressed="true"]') || tabs[0]);
    });
  }

  var mobileNote = document.querySelector('[data-mobile-note]');
  if (mobileNote && /android|iphone|ipad|ipod|mobile/i.test(navigator.userAgent)) mobileNote.hidden = false;
  document.querySelectorAll('.desktop-nav a, .mobile-menu nav a').forEach(function (link) {
    var path = window.location.pathname;
    var destination = link.getAttribute('href');
    if (path === destination || (destination.endsWith('/') && path.indexOf(destination) === 0)) link.setAttribute('aria-current', 'page');
  });
})();
