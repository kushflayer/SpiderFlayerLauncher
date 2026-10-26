/* Tiny Markdown renderer + HTML sanitizer for project descriptions.
   Descriptions come from strangers on the internet, so everything is rebuilt through an
   allow-list: only known tags and attributes survive, links and images must be https,
   and links never navigate the launcher itself (they open in the real browser). */
(() => {
  'use strict';

  const escHtml = (s) => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
  const escAttr = (s) => escHtml(s);

  // ---------- inline ----------
  function inline(src) {
    const codes = [];
    let s = src.replace(/`([^`\n]+)`/g, (_, c) => { codes.push(c); return `\u0000${codes.length - 1}\u0000`; });
    // image (optionally wrapped in a link is handled by the link rule afterwards)
    s = s.replace(/!\[([^\]]*)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g, (_, alt, url) => `<img alt="${escAttr(alt)}" src="${escAttr(url)}">`);
    s = s.replace(/\[([^\]]+)\]\(\s*<?([^)\s>]+)>?(?:\s+"[^"]*")?\s*\)/g, (_, text, url) => `<a href="${escAttr(url)}">${text}</a>`);
    s = s.replace(/\*\*\*([^*\n]+)\*\*\*/g, '<strong><em>$1</em></strong>')
      .replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>')
      .replace(/__([^_\n]+)__/g, '<strong>$1</strong>')
      .replace(/(^|[^*\w])\*([^*\n]+)\*(?!\w)/g, '$1<em>$2</em>')
      .replace(/(^|[^_\w])_([^_\n]+)_(?!\w)/g, '$1<em>$2</em>')
      .replace(/~~([^~\n]+)~~/g, '<del>$1</del>');
    s = s.replace(/(^|[\s(])(https?:\/\/[^\s<)]+)/g, (_, pre, url) => `${pre}<a href="${escAttr(url)}">${escHtml(url)}</a>`);
    return s.replace(/\u0000(\d+)\u0000/g, (_, i) => `<code>${escHtml(codes[+i])}</code>`);
  }

  // ---------- blocks ----------
  const RAW_HTML = /^\s*<\/?(p|div|center|img|a|h[1-6]|details|summary|table|thead|tbody|tr|td|th|br|hr|ul|ol|li|picture|source|iframe|video|span|b|i|strong|em|kbd|sub|sup)\b/i;
  const LIST_ITEM = /^(\s*)([-*+]|\d+[.)])\s+(.*)$/;
  const TABLE_SEP = /^\s*\|?\s*:?-{2,}:?\s*(\|\s*:?-{2,}:?\s*)*\|?\s*$/;

  function splitRow(line) {
    let t = line.trim();
    if (t.startsWith('|')) t = t.slice(1);
    if (t.endsWith('|')) t = t.slice(0, -1);
    return t.split('|').map((c) => c.trim());
  }

  function blocks(text, depth = 0) {
    if (depth > 6) return escHtml(text);
    const lines = text.replace(/\r\n?/g, '\n').split('\n');
    const out = [];
    let i = 0;
    let para = [];
    const flush = () => {
      if (!para.length) return;
      const joined = para.map((l, n) => (n < para.length - 1 && / {2,}$/.test(l) ? l.trimEnd() + '<br>' : l.trim())).join(' ');
      out.push(`<p>${inline(joined)}</p>`);
      para = [];
    };
    while (i < lines.length) {
      const line = lines[i];
      if (/^\s*$/.test(line)) { flush(); i++; continue; }

      const fence = line.match(/^\s*(```|~~~)/);
      if (fence) {
        flush();
        const code = [];
        i++;
        while (i < lines.length && !lines[i].trim().startsWith(fence[1])) code.push(lines[i++]);
        i++;
        out.push(`<pre><code>${escHtml(code.join('\n'))}</code></pre>`);
        continue;
      }
      const h = line.match(/^\s{0,3}(#{1,6})\s+(.*?)\s*#*\s*$/);
      if (h) { flush(); out.push(`<h${h[1].length}>${inline(h[2])}</h${h[1].length}>`); i++; continue; }
      if (/^\s{0,3}([-*_])(\s*\1){2,}\s*$/.test(line)) { flush(); out.push('<hr>'); i++; continue; }
      if (/^\s*>/.test(line)) {
        flush();
        const q = [];
        while (i < lines.length && /^\s*>/.test(lines[i])) q.push(lines[i++].replace(/^\s*>\s?/, ''));
        out.push(`<blockquote>${blocks(q.join('\n'), depth + 1)}</blockquote>`);
        continue;
      }
      if (RAW_HTML.test(line) && !para.length) { out.push(line); i++; continue; }
      if (line.includes('|') && i + 1 < lines.length && TABLE_SEP.test(lines[i + 1]) && lines[i + 1].includes('-')) {
        flush();
        const head = splitRow(line);
        i += 2;
        const rows = [];
        while (i < lines.length && lines[i].includes('|') && !/^\s*$/.test(lines[i])) rows.push(splitRow(lines[i++]));
        out.push(`<table><thead><tr>${head.map((c) => `<th>${inline(c)}</th>`).join('')}</tr></thead><tbody>${rows.map((r) => `<tr>${r.map((c) => `<td>${inline(c)}</td>`).join('')}</tr>`).join('')}</tbody></table>`);
        continue;
      }
      const li = line.match(LIST_ITEM);
      if (li) {
        flush();
        const baseIndent = li[1].length;
        const ordered = /\d/.test(li[2]);
        const items = [];
        while (i < lines.length) {
          const m = lines[i].match(LIST_ITEM);
          if (m && m[1].length <= baseIndent) {
            items.push([m[3]]);
            i++;
          } else if (items.length && (/^\s+\S/.test(lines[i]) || (/^\s*$/.test(lines[i]) && i + 1 < lines.length && /^\s+\S/.test(lines[i + 1])))) {
            items[items.length - 1].push(lines[i].replace(new RegExp(`^\\s{0,${baseIndent + 2}}`), ''));
            i++;
          } else break;
        }
        const tag = ordered ? 'ol' : 'ul';
        out.push(`<${tag}>${items.map((it) => {
          const [first, ...rest] = it;
          const nested = rest.join('\n').trim();
          return `<li>${inline(first.replace(/^\[( |x|X)\]\s+/, (_, c) => (c === ' ' ? '☐ ' : '☑ ')))}${nested ? blocks(nested, depth + 1) : ''}</li>`;
        }).join('')}</${tag}>`);
        continue;
      }
      para.push(line);
      i++;
    }
    flush();
    return out.join('\n');
  }

  // ---------- sanitizer ----------
  const ALLOWED = new Set('a b blockquote br center code del details div em h1 h2 h3 h4 h5 h6 hr i img kbd li ol p pre s span strong sub summary sup table tbody td th thead tr u ul'.split(' '));
  const DROP = new Set(['script', 'style', 'iframe', 'object', 'embed', 'form', 'input', 'button', 'textarea', 'select', 'option', 'link', 'meta', 'svg', 'math', 'video', 'audio', 'canvas', 'base', 'template', 'noscript', 'head', 'title']);
  const httpsUrl = (u) => { try { const x = new URL(String(u).trim(), 'https://invalid.invalid/'); return x.protocol === 'https:' && x.hostname !== 'invalid.invalid' ? x.href : ''; } catch (e) { return ''; } };
  const httpUrl = (u) => { try { const x = new URL(String(u).trim()); return x.protocol === 'https:' || x.protocol === 'http:' ? x.href : ''; } catch (e) { return ''; } };

  function clean(node, parent, doc) {
    for (const child of Array.from(node.childNodes)) {
      if (child.nodeType === 3) { parent.appendChild(doc.createTextNode(child.nodeValue)); continue; }
      if (child.nodeType !== 1) continue;
      const tag = child.tagName.toLowerCase();
      if (DROP.has(tag)) continue;
      if (!ALLOWED.has(tag)) { clean(child, parent, doc); continue; } // unwrap unknown tags, keep their content
      const el = doc.createElement(tag);
      if (tag === 'a') {
        const href = httpUrl(child.getAttribute('href') || '');
        if (href) { el.setAttribute('href', '#'); el.setAttribute('data-act', 'open-url'); el.setAttribute('data-url', href); el.setAttribute('title', href); }
      } else if (tag === 'img') {
        const src = httpsUrl(child.getAttribute('src') || '');
        if (!src) continue;
        el.setAttribute('src', src);
        el.setAttribute('alt', child.getAttribute('alt') || '');
        el.setAttribute('loading', 'lazy');
        el.setAttribute('referrerpolicy', 'no-referrer');
        for (const a of ['width', 'height']) {
          const v = child.getAttribute(a);
          if (v && /^\d{1,4}%?$/.test(v)) el.setAttribute(a, v);
        }
      }
      const align = (child.getAttribute('align') || '').toLowerCase();
      if (/^(left|center|right)$/.test(align) && /^(p|div|h[1-6]|td|th|img|table)$/.test(tag)) el.setAttribute('align', align);
      clean(child, el, doc);
      parent.appendChild(el);
    }
  }

  function sanitize(html) {
    const doc = new DOMParser().parseFromString(`<body>${html}</body>`, 'text/html');
    const out = doc.createElement('div');
    clean(doc.body, out, doc);
    return out.innerHTML;
  }

  window.SFMD = {
    /** format: 'markdown' (Modrinth) or 'html' (CurseForge). Always returns safe HTML. */
    render(text, format) {
      const t = String(text || '');
      return sanitize(format === 'html' ? t : blocks(t));
    },
    sanitize,
  };
})();
