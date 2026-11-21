/* Offline language switching. Catalogs are served by this website, never a translation service. */
(function () {
  'use strict';
  var root = document.documentElement;
  var data = document.getElementById('page-translations');
  if (!data) return;
  var page = JSON.parse(data.textContent);
  var active = 'en', dictionary = {}, manifest, generation = 0;
  var dialog = document.querySelector('.language-dialog');
  var opener = document.querySelector('[data-language-open]');
  var search = document.querySelector('[data-language-search]');
  var options = document.querySelector('[data-language-options]');
  var notice = document.querySelector('[data-translation-notice]');
  var comments = {}, regions = [];
  var walker = document.createTreeWalker(document, NodeFilter.SHOW_COMMENT);
  while (walker.nextNode()) {
    var match = walker.currentNode.data.match(/^(\/?)swoop-text:(\d+)$/);
    if (!match) continue;
    var id = match[2];
    comments[id] = comments[id] || {};
    comments[id][match[1] ? 'end' : 'start'] = walker.currentNode;
  }
  page.segments.forEach(function (segment) {
    var nodes = comments[segment.id];
    if (!nodes || !nodes.start || !nodes.end) return;
    var range = document.createRange();
    range.setStartAfter(nodes.start); range.setEndBefore(nodes.end);
    var wrapper = document.createElement('div');
    wrapper.appendChild(range.cloneContents());
    regions.push({segment: segment, start: nodes.start, end: nodes.end, original: wrapper.innerHTML});
  });
  var attributes = Array.from(document.querySelectorAll('[data-l10n-attrs]')).map(function (node) {
    return {node: node, values: JSON.parse(node.getAttribute('data-l10n-attrs'))};
  });
  var originalTitle = document.title;
  var escape = function (value) {
    var text = document.createElement('textarea'); text.innerHTML = value;
    var span = document.createElement('span'); span.textContent = text.value;
    return span.innerHTML;
  };
  function translate(value) { return dictionary[value] || value; }
  window.SwoopdlTranslate = translate;
  window.SwoopdlHasTranslation = function (value) { return Object.prototype.hasOwnProperty.call(dictionary, value); };
  function restore(text, marks) {
    var result = '', position = 0;
    text.replace(/<(\/?)([gx])(\d+)(\/?)>/g, function (token, closing, kind, number, self, offset) {
      result += escape(text.slice(position, offset));
      result += marks[(closing ? '/' : '') + kind + number] || '';
      position = offset + token.length;
      return token;
    });
    return result + escape(text.slice(position));
  }
  function partial(source, originalMarks) {
    var changed = false, missing = 0, marks = Object.assign({}, originalMarks);
    var value = source.split(/(<\/?[gx]\d+\/?>)/g).map(function (part) {
      if (/^<\/?[gx]\d+\/?>$/.test(part)) return part;
      var clean = part.trim();
      var translated = dictionary[clean];
      if (translated) { changed = true; return part.replace(clean, translated); }
      if (!/[a-zA-Z]{3}/.test(clean) || /^(Swoop(?:dl)?|Windows|Linux|macOS|Firefox|Chromium|Rust|GPL|MCP)$/.test(clean)) return part;
      var number = 90000 + missing++;
      marks['g' + number] = '<span lang="en" dir="ltr" data-english-fallback="">';
      marks['/g' + number] = '</span>';
      return '<g' + number + '>' + part + '</g' + number + '>';
    }).join('');
    return changed ? {text: value, marks: marks, missing: missing} : null;
  }
  function updateLinks() {
    document.querySelectorAll('a[href]').forEach(function (link) {
      var href = link.getAttribute('href');
      if (link.hasAttribute('data-language-choice')) return;
      if (!href.startsWith('/') || href.startsWith('//')) return;
      var url = new URL(href, location.origin);
      if (!url.pathname.endsWith('/') && !url.pathname.endsWith('.html')) return;
      if (active === 'en') url.searchParams.delete('lang');
      else url.searchParams.set('lang', active);
      link.setAttribute('href', url.pathname + url.search + url.hash);
    });
  }
  function renderOptions(filter) {
    options.replaceChildren();
    var query = (filter || '').toLocaleLowerCase();
    manifest.languages.forEach(function (language) {
      if (!(language.native + ' ' + language.english + ' ' + language.code).toLocaleLowerCase().includes(query)) return;
      var item = document.createElement('button');
      item.type = 'button'; item.className = 'language-option'; item.lang = language.code; item.dir = 'auto';
      item.setAttribute('aria-pressed', String(language.code === active));
      var name = document.createElement('strong'); name.textContent = language.native;
      var label = document.createElement('span'); label.textContent = language.english;
      item.append(name, label);
      item.addEventListener('click', function () { setLanguage(language.code, true); dialog.close(); });
      options.appendChild(item);
    });
    document.querySelector('[data-language-empty]').hidden = !!options.children.length;
  }
  async function setLanguage(code, remember) {
    var language = manifest.languages.find(function (item) { return item.code === code; });
    if (!language) language = manifest.languages[0];
    var token = ++generation;
    try {
      var response, catalog = {};
      if (language.code !== 'en') {
        response = await fetch('/locales/' + language.code + '.json?v=' + manifest.version);
        if (!response.ok) throw new Error('Language catalog is unavailable');
        catalog = await response.json();
      }
      if (token !== generation) return;
      active = language.code; dictionary = catalog.translations || {};
      root.lang = active; root.dir = language.dir;
      var missing = 0;
      regions.forEach(function (region) {
        if (!region.start.isConnected || !region.end.isConnected) return;
        var source = region.segment.source;
        var translation = active === 'en' ? null : dictionary[source];
        var mixed = active !== 'en' && !translation ? partial(source, region.segment.marks) : null;
        var range = document.createRange();
        range.setStartAfter(region.start); range.setEndBefore(region.end);
        range.deleteContents();
        var template = document.createElement('template');
        template.innerHTML = translation ? restore(translation, region.segment.marks) : mixed ? restore(mixed.text, mixed.marks) : region.original;
        range.insertNode(template.content);
        var parent = region.start.parentElement;
        if (parent && active !== 'en' && !translation && !mixed && /[a-zA-Z]{3}/.test(source) && !/^Swoopdl$/.test(source)) {
          parent.setAttribute('data-english-fallback', ''); parent.lang = 'en'; parent.dir = 'ltr'; missing++;
        } else if (parent && parent.hasAttribute('data-english-fallback')) {
          parent.removeAttribute('data-english-fallback'); parent.removeAttribute('lang'); parent.removeAttribute('dir');
        }
      });
      attributes.forEach(function (item) {
        Object.keys(item.values).forEach(function (name) { item.node.setAttribute(name, translate(item.values[name])); });
      });
      document.title = translate(originalTitle);
      document.querySelector('[data-current-language]').textContent = active.split('-')[0].toUpperCase();
      opener.setAttribute('aria-label', translate('Choose your language') + ': ' + language.native);
      document.querySelector('[data-translation-message]').textContent = translate('Some content is shown in English.');
      document.querySelector('[data-english-original]').textContent = translate('Read in English');
      notice.hidden = active === 'en' || missing === 0;
      updateLinks(); renderOptions(search.value);
      var url = new URL(location.href);
      if (active === 'en') url.searchParams.delete('lang'); else url.searchParams.set('lang', active);
      history.replaceState(null, '', url.pathname + url.search + url.hash);
      if (remember) { try { localStorage.setItem('swoopdl-language', active); } catch (error) { /* Storage can be disabled. */ } }
      document.dispatchEvent(new CustomEvent('swoopdl:language', {detail: {language: active, missing: missing}}));
      // Runtime previews can replace their own text; include their current fallback metadata.
      notice.hidden = active === 'en' || !document.querySelector('[data-english-fallback]');
    } catch (error) {
      if (token !== generation) return;
      notice.hidden = false;
      document.querySelector('[data-translation-message]').textContent = 'This language could not be loaded. Please try again.';
    }
  }
  fetch('/locales/index.json', {cache: 'no-cache'}).then(function (response) {
    if (!response.ok) throw new Error('Language list is unavailable');
    return response.json();
  }).then(function (value) {
    manifest = value;
    function openLanguages(event) {
      if (typeof dialog.showModal !== 'function') return;
      event.preventDefault(); renderOptions(''); search.value = ''; dialog.showModal(); search.focus();
    }
    opener.addEventListener('click', openLanguages);
    if (typeof dialog.showModal === 'function') {
      opener.setAttribute('role', 'button');
      opener.addEventListener('keydown', function (event) { if (event.key === ' ') openLanguages(event); });
    }
    dialog.querySelector('.language-close').addEventListener('click', function () { dialog.close(); });
    dialog.addEventListener('click', function (event) { if (event.target === dialog) dialog.close(); });
    dialog.addEventListener('keydown', function (event) {
      if (event.key === 'Escape') { event.preventDefault(); dialog.close(); }
    });
    dialog.addEventListener('close', function () { opener.focus(); });
    search.addEventListener('input', function () { renderOptions(search.value); });
    document.addEventListener('click', function (event) {
      var link = event.target.closest('[data-language-choice]');
      if (!link) return;
      event.preventDefault(); setLanguage(link.getAttribute('data-language-choice'), true);
    });
    var wanted = new URLSearchParams(location.search).get('lang');
    if (!wanted) { try { wanted = localStorage.getItem('swoopdl-language'); } catch (error) { /* Default to English. */ } }
    setLanguage(wanted || 'en', false);
  }).catch(function () { /* The link to the language directory remains usable without enhancement. */ });
})();
