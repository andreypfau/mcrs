(() => {
  // To add a language: add an entry here and a site/i18n/<code>.json holding the same keys
  // as ru.json, then run `node scripts/check-site-i18n.mjs`. The switcher renders from this list.
  const LANGUAGES = [
    { code: 'en', label: 'EN', name: 'English' },
    { code: 'ru', label: 'RU', name: 'Русский' },
  ];
  const ATTRS = ['alt', 'aria-label', 'content'];
  const STORAGE_KEY = 'lang';
  const root = document.documentElement;

  const saved = () => {
    try { return localStorage.getItem(STORAGE_KEY); } catch { return null; }
  };
  const save = (code) => {
    try { localStorage.setItem(STORAGE_KEY, code); } catch { /* private mode: the choice lasts until reload */ }
  };

  const supported = (tag) => LANGUAGES.find((l) => l.code.toLowerCase() === tag)?.code;
  const match = (tag) => {
    const lower = tag.toLowerCase().replace('_', '-');
    return supported(lower) ?? supported(lower.split('-')[0]);
  };
  const pick = () => [new URLSearchParams(location.search).get('lang'), saved(), ...(navigator.languages || [])]
    .filter(Boolean)
    .map(match)
    .find(Boolean) || 'en';

  let lang = 'en';
  let dict = {};
  let english;
  const t = (key, vars = {}) => {
    english ||= JSON.parse(document.getElementById('i18n-en').textContent);
    let text = dict[key] ?? english[key] ?? key;
    if (typeof text === 'object') text = text[new Intl.PluralRules(lang).select(vars.n)] ?? text.other;
    return text.replace(/\{(\w+)\}/g, (match, name) => vars[name] ?? match);
  };
  window.I18N = { t };

  const build = (text, nodes) => {
    const kids = nodes.filter((n) => n.nodeType === Node.ELEMENT_NODE);
    const out = [];
    let from = 0;
    for (const m of text.matchAll(/<(\d+)(?:\/>|>(.*?)<\/\1>)/gs)) {
      out.push(document.createTextNode(text.slice(from, m.index)));
      from = m.index + m[0].length;
      const kid = kids[m[1]]?.cloneNode(true);
      if (kid && m[2] !== undefined) kid.textContent = m[2];
      out.push(kid || document.createTextNode(m[2] ?? ''));
    }
    out.push(document.createTextNode(text.slice(from)));
    return out;
  };

  let content;
  let attrs;
  const capture = () => {
    content = [...document.querySelectorAll('[data-i18n]')]
      .map((el) => ({ el, key: el.dataset.i18n, nodes: [...el.childNodes] }));
    attrs = ATTRS.flatMap((attr) => [...document.querySelectorAll(`[data-i18n-${attr}]`)]
      .map((el) => ({ el, attr, key: el.getAttribute(`data-i18n-${attr}`), source: el.getAttribute(attr) })));
  };
  const apply = (code, next) => {
    if (!content) capture();
    lang = code;
    dict = next;
    root.lang = code;
    content.forEach(({ el, key, nodes }) => {
      el.replaceChildren(...(typeof dict[key] === 'string' ? build(dict[key], nodes) : nodes));
    });
    attrs.forEach(({ el, attr, key, source }) => el.setAttribute(attr, dict[key] ?? source));
    document.querySelectorAll('[data-lang-switcher] button')
      .forEach((b) => b.setAttribute('aria-pressed', String(b.lang === code)));
    document.dispatchEvent(new Event('i18n:change'));
  };

  const domReady = new Promise((resolve) => document.addEventListener('DOMContentLoaded', resolve, { once: true }));

  const loading = {};
  const load = (code) => {
    loading[code] ||= fetch(`i18n/${code}.json`)
      .then((r) => {
        if (!r.ok) throw new Error(`${r.status} for ${code}`);
        return r.json();
      })
      .catch((err) => {
        delete loading[code];
        console.warn(`Could not load the ${code} dictionary:`, err);
        throw err;
      });
    return loading[code];
  };

  let latest = 0;
  const switchTo = async (code) => {
    const mine = ++latest;
    const next = code === 'en' ? {} : await load(code);
    await domReady;
    if (mine !== latest) return false;
    apply(code, next);
    return true;
  };

  const choose = async (code) => {
    const switched = await switchTo(code).catch(() => false);
    if (!switched) return;
    save(code);
    const url = new URL(location.href);
    url.searchParams.delete('lang');
    history.replaceState(history.state, '', url);
  };

  domReady.then(() => {
    const box = document.querySelector('[data-lang-switcher]');
    LANGUAGES.forEach(({ code, label, name }) => {
      const b = document.createElement('button');
      b.type = 'button';
      b.lang = code;
      b.title = name;
      b.textContent = label;
      b.setAttribute('aria-pressed', String(code === lang));
      b.addEventListener('click', () => choose(code));
      box.append(b);
    });
  });

  const first = pick();
  if (first !== 'en') {
    const reveal = () => root.classList.remove('i18n-pending');
    root.classList.add('i18n-pending');
    setTimeout(reveal, 3000);
    switchTo(first).catch(() => {}).finally(reveal);
  }
})();
