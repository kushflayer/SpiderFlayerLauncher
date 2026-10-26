// Test-only stand-in for the Tauri backend. Not shipped with the app.
(() => {
  const listeners = {};
  let settings = { curseforge_key: '', ms_client_id: '', java_path: '', default_memory_mb: 2048, background: null, bg_blur: 0, bg_dim: 25, glass: 60, accent: '#ff3b4a', theme: 'oled', hide_on_launch: true };
  let instances = [
    { id: 'fabric-1-21-1', name: 'Vanilla+ Fabric', mc_version: '1.21.1', loader: 'fabric', loader_version: '0.16.9', version_id: 'x', memory_mb: 0, java_args: '', created: 1, last_played: 1700000000 },
    { id: 'forge-1-20-1', name: 'Create: Above and Beyond', mc_version: '1.20.1', loader: 'forge', loader_version: '47.3.0', version_id: 'y', memory_mb: 4096, java_args: '', created: 2, last_played: 1699000000 },
    { id: 'plain', name: 'Plain 1.8.9', mc_version: '1.8.9', loader: 'vanilla', loader_version: '', version_id: 'z', memory_mb: 0, java_args: '', created: 3, last_played: 0 },
  ];
  let acct = { accounts: [], selected: '' };
  if (!location.search.includes('noacct')) acct = { accounts: [{ id: 'u1', name: 'SpiderSlayer', kind: 'offline' }, { id: 'u2', name: 'Notch', kind: 'microsoft' }], selected: 'u1' };
  const hit = (n, t, d, dl, a, kind, src) => ({ source: src || 'modrinth', id: 'p' + n, title: t, description: d, icon: null, downloads: dl, author: a, kind });
  const packs = [
    hit(1, 'Fabulously Optimized', 'Improve your graphics and performance with this carefully curated modpack.', 18400000, 'robotkoer', 'modpack'),
    hit(2, 'Cobblemon', 'Catch and battle Pokémon in a Minecraft-native modpack.', 9200000, 'Cobblemon', 'modpack'),
    hit(3, 'Better MC [FORGE]', 'Expands vanilla without losing its charm.', 7100000, 'LunaPixelStudios', 'modpack'),
    hit(4, 'Simply Optimized', 'Maximum FPS with minimal changes.', 3200000, 'Surge', 'modpack'),
  ];
  const mods = [
    hit(11, 'Sodium', 'The fastest and most compatible rendering optimization mod for Minecraft.', 62000000, 'jellysquid3', 'mod'),
    hit(12, 'Iris Shaders', 'A modern shaders mod compatible with existing OptiFine shader packs.', 41000000, 'coderbot', 'mod'),
    hit(13, 'Lithium', 'No-compromises game logic and server optimization mod.', 38000000, 'jellysquid3', 'mod'),
    hit(14, 'Fabric API', 'Lightweight and modular API providing common hooks and intercompatibility.', 120000000, 'modmuss50', 'mod'),
  ];
  const versions = ['1.21.4', '1.21.3', '1.21.1', '1.21', '1.20.6', '1.20.4', '1.20.1', '1.19.4', '1.18.2', '1.16.5', '1.12.2', '1.8.9'].map((id) => ({ id, type: 'release', released: '' }))
    .concat([{ id: '25w10a', type: 'snapshot', released: '' }]);
  const files = { mods: [{ name: 'sodium-fabric-0.6.0.jar', enabled: true, size: 1200000 }, { name: 'lithium-fabric-0.14.jar', enabled: true, size: 600000 }, { name: 'iris-1.8.jar.disabled', enabled: false, size: 2500000 }], resourcepacks: [], shaderpacks: [] };
  const IMG = (n) => `https://cdn.modrinth.com/data/mock/images/${n}.png`;
  const BODY = [
    '# Sodium', '',
    '<p align="center"><img src="https://cdn.modrinth.com/data/mock/banner.png" width="420" alt="banner"></p>', '',
    'A **fast** rendering engine with _smart_ culling. See the [docs](https://example.com/docs) or https://example.com/bare for more.', '',
    '## Features', '- Faster chunk rendering', '- Lower memory use', '  - nested child item', '- `config.json` is simple', '',
    '> Works great with Iris.', '',
    '| Mod | Role |', '|---|---|', '| Sodium | Renderer |', '| Lithium | Logic |', '',
    '```', 'java -jar server.jar', '```', '',
    '## Hostile content below (must all be neutralised)', '',
    '<script>window.__pwned=1</script>',
    '<img src="https://cdn.modrinth.com/x.png" onerror="window.__pwned=2">',
    '[bad link](javascript:window.__pwned=3)',
    '<a href="javascript:window.__pwned=4">bad two</a>',
    '<iframe src="https://evil.example"></iframe>',
    '<svg onload="window.__pwned=5"></svg>',
    '<div style="position:fixed;inset:0;background:red" onclick="window.__pwned=6">overlay</div>',
    '<img src="http://insecure.example/a.png">',
  ].join('\n');
  const mkVersions = (mc) => [
    { id: 'v5', name: 'Release 0.6.1', number: '0.6.1+mc1.21.1', kind: 'release', game_versions: ['1.21.1', '1.21'], loaders: ['fabric', 'quilt'], downloads: 900000, published: '2026-09-01T00:00:00Z', changelog: '## Fixes\n- Fixed a crash\n- Faster **meshing**', files: [{ name: 'sodium-0.6.1.jar', size: 1200000 }] },
    { id: 'v4', name: 'Release 0.6.0', number: '0.6.0+mc1.21.1', kind: 'release', game_versions: ['1.21.1'], loaders: ['fabric'], downloads: 800000, published: '2026-08-01T00:00:00Z', changelog: '', files: [{ name: 'sodium-0.6.0.jar', size: 1100000 }] },
    { id: 'v3', name: 'Beta 0.5.9', number: '0.5.9+mc1.20.1', kind: 'beta', game_versions: ['1.20.1'], loaders: ['fabric'], downloads: 500000, published: '2026-01-01T00:00:00Z', changelog: 'beta notes', files: [{ name: 'sodium-0.5.9.jar', size: 900000 }] },
    { id: 'v2', name: 'Alpha 0.5.0', number: '0.5.0+mc1.19.4', kind: 'alpha', game_versions: ['1.19.4'], loaders: ['forge'], downloads: 100000, published: '2025-01-01T00:00:00Z', changelog: '', files: [{ name: 'sodium-0.5.0.jar', size: 800000 }] },
    { id: 'v1', name: 'Release 0.4.10', number: '0.4.10+mc1.18.2', kind: 'release', game_versions: ['1.18.2'], loaders: ['fabric'], downloads: 90000, published: '2024-01-01T00:00:00Z', changelog: '', files: [{ name: 'sodium-0.4.10.jar', size: 700000 }] },
  ].filter((v) => !mc || v.game_versions.includes(mc));
  let msCancel = false;
  const emit = (ev, payload) => (listeners[ev] || []).forEach((cb) => cb({ payload }));
  window.__emit = emit;
  window.__calls = [];
  const slow = (v, ms = 40) => new Promise((r) => setTimeout(() => r(v), ms));
  const handlers = {
    get_settings: () => settings,
    save_settings: ({ settings: s }) => { settings = s; },
    get_background: () => null,
    set_background: () => { settings.background = 'background.png'; },
    clear_background: () => { settings.background = null; },
    list_instances: () => instances,
    accounts_list: () => acct,
    running: () => [],
    mc_versions: () => versions,
    loader_versions: ({ loader, mc }) => (mc === '1.8.9' && loader !== 'forge' ? [] : ['0.16.9', '0.16.8', '0.16.7']),
    search: ({ kind, query }) => {
      const base = kind === 'modpack' ? packs : mods;
      const hits = base.filter((h) => !query || h.title.toLowerCase().includes(query.toLowerCase()));
      return slow({ hits, total: hits.length + (query ? 0 : 36) }, 80);
    },
    create_instance: ({ name, mc, loader, loaderVersion }) => { const i = { id: name.toLowerCase().replace(/\W+/g, '-'), name, mc_version: mc, loader, loader_version: loaderVersion, version_id: '', memory_mb: 0, java_args: '', created: 9, last_played: 0 }; instances.unshift(i); return i; },
    update_instance: ({ inst }) => { instances = instances.map((i) => (i.id === inst.id ? { ...i, ...inst } : i)); },
    delete_instance: ({ id }) => { instances = instances.filter((i) => i.id !== id); },
    list_content: ({ kind }) => files[kind] || [],
    toggle_content: ({ name, enabled }) => { const f = files.mods.find((x) => x.name === name); if (f) { f.enabled = enabled; f.name = enabled ? name.replace(/\.disabled$/, '') : name + '.disabled'; } },
    remove_content: ({ name }) => { files.mods = files.mods.filter((x) => x.name !== name); },
    project_details: ({ source, id }) => slow({
      source, id, title: id === 'p1' ? 'Fabulously Optimized' : 'Sodium', summary: 'The fastest and most compatible rendering optimization mod for Minecraft.',
      body: BODY, body_format: 'markdown', icon: null,
      gallery: [1, 2, 3].map((n) => ({ url: IMG(n), full: IMG(n), title: ['Menu screen', 'Performance graph', ''][n - 1], description: n === 1 ? 'The main menu with lots of text to see how the caption wraps nicely.' : '' })),
      categories: ['optimization', 'fabric', 'client'], loaders: ['fabric', 'quilt'], game_versions: ['1.18.2', '1.19.4', '1.20.1', '1.21', '1.21.1', '25w10a'],
      downloads: 62000000, followers: 120000, license: 'LGPL-3.0', published: '2020-01-01T00:00:00Z', updated: '2026-10-01T00:00:00Z',
      links: [{ label: 'Source', url: 'https://github.com/example/sodium' }, { label: 'Issues', url: 'https://github.com/example/sodium/issues' }, { label: 'Discord', url: 'https://discord.gg/x' }],
      team: [{ name: 'jellysquid3', role: 'Owner', avatar: null }, { name: 'IMS', role: 'Member', avatar: null }],
      kind: id === 'p1' ? 'modpack' : 'mod', web_url: 'https://modrinth.com/mod/sodium',
    }, 60),
    project_versions: ({ mc }) => slow(mkVersions(mc), 60),
    install_version_content: () => ['picked.jar'],
    install_modpack_version: ({ versionId }) => handlers.create_instance({ name: 'Pack ' + versionId, mc: '1.20.1', loader: 'fabric', loaderVersion: '0.16.9' }),
    ms_configured: () => !location.search.includes('nomsid'),
    ms_cancel: () => { msCancel = true; },
    ms_login: async () => { msCancel = false; for (let i = 0; i < 30; i++) { await slow(null, 100); if (msCancel) throw 'Sign-in cancelled'; } acct.accounts.push({ id: 'ms1', name: 'MSPlayer', kind: 'microsoft' }); acct.selected = 'ms1'; return acct; },
    install_content: () => ['sodium.jar', 'dep.jar'],
    install_modpack: () => handlers.create_instance({ name: 'Fabulously Optimized', mc: '1.21.1', loader: 'fabric', loaderVersion: '0.16.9' }),
    add_offline: ({ name }) => { if (name.length < 3) throw 'Usernames are 3-16 letters, numbers or underscores'; acct.accounts.push({ id: 'o' + name, name, kind: 'offline' }); acct.selected = 'o' + name; return acct; },
    select_account: ({ id }) => { acct.selected = id; return acct; },
    remove_account: ({ id }) => { acct.accounts = acct.accounts.filter((a) => a.id !== id); return acct; },
    open_url: () => {}, open_instance_folder: () => {}, kill_game: () => {},
    launch_game: async ({ id }) => { emit('progress', { stage: 'assets', done: 412, total: 3120, message: 'Downloading assets' }); await slow(null, 300); emit('game-started', { id }); emit('log', { id, line: '[12:00:01] [main/INFO]: Loading Minecraft 1.21.1 with Fabric Loader 0.16.9' }); emit('log', { id, line: '[12:00:02] [main/INFO]: Setting user: SpiderSlayer' }); },
  };
  window.__TAURI__ = {
    core: { invoke: async (cmd, args) => { window.__calls.push([cmd, args]); const h = handlers[cmd]; if (!h) throw 'unmocked command ' + cmd; return await h(args || {}); } },
    event: { listen: async (ev, cb) => { (listeners[ev] = listeners[ev] || []).push(cb); return () => {}; } },
    window: { getCurrentWindow: () => ({ minimize() {}, toggleMaximize() {}, close() {} }) },
  };
})();
