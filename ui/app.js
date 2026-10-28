/* Spiderflayer Launcher front end. Plain JS, no framework, no build step. */
(() => {
  'use strict';

  const TAURI = window.__TAURI__;
  const invoke = (cmd, args) => TAURI.core.invoke(cmd, args || {});
  const listen = (ev, cb) => TAURI.event.listen(ev, cb);

  const $ = (sel, root = document) => root.querySelector(sel);
  const $$ = (sel, root = document) => Array.from(root.querySelectorAll(sel));
  const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const debounce = (fn, ms) => { let t; return (...a) => { clearTimeout(t); t = setTimeout(() => fn(...a), ms); }; };
  const fmt = (n) => new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 1 }).format(Number(n) || 0);
  const safeUrl = (u) => (typeof u === 'string' && /^https:\/\//.test(u) ? u : '');
  const fileB64 = (f) => new Promise((res, rej) => { const r = new FileReader(); r.onload = () => res(String(r.result).split(',')[1]); r.onerror = rej; r.readAsDataURL(f); });
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const errText = (e) => (typeof e === 'string' ? e : (e && e.message) || String(e));
  const fmtDate = (d) => { try { return new Date(d).toLocaleDateString(undefined, { year: 'numeric', month: 'short', day: 'numeric' }); } catch (e) { return ''; } };
  const mcKey = (v) => { const m = /^(\d+)\.(\d+)(?:\.(\d+))?$/.exec(v); return m ? [+m[1], +m[2], +(m[3] || 0)] : null; };
  // Release versions newest first, then everything else in the order given.
  const sortMc = (list) => {
    const rel = list.filter((v) => mcKey(v)).sort((a, b) => { const x = mcKey(a), y = mcKey(b); return y[0] - x[0] || y[1] - x[1] || y[2] - x[2]; });
    return rel.concat(list.filter((v) => !mcKey(v)));
  };

  // ---------- icons (Lucide-style strokes, inline so nothing is downloaded) ----------
  const ICON = {
    home: '<path d="M3 11.5 12 4l9 7.5"/><path d="M5.5 10v9.5h13V10"/><path d="M10 19.5v-5h4v5"/>',
    compass: '<circle cx="12" cy="12" r="9"/><path d="m15.8 8.2-2.2 5.4-5.4 2.2 2.2-5.4z"/>',
    box: '<path d="M21 8 12 3 3 8v8l9 5 9-5z"/><path d="m3 8 9 5 9-5M12 13v8"/>',
    plus: '<path d="M12 5v14M5 12h14"/>',
    gear: '<circle cx="12" cy="12" r="3"/><path d="M12 2.5v3M12 18.5v3M2.5 12h3M18.5 12h3M5.3 5.3l2.1 2.1M16.6 16.6l2.1 2.1M5.3 18.7l2.1-2.1M16.6 7.4l2.1-2.1"/>',
    play: '<path d="M7 4.5v15l13-7.5z" fill="currentColor" stroke="none"/>',
    stop: '<rect x="6" y="6" width="12" height="12" rx="3" fill="currentColor" stroke="none"/>',
    download: '<path d="M12 3.5v12M7 10.5l5 5 5-5M4.5 20h15"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-4-4"/>',
    folder: '<path d="M3 7.5A2.5 2.5 0 0 1 5.5 5H9l2 2.2h7.5A2.5 2.5 0 0 1 21 9.7v7.8a2.5 2.5 0 0 1-2.5 2.5h-13A2.5 2.5 0 0 1 3 17.5z"/>',
    trash: '<path d="M4.5 7h15M9.5 7V4.5h5V7M6.5 7l1 13h9l1-13"/>',
    x: '<path d="m6 6 12 12M18 6 6 18"/>',
    min: '<path d="M5.5 12h13"/>',
    max: '<rect x="5.5" y="5.5" width="13" height="13" rx="3.5"/>',
    left: '<path d="m14.5 5.5-6.5 6.5 6.5 6.5"/>',
    user: '<circle cx="12" cy="8.5" r="3.7"/><path d="M4.8 20c.9-3.6 3.8-5.4 7.2-5.4s6.3 1.8 7.2 5.4"/>',
    check: '<path d="m5 12.5 4.5 4.5L19 7.5"/>',
    link: '<path d="M10 14a4.5 4.5 0 0 0 6.4 0l3-3a4.5 4.5 0 0 0-6.4-6.4l-1 1M14 10a4.5 4.5 0 0 0-6.4 0l-3 3a4.5 4.5 0 0 0 6.4 6.4l1-1"/>',
    file: '<path d="M6.5 3.5h7l4 4v13h-11z"/><path d="M13.5 3.5v4h4"/>',
    upload: '<path d="M12 15.5v-12M7 8.5l5-5 5 5M4.5 20h15"/>',
    image: '<rect x="3.5" y="4.5" width="17" height="15" rx="4"/><circle cx="9" cy="10" r="1.6"/><path d="m4 17 5-4.5 4 3.5 3-2.5 4 3.5"/>',
    reset: '<path d="M4.5 12a7.5 7.5 0 1 0 2.4-5.5M4.5 4.5v4h4"/>',
    heart: '<path d="M12 20s-7.5-4.6-9-9.6C2 6.9 4.3 4.5 7 4.5c1.9 0 3.4 1 5 3 1.6-2 3.1-3 5-3 2.700 0 5 2.400 4 5.900-1.500 5-9 9.600-9 9.600z"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3.500 2"/>',
    ext: '<path d="M14 4h6v6M20 4l-9 9M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
    right: '<path d="m9.500 5.500 6.500 6.500-6.500 6.500"/>',
  };
  const ic = (name, size = 20) => `<svg class="ic" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">${ICON[name] || ''}</svg>`;

  const LOADER = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' };
  const KINDS = [
    { id: 'modpack', label: 'Modpacks' },
    { id: 'mod', label: 'Mods' },
    { id: 'resourcepack', label: 'Resource packs' },
    { id: 'shader', label: 'Shaders' },
  ];
  const FOLDERS = [
    { id: 'mods', label: 'Mods' },
    { id: 'resourcepacks', label: 'Resource packs' },
    { id: 'shaderpacks', label: 'Shaders' },
  ];
  const SORTS = [['relevance', 'Relevance'], ['downloads', 'Downloads'], ['newest', 'Newest'], ['updated', 'Recently updated']];
  const ACCENTS = ['#8b5cf6', '#a78bfa', '#c084fc', '#ec4899', '#ff3b4a'];

  // ---------- state ----------
  const S = {
    view: { name: 'home' },
    hist: [],
    settings: null,
    instances: [],
    acct: { accounts: [], selected: '' },
    running: new Set(),
    preparing: new Set(),
    logs: {},
    versions: null,
    featured: null,
    bgUrl: '',
    browse: { kind: 'mod', query: '', source: 'modrinth', mc: '', loader: '', sort: 'relevance', target: '', offset: 0, hits: [], total: 0, loading: false, error: '', seq: 0 },
    content: { kind: 'mods', files: [] },
    proj: null,
    lb: null,
    msReady: false,
  };

  // ---------- helpers ----------
  const hue = (s) => { let h = 0; for (const c of String(s)) h = (h * 31 + c.charCodeAt(0)) >>> 0; return (h % 60) - 20; };
  const pic = (name, cls = '') => `<div class="pic ${cls}" style="--h:${hue(name)}">${esc((String(name)[0] || '?').toUpperCase())}</div>`;
  const projPic = (url, name, cls = '') => (safeUrl(url) ? `<img class="pic ${cls}" src="${esc(url)}" alt="" loading="lazy" referrerpolicy="no-referrer">` : pic(name, cls));
  const inst = (id) => S.instances.find((i) => i.id === id);
  const instLabel = (i) => `${LOADER[i.loader] || i.loader}${i.loader_version && i.loader !== 'vanilla' ? ' ' + i.loader_version : ''} · ${i.mc_version}`;
  const acctName = () => { const a = S.acct.accounts.find((x) => x.id === S.acct.selected) || S.acct.accounts[0]; return a ? a.name : ''; };

  function toast(msg, type = 'ok') {
    const el = document.createElement('div');
    el.className = `toast ${type}`;
    el.innerHTML = `<div><b>${type === 'error' ? 'Something went wrong' : 'Done'}</b><div class="muted selectable">${esc(msg)}</div></div>`;
    $('#toasts').appendChild(el);
    setTimeout(() => el.remove(), type === 'error' ? 9000 : 4200);
  }
  const fail = (e) => toast(typeof e === 'string' ? e : (e && e.message) || 'Unknown error', 'error');

  // ---------- modal ----------
  function openModal(inner, opts = {}) {
    closeModal();
    const root = $('#modal-root');
    root.innerHTML = `<div class="scrim"><div class="modal glass ${opts.wide ? 'wide' : ''}" role="dialog" aria-modal="true">${inner}</div></div>`;
    const scrim = root.firstElementChild;
    scrim.addEventListener('mousedown', (e) => { if (e.target === scrim) closeModal(); });
    S.modalClose = opts.onClose || null;
    const first = $('input, select', scrim);
    if (first && !opts.noFocus) first.focus();
    return scrim.firstElementChild;
  }
  function closeModal() {
    const cb = S.modalClose;
    S.modalClose = null;
    $('#modal-root').innerHTML = '';
    if (cb) cb();
  }
  document.addEventListener('keydown', (e) => {
    if (S.lb) {
      if (e.key === 'Escape') { closeLightbox(); return; }
      if (e.key === 'ArrowLeft') lbStep(-1);
      if (e.key === 'ArrowRight') lbStep(1);
      return;
    }
    if (e.key === 'Escape') closeModal();
  });

  // ---------- theme & background ----------
  function applyTheme() {
    const s = S.settings;
    const r = document.documentElement;
    r.dataset.theme = s.theme || 'oled';
    r.dataset.surface = ['aero', 'frosted', 'solid'].includes(s.surface) ? s.surface : 'aero';
    r.style.setProperty('--accent', s.accent || '#8b5cf6');
    r.style.setProperty('--glass', String((s.glass ?? 60) / 100));
    r.style.setProperty('--bgblur', `${s.bg_blur || 0}px`);
    r.style.setProperty('--dim', String((s.bg_dim ?? 25) / 100));
  }
  const saveSettings = debounce(() => invoke('save_settings', { settings: S.settings }).catch(fail), 350);

  async function loadBackground() {
    let url = 'default-bg.webp';
    try {
      if (S.settings.background) {
        const b = await invoke('get_background');
        if (b) {
          const bin = atob(b.data);
          const arr = new Uint8Array(bin.length);
          for (let i = 0; i < bin.length; i++) arr[i] = bin.charCodeAt(i);
          url = URL.createObjectURL(new Blob([arr], { type: b.mime }));
        }
      }
    } catch (e) { /* fall back to the built-in picture */ }
    const old = S.bgUrl;
    S.bgUrl = url;
    $('#bg').style.backgroundImage = `url("${url}")`;
    if (old && old.startsWith('blob:')) URL.revokeObjectURL(old);
  }

  // ---------- data ----------
  async function refreshInstances() { S.instances = await invoke('list_instances'); }
  async function refreshAccounts() { S.acct = await invoke('accounts_list'); }
  async function ensureVersions() {
    if (!S.versions) S.versions = await invoke('mc_versions');
    return S.versions;
  }

  // ---------- navigation ----------
  function go(name, params = {}, push = true) {
    if (push && S.view) S.hist.push(S.view);
    S.view = { name, ...params };
    render();
  }
  function back() {
    const v = S.hist.pop();
    if (v) { S.view = v; render(); }
  }

  function renderRail() {
    const a = S.acct.accounts.find((x) => x.id === S.acct.selected) || S.acct.accounts[0];
    const v = S.view.name;
    const item = (id, icon, label, active) =>
      `<button class="side-item rail-btn ${active ? 'active' : ''}" data-act="nav" data-to="${id}">${ic(icon, 18)}<span>${label}</span></button>`;
    const mine = S.instances.slice(0, 14).map((i) =>
      `<button class="side-item sub ${v === 'instance' && S.view.id === i.id ? 'active' : ''}" data-act="open-inst" data-id="${esc(i.id)}" title="${esc(i.name)}"><b class="mini-pic" style="--h:${hue(i.name)}">${esc((i.name[0] || '?').toUpperCase())}</b><span>${esc(i.name)}</span>${S.running.has(i.id) ? '<i class="live"></i>' : ''}</button>`).join('');
    $('#rail').innerHTML = `
      <div class="brand" data-tauri-drag-region><img src="logo.png" alt=""><b>Spiderflayer</b></div>
      <div class="side-group">
        ${item('home', 'home', 'Home', v === 'home')}
        ${item('browse', 'compass', 'Discover', v === 'browse' || v === 'project')}
        ${item('library', 'box', 'Library', v === 'library')}
      </div>
      <div class="side-label"><span>Instances</span><button class="side-add" data-act="create" aria-label="New instance" title="New instance">${ic('plus', 15)}</button></div>
      <div class="side-list">${mine || '<div class="side-empty">Nothing here yet</div>'}</div>
      <div class="side-foot">
        ${item('settings', 'gear', 'Settings', v === 'settings')}
        <button class="acct-row" data-act="accounts" aria-label="Accounts">
          <span class="avatar">${a ? esc(a.name[0].toUpperCase()) + `<img src="https://minotar.net/helm/${encodeURIComponent(a.name)}/64.png" alt="" onerror="this.remove()">` : ic('user', 18)}</span>
          <span class="acct-name">${a ? esc(a.name) : 'Add account'}<small>${a ? (a.kind === 'microsoft' ? 'Microsoft' : 'Offline') : 'Not signed in'}</small></span>
        </button>
      </div>`;
    $('#runstate').className = S.running.size ? 'on' : '';
    $('#runstate').textContent = S.running.size ? '● Minecraft is running' : '';
  }

  function crumb() {
    const v = S.view;
    const names = { home: 'Home', browse: 'Discover content', library: 'Library', settings: 'Settings' };
    if (v.name === 'instance') {
      const i = inst(v.id);
      return `<a data-act="nav" data-to="library" style="cursor:pointer;color:var(--muted)">Library</a><span class="sep">›</span>${esc(i ? i.name : '')}`;
    }
    if (v.name === 'project') {
      return `<a data-act="nav" data-to="browse" style="cursor:pointer;color:var(--muted)">Discover content</a><span class="sep">›</span>${esc(S.proj && S.proj.data ? S.proj.data.title : v.title || '')}`;
    }
    return names[v.name] || '';
  }

  function render() {
    renderRail();
    $('#crumb').innerHTML = crumb();
    $('#back').disabled = S.hist.length === 0;
    const view = $('#view');
    view.scrollTop = 0;
    const r = { home: renderHome, browse: renderBrowse, library: renderLibrary, instance: renderInstance, settings: renderSettings, project: renderProject }[S.view.name];
    (r || renderHome)(view);
  }

  // ---------- play buttons ----------
  function playState(id) { return S.running.has(id) ? 'running' : S.preparing.has(id) ? 'preparing' : 'idle'; }
  function playHtml(id, fab) {
    const st = playState(id);
    if (fab) return st === 'running' ? ic('stop', 18) : st === 'preparing' ? '…' : ic('play', 20);
    return st === 'running' ? `${ic('stop', 18)} Stop` : st === 'preparing' ? 'Preparing…' : `${ic('play', 18)} Play`;
  }
  function updatePlay() {
    $$('.js-play').forEach((b) => {
      const fab = b.classList.contains('play-fab');
      b.innerHTML = playHtml(b.dataset.id, fab);
      b.classList.toggle('stop', fab && playState(b.dataset.id) === 'running');
      b.disabled = playState(b.dataset.id) === 'preparing';
      b.setAttribute('aria-label', playState(b.dataset.id) === 'running' ? 'Stop' : 'Play');
    });
    renderRail();
  }

  async function play(id) {
    if (S.running.has(id)) { await invoke('kill_game', { id }).catch(fail); return; }
    if (S.preparing.has(id)) return;
    S.preparing.add(id);
    updatePlay();
    try {
      await invoke('launch_game', { id });
      S.running.add(id);
    } catch (e) {
      fail(e);
    } finally {
      S.preparing.delete(id);
      hideProgress();
      updatePlay();
      refreshInstances().then(() => { if (S.view.name === 'library' || S.view.name === 'home') render(); });
    }
  }

  // ---------- progress ----------
  let progTimer;
  function showProgress(p) {
    const el = $('#progress');
    el.hidden = false;
    $('.p-msg', el).textContent = p.total ? `${p.message} (${p.done}/${p.total})` : p.message || 'Working…';
    const bar = $('.bar', el);
    bar.classList.toggle('indet', !p.total);
    $('i', bar).style.width = p.total ? `${Math.min(100, Math.round((p.done / p.total) * 100))}%` : '';
    clearTimeout(progTimer);
    progTimer = setTimeout(hideProgress, 20000);
  }
  function hideProgress() { clearTimeout(progTimer); $('#progress').hidden = true; }

  // ---------- HOME ----------
  // Launchpad-style tile: big icon, name underneath, play button appears on hover/focus.
  function instCard(i) {
    return `<div class="card tile" data-act="open-inst" data-id="${esc(i.id)}" tabindex="0" role="button">
      ${pic(i.name, 'xl')}
      <div class="meta"><b>${esc(i.name)}</b><span>${esc(instLabel(i))}</span></div>
      <button class="play-fab js-play" data-act="play" data-id="${esc(i.id)}">${playHtml(i.id, true)}</button>
    </div>`;
  }

  function projCard(h) {
    return `<div class="card proj-card" data-act="open-project" data-source="${esc(h.source)}" data-pid="${esc(h.id)}" data-kind="${esc(h.kind || '')}" data-title="${esc(h.title)}" data-icon="${esc(h.icon || '')}" tabindex="0" role="button">
      ${projPic(h.icon, h.title)}
      <div class="meta"><b>${esc(h.title)}</b><span>by ${esc(h.author || 'unknown')} · ${fmt(h.downloads)} downloads</span></div>
      <div class="desc">${esc(h.description)}</div>
    </div>`;
  }

  async function renderHome(view) {
    const recent = S.instances.filter((i) => i.last_played).slice(0, 8);
    const hero = recent[0] || S.instances[0];
    const name = acctName();
    const hour = new Date().getHours();
    const greet = hour < 5 ? 'Good night' : hour < 12 ? 'Good morning' : hour < 18 ? 'Good afternoon' : 'Good evening';
    view.innerHTML = `
      <div class="greet">${greet}${name ? ', ' + esc(name) : ''}</div>
      ${hero
        ? `<section class="hero">
            ${pic(hero.name, 'xl')}
            <div class="grow"><div class="eyebrow">${hero.last_played ? 'Continue playing' : 'Ready to play'}</div><h1>${esc(hero.name)}</h1><div class="muted">${esc(instLabel(hero))}</div></div>
            <button class="btn primary lg js-play" data-act="play" data-id="${esc(hero.id)}">${playHtml(hero.id, false)}</button>
          </section>`
        : `<section class="hero empty-hero"><div class="grow"><div class="eyebrow">Welcome</div><h1>Let's get you playing</h1><div class="muted">Create an instance, or install a modpack from Discover.</div></div>
            <button class="btn lg" data-act="nav" data-to="browse">${ic('compass', 18)} Discover</button>
            <button class="btn primary lg" data-act="create">${ic('plus', 18)} New instance</button></section>`}
      ${recent.length > 1 ? `<div class="section-title">Recent</div><div class="tiles">${recent.slice(1).map(instCard).join('')}</div>` : ''}
      <div id="featured"></div>`;
    renderFeatured();
    if (!S.featured) {
      try {
        const [packs, mods] = await Promise.all([
          invoke('search', { source: 'modrinth', query: '', kind: 'modpack', mc: '', loader: '', sort: 'downloads', offset: 0 }),
          invoke('search', { source: 'modrinth', query: '', kind: 'mod', mc: '', loader: '', sort: 'downloads', offset: 0 }),
        ]);
        S.featured = { packs: packs.hits, mods: mods.hits };
      } catch (e) {
        S.featured = { packs: [], mods: [], offline: true };
      }
      if (S.view.name === 'home') renderFeatured();
    }
  }

  function renderFeatured() {
    const el = $('#featured');
    if (!el) return;
    const f = S.featured;
    if (!f) { el.innerHTML = `<div class="section-title">Popular modpacks</div><div class="muted" style="padding:0 6px">Loading…</div>`; return; }
    if (f.offline) { el.innerHTML = `<div class="section-title">Popular content</div><div class="muted" style="padding:0 6px">Couldn't reach Modrinth. Check your connection.</div>`; return; }
    el.innerHTML = `
      <div class="section-title">Popular modpacks</div><div class="hscroll">${f.packs.map(projCard).join('')}</div>
      <div class="section-title">Popular mods</div><div class="hscroll">${f.mods.map(projCard).join('')}</div>`;
  }

  // ---------- LIBRARY ----------
  function renderLibrary(view) {
    const filter = S.view.filter || 'all';
    const loaders = ['all', ...new Set(S.instances.map((i) => i.loader))];
    const list = S.instances.filter((i) => filter === 'all' || i.loader === filter);
    view.innerHTML = `
      <div class="row wrap" style="margin:6px 4px 16px">
        <h1 class="page-title grow" style="margin:0">Library</h1>
        <button class="btn" data-act="import-pack">${ic('upload', 18)} Import .mrpack</button>
        <button class="btn primary" data-act="create">${ic('plus', 18)} New instance</button>
        <input type="file" id="mrpack-file" accept=".mrpack,application/zip" hidden>
      </div>
      ${loaders.length > 2 ? `<div class="chips" style="margin:0 4px 14px">${loaders.map((l) => `<button class="chip ${filter === l ? 'active' : ''}" data-act="lib-filter" data-f="${l}">${l === 'all' ? 'All' : esc(LOADER[l] || l)}</button>`).join('')}</div>` : ''}
      <div class="tiles">${list.map(instCard).join('')}<div class="card tile add" data-act="create" tabindex="0" role="button"><div class="pic xl plus">${ic('plus', 30)}</div><div class="meta"><b>New instance</b></div></div></div>`;
    $('#mrpack-file').addEventListener('change', (e) => { const f = e.target.files[0]; e.target.value = ''; if (f) importPack(f); });
  }

  // ---------- INSTANCE ----------
  function renderInstance(view) {
    const i = inst(S.view.id);
    if (!i) { go('library', {}, false); return; }
    const tab = S.view.tab || 'content';
    view.innerHTML = `
      <div class="inst-head glass">
        ${pic(i.name, 'lg')}
        <div class="grow">
          <h1>${esc(i.name)}</h1>
          <div class="muted" style="margin:4px 0 12px">${esc(instLabel(i))}</div>
          <div class="row wrap">
            <button class="btn primary lg js-play" data-act="play" data-id="${esc(i.id)}">${playHtml(i.id, false)}</button>
            <button class="btn" data-act="open-folder" data-id="${esc(i.id)}">${ic('folder', 18)} Open folder</button>
            <button class="btn" data-act="add-content" data-id="${esc(i.id)}">${ic('plus', 18)} Add content</button>
          </div>
        </div>
      </div>
      <div class="tabs" style="margin:18px auto">
        ${[['content', 'Content'], ['logs', 'Logs'], ['options', 'Options']].map(([id, l]) => `<button class="tab ${tab === id ? 'active' : ''}" data-act="inst-tab" data-tab="${id}">${l}</button>`).join('')}
      </div>
      <div id="tab-body"></div>`;
    ({ content: instContent, logs: instLogs, options: instOptions }[tab])(i);
  }

  async function instContent(i) {
    const body = $('#tab-body');
    body.innerHTML = `<div class="panel">
      <div class="chips" style="margin-bottom:12px">${FOLDERS.map((f) => `<button class="chip ${S.content.kind === f.id ? 'active' : ''}" data-act="content-kind" data-k="${f.id}">${f.label}</button>`).join('')}</div>
      <div id="content-list" class="muted">Loading…</div></div>`;
    await loadContent(i);
  }

  async function loadContent(i) {
    try { S.content.files = await invoke('list_content', { id: i.id, kind: S.content.kind }); } catch (e) { S.content.files = []; fail(e); }
    const el = $('#content-list');
    if (!el) return;
    const label = FOLDERS.find((f) => f.id === S.content.kind).label.toLowerCase();
    if (!S.content.files.length) {
      el.innerHTML = `<div class="empty"><div class="big">No ${label} yet</div><div>Find some in Discover content.</div>
        <div style="margin-top:14px"><button class="btn primary" data-act="add-content" data-id="${esc(i.id)}">${ic('compass', 18)} Browse ${label}</button></div></div>`;
      return;
    }
    el.innerHTML = S.content.files.map((f, n) => `
      <div class="file-row ${f.enabled ? '' : 'off'}">
        <span class="name selectable" title="${esc(f.name)}">${esc(f.name.replace(/\.disabled$/, ''))}</span>
        <span class="faint">${(f.size / 1048576).toFixed(1)} MB</span>
        <label class="switch" title="${f.enabled ? 'Enabled' : 'Disabled'}"><input type="checkbox" data-act="toggle-file" data-n="${n}" ${f.enabled ? 'checked' : ''}><i></i></label>
        <button class="icon-btn" data-act="remove-file" data-n="${n}" aria-label="Remove" title="Remove">${ic('trash', 18)}</button>
      </div>`).join('');
  }

  function instLogs(i) {
    const lines = S.logs[i.id] || [];
    $('#tab-body').innerHTML = `<pre class="log selectable" id="logbox">${lines.length ? esc(lines.join('\n')) : 'Game output shows up here while Minecraft is running.'}</pre>`;
    const box = $('#logbox');
    box.scrollTop = box.scrollHeight;
  }

  function instOptions(i) {
    const mem = i.memory_mb || 0;
    $('#tab-body').innerHTML = `
      <div class="panel">
        <div class="set-grid">
          <div class="field"><label for="o-name">Name</label><input id="o-name" class="input" value="${esc(i.name)}" maxlength="60"></div>
          <div class="field"><label>Memory <span id="o-mem-l" style="text-transform:none;letter-spacing:0">${mem ? (mem / 1024).toFixed(1) + ' GB' : 'Default'}</span></label>
            <input id="o-mem" type="range" min="0" max="16384" step="256" value="${mem}">
            <span class="hint">Slide to 0 to use the global default (${((S.settings.default_memory_mb || 2048) / 1024).toFixed(1)} GB).</span></div>
        </div>
        <div class="field" style="margin-top:14px"><label for="o-args">Extra Java arguments</label><input id="o-args" class="input" value="${esc(i.java_args)}" placeholder="-XX:+UseG1GC"></div>
        <div class="row" style="margin-top:16px"><button class="btn primary" data-act="save-inst" data-id="${esc(i.id)}">Save changes</button></div>
      </div>
      <div class="panel row"><div class="grow"><b>Delete instance</b><div class="muted">Removes its mods, worlds and settings from this computer.</div></div>
        <button class="btn danger" data-act="delete-inst" data-id="${esc(i.id)}">${ic('trash', 18)} Delete</button></div>`;
    $('#o-mem').addEventListener('input', (e) => { const v = +e.target.value; $('#o-mem-l').textContent = v ? (v / 1024).toFixed(1) + ' GB' : 'Default'; });
  }

  // ---------- BROWSE ----------
  function renderBrowse(view) {
    const B = S.browse;
    const target = inst(B.target);
    if (target) { B.mc = target.mc_version; B.loader = target.loader === 'vanilla' ? '' : target.loader; }
    const releases = (S.versions || []).filter((v) => v.type === 'release');
    const showLoader = B.kind === 'mod' || B.kind === 'modpack';
    view.innerHTML = `
      <div class="tabs">${KINDS.map((k) => `<button class="tab ${B.kind === k.id ? 'active' : ''}" data-act="browse-kind" data-k="${k.id}">${k.label}</button>`).join('')}</div>
      <div class="row center-row">
        <div class="searchbar">${ic('search', 18)}<input id="q" class="input" placeholder="Search ${KINDS.find((k) => k.id === B.kind).label.toLowerCase()}…" value="${esc(B.query)}" autocomplete="off"></div>
        <select id="f-sort" class="input" style="width:170px">${SORTS.map(([v, l]) => `<option value="${v}" ${B.sort === v ? 'selected' : ''}>${l}</option>`).join('')}</select>
      </div>
      <div class="browse">
        <aside class="filters panel">
          <div class="field"><label for="f-source">Source</label>
            <select id="f-source" class="input"><option value="modrinth" ${B.source === 'modrinth' ? 'selected' : ''}>Modrinth</option><option value="curseforge" ${B.source === 'curseforge' ? 'selected' : ''}>CurseForge</option></select></div>
          ${B.kind !== 'modpack' ? `<div class="field"><label for="f-target">Install to</label>
            <select id="f-target" class="input"><option value="">Ask me each time</option>${S.instances.map((i) => `<option value="${esc(i.id)}" ${B.target === i.id ? 'selected' : ''}>${esc(i.name)}</option>`).join('')}</select></div>` : ''}
          <div class="field"><label for="f-mc">Game version</label>
            <select id="f-mc" class="input" ${target ? 'disabled' : ''}><option value="">Any version</option>${releases.map((v) => `<option ${B.mc === v.id ? 'selected' : ''}>${esc(v.id)}</option>`).join('')}${B.mc && !releases.some((v) => v.id === B.mc) ? `<option selected>${esc(B.mc)}</option>` : ''}</select></div>
          ${showLoader ? `<div class="field"><label for="f-loader">Loader</label>
            <select id="f-loader" class="input" ${target ? 'disabled' : ''}><option value="">Any loader</option>${['fabric', 'quilt', 'forge', 'neoforge'].map((l) => `<option value="${l}" ${B.loader === l ? 'selected' : ''}>${LOADER[l]}</option>`).join('')}</select></div>` : ''}
        </aside>
        <div class="results" id="results"></div>
      </div>`;
    ensureVersions().then(() => { if (S.view.name === 'browse' && !$('#f-mc option:nth-child(3)')) renderBrowse(view); }).catch(() => {});
    $('#q').addEventListener('input', debounce((e) => { B.query = e.target.value; runSearch(true); }, 350));
    const bind = (id, key) => { const el = $(id); if (el) el.addEventListener('change', (e) => { B[key] = e.target.value; if (key === 'target') { renderBrowse(view); } runSearch(true); }); };
    bind('#f-sort', 'sort'); bind('#f-source', 'source'); bind('#f-target', 'target'); bind('#f-mc', 'mc'); bind('#f-loader', 'loader');
    if (B.hits.length) renderResults(); else runSearch(true);
  }

  async function runSearch(reset) {
    const B = S.browse;
    if (reset) { B.offset = 0; B.hits = []; }
    const seq = ++B.seq;
    B.loading = true; B.error = '';
    renderResults();
    try {
      const r = await invoke('search', { source: B.source, query: B.query, kind: B.kind, mc: B.mc, loader: B.loader, sort: B.sort, offset: B.offset });
      if (seq !== B.seq) return;
      B.hits = B.hits.concat(r.hits);
      B.total = r.total || 0;
      B.offset = B.hits.length;
    } catch (e) {
      if (seq !== B.seq) return;
      B.error = typeof e === 'string' ? e : String(e && e.message || e);
    }
    B.loading = false;
    renderResults();
  }

  function renderResults() {
    const el = $('#results');
    if (!el) return;
    const B = S.browse;
    if (B.error) { el.innerHTML = `<div class="panel empty"><div class="big">Couldn't load results</div><div class="selectable">${esc(B.error)}</div></div>`; return; }
    if (B.loading && !B.hits.length) { el.innerHTML = `<div class="panel empty">Searching…</div>`; return; }
    if (!B.hits.length) { el.innerHTML = `<div class="panel empty"><div class="big">Nothing found</div><div>Try a different search or loosen the filters.</div></div>`; return; }
    const verb = 'Install';
    el.innerHTML = B.hits.map((h, n) => `
      <div class="proj" data-act="open-project" data-n="${n}" tabindex="0" role="link" style="cursor:pointer">
        ${projPic(h.icon, h.title)}
        <div class="meta">
          <div><span class="title">${esc(h.title)}</span><span class="by">by ${esc(h.author || 'unknown')}</span></div>
          <div class="desc">${esc(h.description)}</div>
          <div class="stat"><span>${ic('download', 13)}</span><span>${fmt(h.downloads)} downloads</span><span class="badge ${h.source === 'curseforge' ? 'red' : ''}">${h.source === 'curseforge' ? 'CurseForge' : 'Modrinth'}</span></div>
        </div>
        ${B.kind === 'modpack'
          ? `<button class="btn primary" data-act="open-versions" data-n="${n}">${ic('download', 16)} Choose version</button>`
          : `<button class="btn primary" data-act="install" data-n="${n}">${ic('download', 16)} ${verb}</button>`}
      </div>`).join('') +
      (B.hits.length < B.total ? `<button class="btn" data-act="more" ${B.loading ? 'disabled' : ''}>${B.loading ? 'Loading…' : 'Load more'}</button>` : '');
  }

  function pickInstance(title) {
    return new Promise((resolve) => {
      if (!S.instances.length) { toast('Create an instance first.', 'error'); openCreate(); resolve(null); return; }
      let chosen = null;
      const m = openModal(`<h2>${esc(title)}</h2><div class="muted">Choose where to put it.</div>
        <div class="body">${S.instances.map((i) => `<button class="acct" data-pick="${esc(i.id)}" style="text-align:left;cursor:pointer">${pic(i.name, 'sm')}<div class="grow"><b>${esc(i.name)}</b><div class="muted">${esc(instLabel(i))}</div></div></button>`).join('')}</div>`,
        { onClose: () => resolve(chosen) });
      m.addEventListener('click', (e) => { const b = e.target.closest('[data-pick]'); if (b) { chosen = b.dataset.pick; closeModal(); } });
    });
  }

  async function installProject(h, btn) {
    const B = S.browse;
    const kind = h.kind || B.kind;
    const orig = btn.innerHTML;
    const busy = (t) => { btn.disabled = true; btn.innerHTML = t; };
    const done = () => { btn.disabled = false; btn.innerHTML = orig; };
    try {
      if (kind === 'modpack') {
        if (h.source !== 'modrinth') { toast('CurseForge modpacks are not supported yet. Pick the Modrinth version of the pack.', 'error'); return; }
        busy('Installing…');
        const made = await invoke('install_modpack', { project: h.id });
        await refreshInstances();
        hideProgress();
        toast(`Created ${made.name}`);
        go('instance', { id: made.id });
        return;
      }
      let id = B.target;
      if (!id) { id = await pickInstance(`Install ${h.title}`); if (!id) return; }
      busy('Installing…');
      const files = await invoke('install_content', { source: h.source, project: h.id, kind, instanceId: id });
      toast(`${h.title} installed${files.length > 1 ? ` with ${files.length - 1} dependenc${files.length === 2 ? 'y' : 'ies'}` : ''}.`);
    } catch (e) { hideProgress(); fail(e); } finally { done(); }
  }


  // ---------- PROJECT PAGE ----------
  function openProject(h, tab) {
    S.proj = null; // always start fresh so the instance you came from sets the filters
    go('project', { source: h.source, id: h.id, kind: h.kind || S.browse.kind, title: h.title, icon: h.icon, tab: tab || 'description' });
  }

  function renderProject(view) {
    const v = S.view;
    const key = `${v.source}:${v.id}`;
    if (!S.proj || S.proj.key !== key) {
      const t = inst(S.browse.target);
      const useTarget = t && v.kind !== 'modpack';
      S.proj = {
        key, data: null, error: '', versions: null, vError: '', vLoading: false, bodyHtml: null,
        vf: { mc: useTarget ? t.mc_version : '', loader: useTarget && t.loader !== 'vanilla' ? t.loader : '', type: 'all', show: 25 },
        target: useTarget ? t.id : '',
      };
      loadProject(S.proj, v.source, v.id);
    }
    if (v.tab === 'versions' && S.proj.versions === null && !S.proj.vLoading) loadVersions(S.proj);
    paintProject(true);
  }

  async function loadProject(P, source, id) {
    try { P.data = await invoke('project_details', { source, id }); } catch (e) { P.error = errText(e); }
    if (S.proj === P && S.view.name === 'project') {
      $('#crumb').innerHTML = crumb();
      paintProject();
    }
  }

  async function loadVersions(P) {
    P.vLoading = true; P.vError = ''; P.versions = null;
    if (S.proj === P) paintProject();
    const v = S.view;
    try {
      const list = await invoke('project_versions', { source: v.source, project: v.id, kind: v.kind, mc: P.vf.mc, loader: P.vf.loader });
      if (S.proj !== P) return;
      P.versions = list.map((x, i) => ({ ...x, _i: i }));
    } catch (e) { if (S.proj !== P) return; P.vError = errText(e); P.versions = []; }
    P.vLoading = false;
    if (S.view.name === 'project') paintProject();
  }

  function versionsHtml(P, d, v) {
    const f = P.vf;
    const mcs = sortMc(d.game_versions || []);
    const loaders = d.loaders || [];
    const sel = (id, label, opts, cur) => `<div class="field"><label for="${id}">${label}</label><select id="${id}" class="input" data-vf="${id.slice(3)}">${opts}</select></div>`;
    const opt = (val, text, cur) => `<option value="${esc(val)}" ${cur === val ? 'selected' : ''}>${esc(text)}</option>`;
    const t = inst(P.target);
    const note = (f.mc || f.loader) && t
      ? `<div class="note">Showing versions that fit <b>${esc(t.name)}</b> (${esc(instLabel(t))}). <button class="btn sm ghost" data-act="ver-all">Show all versions</button></div>` : '';
    let body;
    if (P.vLoading || P.versions === null) body = '<div class="panel empty">Loading versions…</div>';
    else if (P.vError) body = `<div class="panel empty"><div class="big">Couldn't load versions</div><div class="selectable">${esc(P.vError)}</div></div>`;
    else {
      const list = P.versions.filter((x) => f.type === 'all' || x.kind === f.type);
      if (!list.length) body = '<div class="panel empty"><div class="big">No versions match</div><div>Try another game version or loader.</div></div>';
      else {
        const verb = v.kind === 'modpack' ? 'Create instance' : 'Install';
        body = list.slice(0, f.show).map((x) => {
          const gv = sortMc(x.game_versions || []);
          return `<div class="ver">
            <div class="ver-top">
              <span class="badge vt-${esc(x.kind)}">${esc(x.kind)}</span>
              <div class="grow" style="min-width:0"><b class="selectable">${esc(x.name || x.number)}</b> <span class="faint selectable">${esc(x.number && x.number !== x.name ? x.number : '')}</span>
                <div class="ver-meta">${gv.slice(0, 5).map((g) => `<span class="mini">${esc(g)}</span>`).join('')}${gv.length > 5 ? `<span class="faint">+${gv.length - 5}</span>` : ''}${(x.loaders || []).map((l) => `<span class="mini red">${esc(LOADER[l] || l)}</span>`).join('')}
                  <span class="faint">${ic('download', 12)} ${fmt(x.downloads)}</span><span class="faint">${esc(fmtDate(x.published))}</span></div></div>
              <button class="btn primary sm" data-act="ver-install" data-n="${x._i}">${ic('download', 15)} ${verb}</button>
            </div>
            ${x.changelog ? `<details class="ver-log"><summary>Changelog</summary><div class="md selectable">${SFMD.render(x.changelog, 'markdown')}</div></details>` : ''}
          </div>`;
        }).join('') + (list.length > f.show ? `<button class="btn" data-act="ver-more">Show more (${list.length - f.show})</button>` : '');
      }
    }
    return `<div class="panel">
      <div class="set-grid" style="grid-template-columns:repeat(3,1fr)">
        ${sel('vf-mc', 'Game version', opt('', 'All versions', f.mc) + mcs.map((g) => opt(g, g, f.mc)).join(''))}
        ${sel('vf-loader', 'Loader', opt('', 'All loaders', f.loader) + loaders.map((l) => opt(l, LOADER[l] || l, f.loader)).join(''))}
        ${sel('vf-type', 'Release type', ['all', 'release', 'beta', 'alpha'].map((x) => opt(x, x === 'all' ? 'All types' : x[0].toUpperCase() + x.slice(1), f.type)).join(''))}
      </div>${note}</div><div class="results" style="margin-top:12px">${body}</div>`;
  }

  function paintProject(resetScroll) {
    if (S.view.name !== 'project') return;
    const view = $('#view');
    const keep = view.scrollTop;
    const P = S.proj, v = S.view, tab = v.tab || 'description';
    if (P.error) {
      view.innerHTML = `<div class="panel empty"><div class="big">Couldn't load this project</div><div class="selectable">${esc(P.error)}</div><div style="margin-top:14px"><button class="btn" data-act="proj-retry">Try again</button></div></div>`;
      return;
    }
    const d = P.data;
    if (!d) {
      view.innerHTML = `<div class="pj-head glass">${projPic(v.icon, v.title || '?', 'lg')}<div class="grow"><h1>${esc(v.title || 'Loading…')}</h1><div class="muted">Loading…</div></div></div>`;
      return;
    }
    if (P.bodyHtml === null) P.bodyHtml = SFMD.render(d.body, d.body_format);
    const kind = v.kind;
    const cats = (d.categories || []).filter((c) => !(d.loaders || []).includes(c)).slice(0, 8);
    const gallery = d.gallery || [];
    const mcs = sortMc(d.game_versions || []);

    let main;
    if (tab === 'gallery') {
      main = gallery.length
        ? `<div class="gal">${gallery.map((g, n) => `<button class="shot" data-act="shot" data-n="${n}" aria-label="${esc(g.title || 'Screenshot ' + (n + 1))}">
            <img src="${esc(safeUrl(g.url) || safeUrl(g.full))}" alt="" loading="lazy" referrerpolicy="no-referrer">
            ${g.title || g.description ? `<span class="cap"><b>${esc(g.title || '')}</b>${g.description ? `<span>${esc(g.description)}</span>` : ''}</span>` : ''}</button>`).join('')}</div>`
        : `<div class="panel empty"><div class="big">No screenshots</div><div>The author hasn't added any images.</div></div>`;
    } else if (tab === 'versions') {
      main = versionsHtml(P, d, v);
    } else {
      main = `<div class="panel md selectable">${P.bodyHtml || '<p class="muted">The author hasn\'t written a description yet.</p>'}</div>`;
    }

    view.innerHTML = `
      <div class="pj-head glass">
        ${projPic(d.icon || v.icon, d.title, 'lg')}
        <div class="grow" style="min-width:0">
          <div class="row wrap" style="gap:8px"><h1>${esc(d.title)}</h1><span class="badge red">${esc(({ modpack: 'Modpack', mod: 'Mod', resourcepack: 'Resource pack', shader: 'Shader' })[kind] || kind)}</span><span class="badge">${d.source === 'curseforge' ? 'CurseForge' : 'Modrinth'}</span></div>
          <div class="muted" style="margin:6px 0 10px">${esc(d.summary)}</div>
          <div class="row wrap pj-stats">
            <span>${ic('download', 15)} ${fmt(d.downloads)} downloads</span>
            ${d.followers ? `<span>${ic('heart', 15)} ${fmt(d.followers)} ${d.source === 'curseforge' ? 'likes' : 'followers'}</span>` : ''}
            ${d.updated ? `<span>${ic('clock', 15)} Updated ${esc(fmtDate(d.updated))}</span>` : ''}
          </div>
          ${cats.length ? `<div class="row wrap" style="margin-top:10px;gap:6px">${cats.map((c) => `<span class="mini">${esc(c)}</span>`).join('')}</div>` : ''}
        </div>
        <div class="pj-actions">
          ${kind === 'modpack'
            ? `<button class="btn primary lg" data-act="proj-tab" data-tab="versions">${ic('download', 18)} Choose version</button>`
            : `<button class="btn primary lg" data-act="proj-install">${ic('download', 18)} Install latest</button>
               <button class="btn" data-act="proj-tab" data-tab="versions">Choose version…</button>`}
          ${safeUrl(d.web_url) ? `<button class="btn ghost sm" data-act="open-url" data-url="${esc(d.web_url)}">${ic('ext', 15)} Open website</button>` : ''}
        </div>
      </div>
      <div class="tabs" style="margin:18px auto">
        <button class="tab ${tab === 'description' ? 'active' : ''}" data-act="proj-tab" data-tab="description">Description</button>
        <button class="tab ${tab === 'gallery' ? 'active' : ''}" data-act="proj-tab" data-tab="gallery">Gallery${gallery.length ? ` <span class="badge">${gallery.length}</span>` : ''}</button>
        <button class="tab ${tab === 'versions' ? 'active' : ''}" data-act="proj-tab" data-tab="versions">Versions</button>
      </div>
      <div class="pj-grid">
        <div class="pj-main">${main}</div>
        <aside class="pj-side">
          <div class="panel"><div class="label">Compatibility</div>
            ${mcs.length ? `<div class="chips-sm">${mcs.slice(0, 14).map((g) => `<span class="mini">${esc(g)}</span>`).join('')}${mcs.length > 14 ? `<span class="faint">+${mcs.length - 14} more</span>` : ''}</div>` : '<div class="faint">Not listed</div>'}
            ${(d.loaders || []).length ? `<div class="chips-sm" style="margin-top:8px">${d.loaders.map((l) => `<span class="mini red">${esc(LOADER[l] || l)}</span>`).join('')}</div>` : ''}</div>
          ${(d.team || []).length ? `<div class="panel"><div class="label">Creators</div>${d.team.map((m) => `<div class="creator">${safeUrl(m.avatar) ? `<img src="${esc(m.avatar)}" alt="" loading="lazy" referrerpolicy="no-referrer">` : pic(m.name || '?', 'sm')}<div><b>${esc(m.name)}</b><div class="faint">${esc(m.role || '')}</div></div></div>`).join('')}</div>` : ''}
          ${(d.links || []).filter((l) => safeUrl(l.url)).length ? `<div class="panel"><div class="label">Links</div>${d.links.filter((l) => safeUrl(l.url)).map((l) => `<button class="linkrow" data-act="open-url" data-url="${esc(l.url)}">${ic('ext', 14)} ${esc(l.label)}</button>`).join('')}</div>` : ''}
          <div class="panel"><div class="label">Details</div>
            ${d.license ? `<div class="kv"><span>Licence</span><b>${esc(d.license)}</b></div>` : ''}
            ${d.published ? `<div class="kv"><span>Published</span><b>${esc(fmtDate(d.published))}</b></div>` : ''}
            ${d.updated ? `<div class="kv"><span>Updated</span><b>${esc(fmtDate(d.updated))}</b></div>` : ''}</div>
        </aside>
      </div>`;
    view.scrollTop = resetScroll ? 0 : keep;
  }

  async function installVersion(n, btn) {
    const P = S.proj, v = S.view, d = P.data;
    const ver = P.versions.find((x) => x._i === n);
    if (!ver) return;
    const orig = btn.innerHTML;
    const busy = () => { btn.disabled = true; btn.innerHTML = 'Installing…'; };
    const done = () => { btn.disabled = false; btn.innerHTML = orig; };
    try {
      if (v.kind === 'modpack') {
        if (v.source !== 'modrinth') { toast('CurseForge modpacks are not supported yet. Pick the Modrinth version of the pack.', 'error'); return; }
        busy();
        const made = await invoke('install_modpack_version', { versionId: ver.id });
        await refreshInstances();
        hideProgress();
        toast(`Created ${made.name} from ${ver.name || ver.number}`);
        go('instance', { id: made.id });
        return;
      }
      let id = P.target;
      if (!id) { id = await pickInstance(`Install ${d.title} ${ver.number || ''}`); if (!id) return; }
      const i = inst(id);
      const mcOk = !(ver.game_versions || []).length || ver.game_versions.includes(i.mc_version);
      const lds = ver.loaders || [];
      const loaderOk = v.kind !== 'mod' || !lds.length || i.loader === 'vanilla' || lds.includes(i.loader) || (i.loader === 'quilt' && lds.includes('fabric'));
      if (!mcOk || !loaderOk) {
        const ok = await confirmBox('This version may not work',
          `${ver.name || ver.number} is for ${(ver.game_versions || []).slice(0, 4).join(', ') || 'other versions'}${lds.length ? ' (' + lds.map((l) => LOADER[l] || l).join(', ') + ')' : ''}, but "${i.name}" is ${instLabel(i)}.`, 'Install anyway');
        if (!ok) return;
      }
      busy();
      const files = await invoke('install_version_content', { source: v.source, project: v.id, versionId: ver.id, kind: v.kind, instanceId: id });
      toast(`${d.title} ${ver.number || ''} installed into ${i.name}${files.length > 1 ? ` with ${files.length - 1} dependenc${files.length === 2 ? 'y' : 'ies'}` : ''}.`);
    } catch (e) { hideProgress(); fail(e); } finally { done(); }
  }

  function confirmBox(title, msg, okLabel) {
    return new Promise((resolve) => {
      let yes = false;
      const m = openModal(`<h2>${esc(title)}</h2><div class="muted" style="margin-top:8px">${esc(msg)}</div>
        <div class="foot"><button class="btn" data-act="close-modal">Cancel</button><button class="btn primary" id="cf-ok">${esc(okLabel)}</button></div>`,
        { onClose: () => resolve(yes), noFocus: true });
      $('#cf-ok', m).addEventListener('click', () => { yes = true; closeModal(); });
    });
  }

  // ---------- lightbox ----------
  function paintLightbox() {
    let el = $('#lightbox');
    if (!S.lb) { if (el) el.remove(); return; }
    if (!el) {
      el = document.createElement('div');
      el.id = 'lightbox';
      document.body.appendChild(el);
      el.addEventListener('click', (e) => {
        const b = e.target.closest('[data-lb]');
        if (b) lbStep(+b.dataset.lb);
        else if (e.target === el || e.target.closest('[data-lbclose]')) closeLightbox();
      });
    }
    const { n, list } = S.lb;
    const g = list[n];
    el.innerHTML = `<button class="icon-btn lb-x" data-lbclose="1" aria-label="Close">${ic('x', 22)}</button>
      ${list.length > 1 ? `<button class="icon-btn lb-nav prev" data-lb="-1" aria-label="Previous">${ic('left', 26)}</button><button class="icon-btn lb-nav next" data-lb="1" aria-label="Next">${ic('right', 26)}</button>` : ''}
      <figure><img src="${esc(safeUrl(g.full) || safeUrl(g.url))}" alt="" referrerpolicy="no-referrer">
        <figcaption>${g.title ? `<b>${esc(g.title)}</b>` : ''}${g.description ? `<span>${esc(g.description)}</span>` : ''}<em>${n + 1} / ${list.length}</em></figcaption></figure>`;
  }
  function lbStep(dir) { if (!S.lb) return; const t = S.lb.list.length; S.lb.n = (S.lb.n + dir + t) % t; paintLightbox(); }
  function closeLightbox() { S.lb = null; paintLightbox(); }

  // ---------- SETTINGS ----------
  function renderSettings(view) {
    const s = S.settings;
    const sw = (label, key, desc) => `<div class="set-row"><div><b>${label}</b><div class="desc">${desc}</div></div><label class="switch"><input type="checkbox" data-set="${key}" ${s[key] ? 'checked' : ''}><i></i></label></div>`;
    view.innerHTML = `
      <h1 class="page-title">Settings</h1>
      <div class="panel">
        <div class="section-title" style="margin-top:0">Appearance</div>
        <div class="label" style="margin-bottom:8px">Surface style</div>
        <div class="chips surf-row" style="gap:12px;margin-bottom:18px">
          ${[['aero', 'Aero', 'Glass with a bright edge'], ['frosted', 'Frosted', 'Soft, matte blur'], ['solid', 'Solid', 'No blur, lightest']].map(([id, n, d]) => `<button class="surf-card ${(s.surface || 'aero') === id ? 'active' : ''}" data-act="surface" data-sf="${id}"><i class="sf-${id}"></i><b>${n}</b><small>${d}</small></button>`).join('')}
        </div>
        <div class="label" style="margin-bottom:8px">Colour mode</div>
        <div class="chips" style="gap:12px;margin-bottom:16px">
          <button class="theme-card ${s.theme === 'oled' ? 'active' : ''}" data-act="theme" data-t="oled"><i style="background:#000"></i><b>OLED</b></button>
          <button class="theme-card ${s.theme === 'dark' ? 'active' : ''}" data-act="theme" data-t="dark"><i style="background:#1a1214"></i><b>Dark</b></button>
        </div>
        <div class="set-grid">
          <div class="field"><label>Accent colour</label>
            <div class="swatches">${ACCENTS.map((c) => `<button class="swatch ${s.accent.toLowerCase() === c ? 'active' : ''}" data-act="accent" data-c="${c}" style="background:${c}" aria-label="${c}"></button>`).join('')}<input type="color" id="accent-custom" value="${esc(s.accent)}" aria-label="Custom colour"></div></div>
          <div class="field"><label>Glass opacity <span id="v-glass" style="text-transform:none;letter-spacing:0">${s.glass}%</span></label><input type="range" min="0" max="85" value="${s.glass}" data-slide="glass"></div>
          <div class="field"><label>Background blur <span id="v-bg_blur" style="text-transform:none;letter-spacing:0">${s.bg_blur}px</span></label><input type="range" min="0" max="30" value="${s.bg_blur}" data-slide="bg_blur"></div>
          <div class="field"><label>Background dimming <span id="v-bg_dim" style="text-transform:none;letter-spacing:0">${s.bg_dim}%</span></label><input type="range" min="0" max="80" value="${s.bg_dim}" data-slide="bg_dim"></div>
        </div>
        <div class="bg-row">
          <div class="bg-preview" style="background-image:url('${esc(S.bgUrl)}')"></div>
          <div class="grow">
            <b>${s.background ? 'Your own background' : 'Default background'}</b>
            <div class="muted" style="margin:2px 0 12px">${s.background ? 'Using the picture you chose.' : 'The built-in lavender fields. Use any image you like instead.'}<br>Tip: you can also drop an image anywhere on the window, or use the picture button in the top bar.</div>
            <div class="row wrap">
              <button class="btn primary" data-act="pick-bg">${ic('image', 18)} Choose picture…</button>
              <button class="btn ghost" data-act="reset-bg" ${s.background ? '' : 'disabled'}>${ic('reset', 18)} Use default</button>
            </div>
          </div>
        </div>
      </div>
      <div class="panel">
        <div class="section-title" style="margin-top:0">Game</div>
        <div class="set-grid">
          <div class="field"><label>Default memory <span id="v-default_memory_mb" style="text-transform:none;letter-spacing:0">${(s.default_memory_mb / 1024).toFixed(1)} GB</span></label>
            <input type="range" min="1024" max="16384" step="256" value="${s.default_memory_mb}" data-slide="default_memory_mb">
            <span class="hint">2 GB is plenty for most light setups. Lower it to keep RAM free.</span></div>
          <div class="field"><label for="s-java">Java path (optional)</label><input id="s-java" class="input" data-text="java_path" value="${esc(s.java_path)}" placeholder="Automatic: the launcher downloads the right Java"></div>
        </div>
        ${sw('Hide launcher while playing', 'hide_on_launch', 'Frees up screen space; the launcher comes back when the game closes.')}
      </div>
      <div class="panel">
        <div class="section-title" style="margin-top:0">Accounts &amp; keys</div>
        <div class="row wrap" style="margin-bottom:14px">
          <button class="btn" data-act="accounts">${ic('user', 18)} Manage accounts</button>
          <span class="muted">${S.acct.accounts.length ? `Playing as <b>${esc(acctName())}</b>` : 'No account yet'}</span>
        </div>
        <div class="set-grid">
          <div class="field"><label for="s-cf">CurseForge API key</label><input id="s-cf" class="input" type="password" data-text="curseforge_key" value="${esc(s.curseforge_key)}" placeholder="Needed to browse CurseForge" autocomplete="off">
            <span class="hint">Free key from console.curseforge.com</span></div>
          <div class="field"><label for="s-ms">Microsoft app ID (advanced)</label><input id="s-ms" class="input" data-text="ms_client_id" value="${esc(s.ms_client_id)}" placeholder="Optional if one is built in" autocomplete="off">
            <span class="hint">Used for Microsoft sign-in. You can also set it from the sign-in window.</span></div>
        </div>
      </div>
      <div class="panel row"><img src="logo.png" width="46" height="46" alt=""><div class="grow"><b>Spiderflayer Launcher</b><div class="muted">v0.1.0 · lightweight Minecraft launcher</div></div></div>`;

    $$('[data-slide]', view).forEach((el) => el.addEventListener('input', () => {
      const k = el.dataset.slide;
      s[k] = +el.value;
      const l = $(`#v-${k}`);
      l.textContent = k === 'default_memory_mb' ? (s[k] / 1024).toFixed(1) + ' GB' : k === 'bg_blur' ? s[k] + 'px' : k === 'glass' || k === 'bg_dim' ? s[k] + '%' : s[k];
      applyTheme(); saveSettings();
    }));
    $$('[data-text]', view).forEach((el) => el.addEventListener('input', () => { s[el.dataset.text] = el.value; saveSettings(); }));
    $$('[data-set]', view).forEach((el) => el.addEventListener('change', () => { s[el.dataset.set] = el.checked; saveSettings(); }));
    $('#accent-custom').addEventListener('input', (e) => { s.accent = e.target.value; applyTheme(); saveSettings(); });
  }

  // ---------- background picking ----------
  async function setBackgroundFile(f) {
    if (!f) return;
    if (!/^image\//.test(f.type)) { toast('Pick an image file (PNG, JPG, WebP or GIF).', 'error'); return; }
    if (f.size > 40 * 1024 * 1024) { toast('That image is too large (40 MB max).', 'error'); return; }
    let ext = (f.name.split('.').pop() || '').toLowerCase();
    if (!['png', 'jpg', 'jpeg', 'webp', 'gif'].includes(ext)) ext = ({ 'image/png': 'png', 'image/webp': 'webp', 'image/gif': 'gif' })[f.type] || 'jpg';
    try {
      await invoke('set_background', { ext, data: await fileB64(f) });
      S.settings = await invoke('get_settings');
      await loadBackground();
      toast('Background updated.');
      if (S.view.name === 'settings') render();
    } catch (e) { fail(e); }
  }

  async function importPack(f) {
    try {
      const data = await fileB64(f);
      const made = await invoke('import_mrpack', { data });
      await refreshInstances();
      hideProgress();
      toast(`Imported ${made.name}`);
      go('instance', { id: made.id });
    } catch (err) { hideProgress(); fail(err); }
  }

  // ---------- accounts ----------
  function accountsHtml(extra = '') {
    const list = S.acct.accounts;
    return `<h2>Accounts</h2><div class="muted">Play with a Microsoft account, or offline with just a name.</div>
      <div class="body">
        ${list.length ? list.map((a) => `
          <div class="acct ${a.id === S.acct.selected ? 'sel' : ''}">
            <span class="avatar" style="width:40px;height:40px;position:relative">${esc(a.name[0].toUpperCase())}<img src="https://minotar.net/helm/${encodeURIComponent(a.name)}/64.png" alt="" style="position:absolute;inset:0;width:40px;height:40px" onerror="this.remove()"></span>
            <div class="grow"><b>${esc(a.name)}</b><div class="muted">${a.kind === 'microsoft' ? 'Microsoft account' : 'Offline account'}</div></div>
            ${a.id === S.acct.selected ? '<span class="badge red">Active</span>' : `<button class="btn sm" data-act="acct-select" data-id="${esc(a.id)}">Use</button>`}
            <button class="icon-btn" data-act="acct-remove" data-id="${esc(a.id)}" aria-label="Remove account">${ic('trash', 17)}</button>
          </div>`).join('') : '<div class="muted">No accounts yet.</div>'}
        ${extra || `
        <div class="field"><button class="btn primary lg" data-act="ms-login">${ic('link', 18)} Sign in with Microsoft</button>
          <span class="hint">Opens Microsoft's sign-in page. Approve it and you're in. <button class="linklike" data-act="ms-login-app">Use a window inside the launcher</button></span></div>
        <div class="field"><label for="off-name">Offline account</label>
          <div class="row"><input id="off-name" class="input" placeholder="Username" maxlength="16" autocomplete="off"><button class="btn" data-act="acct-offline">Add</button></div></div>`}
      </div>
      <div class="foot"><button class="btn" data-act="close-modal">Done</button></div>`;
  }
  async function openAccounts() {
    try { S.msReady = await invoke('ms_configured'); } catch (e) { S.msReady = false; }
    openModal(accountsHtml(), { noFocus: !!S.acct.accounts.length });
  }
  function refreshAccountsModal(extra) { const m = $('#modal-root .modal'); if (m) m.innerHTML = accountsHtml(extra); }

  function msSetupHtml(inApp) {
    return `<div class="field"><label for="ms-id">Microsoft app ID</label>
      <input id="ms-id" class="input" placeholder="00000000-0000-0000-0000-000000000000" autocomplete="off" spellcheck="false">
      <span class="hint">One-time setup. Paste the ID of your Microsoft app and you're signed in right away. Mojang has to approve the app before premium accounts work.</span>
      <div class="row wrap"><button class="btn primary" data-act="ms-save" data-app="${inApp ? 1 : 0}">Save and sign in</button>
        <button class="btn" data-act="open-url" data-url="https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/CreateApplicationBlade/isMSAApp~/true">${ic('ext', 15)} Create an app ID</button>
        <button class="btn ghost" data-act="ms-setup-cancel">Back</button></div></div>`;
  }

  async function msLogin(inApp) {
    refreshAccountsModal(`<div class="field"><div class="row"><span class="spinner"></span><b>Waiting for Microsoft…</b></div>
      <span class="hint">${inApp ? 'Sign in in the window that just opened.' : 'Finish signing in in your browser, then come back here.'} This closes by itself when you're done.</span>
      <div class="row"><button class="btn" data-act="ms-cancel">Cancel</button></div></div>`);
    try {
      S.acct = await invoke('ms_login', { inApp });
      renderRail();
      refreshAccountsModal();
      toast(`Signed in as ${acctName()}.`);
      if (S.view.name === 'home') render();
    } catch (e) {
      refreshAccountsModal();
      if (!/cancel/i.test(errText(e))) fail(e);
    }
  }

  // ---------- create instance ----------
  async function openCreate() {
    const m = openModal(`<h2>New instance</h2><div class="muted">Pick a Minecraft version and, optionally, a mod loader.</div>
      <div class="body">
        <div class="field"><label for="c-name">Name</label><input id="c-name" class="input" placeholder="My instance" maxlength="60" autocomplete="off"></div>
        <div class="set-grid">
          <div class="field"><label for="c-mc">Game version</label><select id="c-mc" class="input"><option>Loading…</option></select></div>
          <div class="field"><label for="c-loader">Loader</label><select id="c-loader" class="input">${Object.entries(LOADER).map(([k, v]) => `<option value="${k}">${v}</option>`).join('')}</select></div>
        </div>
        <label class="row" style="gap:8px"><input type="checkbox" id="c-snap"> <span class="muted">Show snapshots and old versions</span></label>
        <div class="field" id="c-lv-wrap" hidden><label for="c-lv">Loader version</label><select id="c-lv" class="input"></select><span class="hint" id="c-lv-hint"></span></div>
      </div>
      <div class="foot"><button class="btn" data-act="close-modal">Cancel</button><button class="btn primary" id="c-go" disabled>Create</button></div>`);
    const mcSel = $('#c-mc', m), snap = $('#c-snap', m), loaderSel = $('#c-loader', m), lvWrap = $('#c-lv-wrap', m), lvSel = $('#c-lv', m), lvHint = $('#c-lv-hint', m), go_ = $('#c-go', m);
    let versions = [];
    const fillMc = () => {
      const keep = mcSel.value;
      const list = versions.filter((v) => snap.checked || v.type === 'release');
      mcSel.innerHTML = list.map((v) => `<option>${esc(v.id)}</option>`).join('');
      if (list.some((v) => v.id === keep)) mcSel.value = keep;
    };
    const fillLoader = async () => {
      const l = loaderSel.value;
      lvWrap.hidden = l === 'vanilla';
      go_.disabled = !mcSel.value || mcSel.value === 'Loading…';
      if (l === 'vanilla') return;
      lvSel.innerHTML = '<option value="">Loading…</option>';
      lvHint.textContent = '';
      go_.disabled = true;
      const want = `${l}|${mcSel.value}`;
      try {
        const vs = await invoke('loader_versions', { loader: l, mc: mcSel.value });
        if (`${loaderSel.value}|${mcSel.value}` !== want) return;
        if (!vs.length) { lvSel.innerHTML = '<option value="">None available</option>'; lvHint.textContent = `${LOADER[l]} doesn't support ${mcSel.value} yet.`; return; }
        lvSel.innerHTML = vs.slice(0, 40).map((v, n) => `<option value="${esc(v)}">${esc(v)}${n === 0 ? ' (latest)' : ''}</option>`).join('');
        go_.disabled = false;
      } catch (e) { lvSel.innerHTML = '<option value="">Failed to load</option>'; lvHint.textContent = String(e); }
    };
    snap.addEventListener('change', () => { fillMc(); fillLoader(); });
    mcSel.addEventListener('change', fillLoader);
    loaderSel.addEventListener('change', fillLoader);
    go_.addEventListener('click', async () => {
      const mc = mcSel.value, loader = loaderSel.value;
      const name = $('#c-name', m).value.trim() || (loader === 'vanilla' ? `Minecraft ${mc}` : `${LOADER[loader]} ${mc}`);
      go_.disabled = true;
      try {
        const made = await invoke('create_instance', { name, mc, loader, loaderVersion: loader === 'vanilla' ? '' : lvSel.value });
        await refreshInstances();
        closeModal();
        go('instance', { id: made.id });
      } catch (e) { go_.disabled = false; fail(e); }
    });
    try {
      versions = await ensureVersions();
      fillMc();
      await fillLoader();
    } catch (e) { mcSel.innerHTML = '<option value="">Could not load versions</option>'; fail(e); }
  }

  // ---------- actions (event delegation) ----------
  const A = {
    nav: (el) => { const to = el.dataset.to; S.hist = []; S.view = { name: to }; render(); },
    'open-inst': (el) => go('instance', { id: el.dataset.id }),
    play: (el, e) => { e.stopPropagation(); play(el.dataset.id); },
    create: () => openCreate(),
    accounts: () => openAccounts(),
    'close-modal': () => closeModal(),
    'open-url': (el) => { if (el.dataset.url) invoke('open_url', { url: el.dataset.url }).catch(fail); },
    'lib-filter': (el) => { S.view.filter = el.dataset.f; render(); },
    'import-pack': () => $('#mrpack-file').click(),
    'open-folder': (el) => invoke('open_instance_folder', { id: el.dataset.id }).catch(fail),
    'add-content': (el) => { S.browse.target = el.dataset.id; S.browse.kind = S.browse.kind === 'modpack' ? 'mod' : S.browse.kind; S.browse.hits = []; go('browse'); },
    'inst-tab': (el) => { S.view.tab = el.dataset.tab; render(); },
    'content-kind': (el) => { S.content.kind = el.dataset.k; render(); },
    'toggle-file': async (el) => {
      const f = S.content.files[+el.dataset.n];
      try { await invoke('toggle_content', { id: S.view.id, kind: S.content.kind, name: f.name, enabled: el.checked }); await loadContent(inst(S.view.id)); } catch (e) { fail(e); }
    },
    'remove-file': async (el) => {
      const f = S.content.files[+el.dataset.n];
      try { await invoke('remove_content', { id: S.view.id, kind: S.content.kind, name: f.name }); await loadContent(inst(S.view.id)); } catch (e) { fail(e); }
    },
    'save-inst': async (el) => {
      const i = { ...inst(el.dataset.id), name: $('#o-name').value.trim() || 'Instance', memory_mb: +$('#o-mem').value, java_args: $('#o-args').value.trim() };
      try { await invoke('update_instance', { inst: i }); await refreshInstances(); toast('Saved.'); render(); } catch (e) { fail(e); }
    },
    'delete-inst': (el) => {
      const i = inst(el.dataset.id);
      const m = openModal(`<h2>Delete ${esc(i.name)}?</h2><div class="muted">This permanently removes its worlds, mods and settings.</div>
        <div class="foot"><button class="btn" data-act="close-modal">Cancel</button><button class="btn danger" id="del-go">${ic('trash', 18)} Delete</button></div>`);
      $('#del-go', m).addEventListener('click', async () => {
        try { await invoke('delete_instance', { id: i.id }); await refreshInstances(); closeModal(); S.hist = []; S.view = { name: 'library' }; render(); } catch (e) { fail(e); }
      });
    },
    'browse-kind': (el) => { S.browse.kind = el.dataset.k; S.browse.hits = []; if (S.browse.kind === 'modpack') S.browse.target = ''; render(); },
    install: (el) => installProject(S.browse.hits[+el.dataset.n], el),
    more: () => runSearch(false),
    surface: (el) => { S.settings.surface = el.dataset.sf; applyTheme(); saveSettings(); render(); },
    theme: (el) => { S.settings.theme = el.dataset.t; applyTheme(); saveSettings(); render(); },
    accent: (el) => { S.settings.accent = el.dataset.c; applyTheme(); saveSettings(); render(); },
    'pick-bg': () => $('#bg-file').click(),
    'pick-bg-top': () => $('#bg-file').click(),
    'reset-bg': async () => { try { await invoke('clear_background'); S.settings = await invoke('get_settings'); await loadBackground(); render(); } catch (e) { fail(e); } },
    'acct-select': async (el) => { try { S.acct = await invoke('select_account', { id: el.dataset.id }); refreshAccountsModal(); renderRail(); } catch (e) { fail(e); } },
    'acct-remove': async (el) => { try { S.acct = await invoke('remove_account', { id: el.dataset.id }); refreshAccountsModal(); renderRail(); } catch (e) { fail(e); } },
    'acct-offline': async () => {
      try { S.acct = await invoke('add_offline', { name: $('#off-name').value }); refreshAccountsModal(); renderRail(); if (S.view.name === 'home') render(); } catch (e) { fail(e); }
    },
    'ms-login': () => (S.msReady ? msLogin(false) : refreshAccountsModal(msSetupHtml(false))),
    'ms-login-app': () => (S.msReady ? msLogin(true) : refreshAccountsModal(msSetupHtml(true))),
    'ms-setup-cancel': () => refreshAccountsModal(),
    'ms-save': async (el) => {
      const v = ($('#ms-id').value || '').trim();
      if (!/^[0-9a-fA-F-]{30,40}$/.test(v)) { toast("That doesn't look like an app ID. It's a long code with dashes.", 'error'); return; }
      S.settings.ms_client_id = v;
      try { await invoke('save_settings', { settings: S.settings }); S.msReady = true; } catch (e) { fail(e); return; }
      msLogin(el.dataset.app === '1');
    },
    'ms-cancel': () => invoke('ms_cancel').catch(() => {}),
    'goto-settings': () => { closeModal(); S.hist = []; S.view = { name: 'settings' }; render(); },
    'open-project': (el) => {
      if (el.dataset.n !== undefined) { openProject(S.browse.hits[+el.dataset.n]); return; }
      openProject({ source: el.dataset.source, id: el.dataset.pid, kind: el.dataset.kind, title: el.dataset.title, icon: el.dataset.icon });
    },
    'open-versions': (el) => openProject(S.browse.hits[+el.dataset.n], 'versions'),
    'proj-tab': (el) => {
      S.view.tab = el.dataset.tab;
      if (S.view.tab === 'versions' && S.proj.versions === null && !S.proj.vLoading) loadVersions(S.proj);
      paintProject(true);
    },
    'proj-retry': () => { const v = S.view; S.proj = null; renderProject($('#view')); },
    'proj-install': (el) => installProject({ source: S.view.source, id: S.view.id, title: S.proj.data.title, kind: S.view.kind }, el),
    'ver-install': (el) => installVersion(+el.dataset.n, el),
    'ver-more': () => { S.proj.vf.show += 25; paintProject(); },
    'ver-all': () => { S.proj.vf.mc = ''; S.proj.vf.loader = ''; S.proj.target = ''; loadVersions(S.proj); },
    shot: (el) => { S.lb = { n: +el.dataset.n, list: S.proj.data.gallery }; paintLightbox(); },
  };

  document.addEventListener('click', (e) => {
    const el = e.target.closest('[data-act]');
    if (!el || el.tagName === 'INPUT') return;
    if (el.tagName === 'A') e.preventDefault();
    const fn = A[el.dataset.act];
    if (fn) fn(el, e);
  });
  document.addEventListener('change', (e) => {
    const el = e.target;
    if (el.tagName === 'INPUT' && el.dataset.act && A[el.dataset.act]) A[el.dataset.act](el, e);
    if (el.tagName === 'SELECT' && el.dataset.vf && S.proj) {
      const k = el.dataset.vf;
      S.proj.vf[k] = el.value;
      if (k === 'mc' || k === 'loader') { S.proj.target = ''; loadVersions(S.proj); } else paintProject();
    }
  });
  document.addEventListener('keydown', (e) => {
    if (e.key === 'Enter' && e.target.matches('.card[data-act], #off-name')) {
      if (e.target.id === 'off-name') A['acct-offline']();
      else e.target.click();
    }
  });

  // ---------- window chrome ----------
  function setupChrome() {
    $('#back').innerHTML = ic('left', 20);
    $('#bgbtn').innerHTML = ic('image', 19);
    $('#bgbtn').addEventListener('click', () => $('#bg-file').click());
    $('#bg-file').addEventListener('change', (e) => { const f = e.target.files[0]; e.target.value = ''; setBackgroundFile(f); });
    let depth = 0;
    const hasFiles = (e) => e.dataTransfer && Array.from(e.dataTransfer.types || []).includes('Files');
    window.addEventListener('dragenter', (e) => { if (!hasFiles(e)) return; e.preventDefault(); depth++; $('#drop').hidden = false; });
    window.addEventListener('dragover', (e) => { if (hasFiles(e)) e.preventDefault(); });
    window.addEventListener('dragleave', (e) => { if (!hasFiles(e)) return; depth = Math.max(0, depth - 1); if (!depth) $('#drop').hidden = true; });
    window.addEventListener('drop', (e) => {
      if (!hasFiles(e)) return;
      e.preventDefault(); depth = 0; $('#drop').hidden = true;
      const f = e.dataTransfer.files[0];
      if (!f) return;
      if (/\.mrpack$/i.test(f.name)) importPack(f); else setBackgroundFile(f);
    });
    $('#back').addEventListener('click', back);
    const win = TAURI.window && TAURI.window.getCurrentWindow ? TAURI.window.getCurrentWindow() : null;
    $('#wmin').addEventListener('click', () => win && win.minimize());
    $('#wmax').addEventListener('click', () => win && win.toggleMaximize());
    $('#wclose').addEventListener('click', () => win && win.close());
    $('#top').addEventListener('dblclick', (e) => { if (e.target.closest('button')) return; if (win) win.toggleMaximize(); });
  }

  // ---------- boot ----------
  async function boot() {
    setupChrome();
    try {
      S.settings = await invoke('get_settings');
      applyTheme();
      loadBackground();
      await Promise.all([refreshInstances(), refreshAccounts()]);
      const run = await invoke('running');
      S.running = new Set(run);
      S.msReady = await invoke('ms_configured').catch(() => false);
    } catch (e) { fail(e); }
    render();

    listen('progress', (ev) => showProgress(ev.payload));
    listen('log', (ev) => {
      const { id, line } = ev.payload;
      const arr = (S.logs[id] = S.logs[id] || []);
      arr.push(line);
      if (arr.length > 3000) arr.splice(0, arr.length - 3000);
      const box = $('#logbox');
      if (box && S.view.name === 'instance' && S.view.id === id) {
        const stick = box.scrollTop + box.clientHeight >= box.scrollHeight - 30;
        if (arr.length === 1) box.textContent = '';
        box.appendChild(document.createTextNode(line + '\n'));
        if (stick) box.scrollTop = box.scrollHeight;
      }
    });
    listen('game-started', (ev) => { S.running.add(ev.payload.id); S.logs[ev.payload.id] = []; hideProgress(); updatePlay(); });
    listen('game-exit', (ev) => {
      S.running.delete(ev.payload.id);
      updatePlay();
      const code = ev.payload.code;
      const i = inst(ev.payload.id);
      if (code && code !== 0) toast(`${i ? i.name : 'Minecraft'} closed unexpectedly (exit code ${code}). Check the Logs tab.`, 'error');
    });

    if (!S.acct.accounts.length) openAccounts();
  }

  boot();
})();
