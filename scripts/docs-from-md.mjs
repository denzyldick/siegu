/**
 * Siegu landing — build the Docs page from the app repo's real markdown
 * documentation (../siegu/docs by default, override with SIEGU_APP_DOCS).
 *
 * Zero-dependency markdown subset: headings, paragraphs, lists, fenced code
 * (incl. ASCII diagrams), inline code/bold/emphasis/links, tables, quotes.
 * Renders into the site's page-body chrome with stable anchor ids and a table
 * of contents, so every release's doc updates land on the landing page
 * automatically.
 */
import { existsSync, readFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = join(__dirname, '..');

const GITHUB_DOCS_DIR = 'https://github.com/denzyldick/siegu/tree/main/docs';

/* Which files become landing docs, in reading order. stopAt trims dev-only
   sections (e.g. building from source) off the end of the file. */
export const DOC_WHITELIST = [
  { file: 'getting-started.md', stopAt: '## Prerequisites', label: 'Getting started' },
  { file: 'sync.md', label: 'Mesh synchronization' },
  { file: 'sharing.md', label: 'Collection sharing' },
  { file: 'webclient.md', label: 'Web client (view-only)' },
  { file: 'configuration.md', label: 'Configuration' },
  { file: 'security.md', label: 'Security & privacy' },
  { file: 'cli.md', label: 'Command line (siegu-cli)' },
];

function esc(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}

function slugify(s) {
  return s
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, '-')
    .replace(/(^-+|-+$)/g, '');
}

/* Inline Markdown → HTML. Inline code is protected behind placeholders first,
   so bold/emphasis/link rewrites never touch code content. */
function inlineMd(s, ctx) {
  const codes = [];
  let out = esc(s).replace(/`([^`]+)`/g, (m, c) => {
    codes.push(c);
    return '\u0000' + (codes.length - 1) + '\u0000';
  });
  out = out
    .replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>')
    .replace(/(^|[^*])\*([^*\n]+)\*/g, '$1<em>$2</em>')
    .replace(/\b_(.+?)_\b/g, '<em>$1</em>')
    .replace(/\[([^\]]+)\]\(([^)]+)\)/g, (m, text, target) => {
      const t = target.trim();
      if (/^(https?:|mailto:)/.test(t)) {
        return `<a href="${t}" target="_blank" rel="noopener">${text}</a>`;
      }
      if (t.startsWith('#')) {
        return `<a href="${t}">${text}</a>`;
      }
      if (/\.md(?:$|#)/.test(t)) {
        const base = t.split('#')[0].replace(/\.md$/, '').toLowerCase();
        const known = ctx && ctx.anchorsMap[base];
        const hash = t.indexOf('#') !== -1 ? t.slice(t.indexOf('#')) : '';
        const href = known ? `#${known}${hash}` : `${GITHUB_DOCS_DIR}/${base}.md`;
        return `<a href="${href}">${text}</a>`;
      }
      return `<a href="${t}">${text}</a>`;
    });
  out = out.replace(/\u0000(\d+)\u0000/g, (m, i) => `<code>${esc(codes[+i])}</code>`);
  return out;
}

const inlineText = (s, ctx) => inlineMd(s, ctx).replace(/<[^>]+>/g, '');

function cellHtml(c, ctx) {
  return inlineMd(c, ctx);
}

function parseTable(rows, ctx) {
  const cells = (r) =>
    r.replace(/^\s*\|/, '').replace(/\|\s*$/, '').split('|').map((c) => c.trim());
  const header = cells(rows[0]);
  const body = rows.slice(2).filter(Boolean).map(cells);
  const th = header.map((c) => `<th>${cellHtml(c, ctx)}</th>`).join('');
  const trs = body
    .map((r) => `<tr>${r.map((c) => `<td>${cellHtml(c, ctx)}</td>`).join('')}</tr>`)
    .join('');
  return `<div class="table-wrap"><table><thead><tr>${th}</tr></thead><tbody>${trs}</tbody></table></div>`;
}

const isListStart = (l) => /^[-*+]\s+/.test(l) || /^\d+[.)]\s+/.test(l);
const listKind = (l) => (/^\d+[.)]\s+/.test(l) ? 'ol' : 'ul');
const listItemText = (l) => l.replace(/^[-*+]\s+/, '').replace(/^\d+[.)]\s+/, '');
const isFence = (l) => /^\s*(```|~~~)/.test(l);

function uniqueId(base, ctx) {
  let id = base || 'section';
  let n = 2;
  while (ctx.usedIds.has(id)) id = `${base}-${n++}`;
  ctx.usedIds.add(id);
  return id;
}

/* One markdown file → { title, rest } (dev sections after stopAt trimmed). */
function splitFile(file, raw, stopAt) {
  const lines = raw.replace(/\r/g, '').split('\n');
  const h = lines.findIndex((l) => /^#\s/.test(l));
  if (h === -1) return { title: file, rest: lines };
  let end = lines.length;
  if (stopAt) {
    const stop = lines.findIndex((l) => l.trim() === stopAt);
    if (stop > h) end = stop;
  }
  return { title: lines[h].replace(/^#\s+/, '').trim(), rest: lines.slice(h + 1, end) };
}

/* Lines → HTML blocks for one file's body. Headings shift down one level so
   the page itself keeps a single h1. */
function blockify(section, ctx) {
  const out = [];
  const lines = section.rest;
  let i = 0;
  while (i < lines.length) {
    const t = lines[i].trim();
    if (t === '' || t === '---') { i++; continue; }

    if (isFence(t)) {
      const fence = t.match(/^\s*(```|~~~)/)[1];
      const code = [];
      i++;
      while (i < lines.length && !isFence(lines[i])) { code.push(lines[i]); i++; }
      i++;
      out.push(`<pre><code>${esc(code.join('\n'))}</code></pre>`);
      continue;
    }

    const h = t.match(/^(#{1,4})\s+(.*)$/);
    if (h) {
      const level = Math.min(h[1].length + 1, 6);
      const id = uniqueId(slugify(h[2]), ctx);
      const hrefText = inlineText(h[2], ctx);
      out.push(`<h${level} id="${id}">${inlineMd(h[2], ctx)}</h${level}>`);
      ctx.toc.push({ level, href: `#${id}`, hrefText });
      i++;
      continue;
    }

    if (/^>\s?/.test(t)) {
      const q = [];
      while (i < lines.length && /^>\s?/.test(lines[i].trim())) {
        q.push(inlineMd(lines[i].trim().replace(/^>\s?/, ''), ctx));
        i++;
      }
      out.push(`<blockquote>${q.join('<br>')}</blockquote>`);
      continue;
    }

    if (t.startsWith('|')) {
      const rows = [];
      while (i < lines.length && lines[i].trim().startsWith('|')) {
        rows.push(lines[i].trim());
        i++;
      }
      if (rows.length >= 2) out.push(parseTable(rows, ctx));
      continue;
    }

    if (isListStart(t)) {
      const kind = listKind(t);
      const items = [];
      let current = listItemText(t);
      i++;
      while (i < lines.length) {
        const n = lines[i].trim();
        if (isListStart(n)) {
          items.push(current);
          current = listItemText(n);
        } else if (n === '' || /^#{1,4}\s/.test(n) || isFence(n) || n.startsWith('|') || /^>\s?/.test(n)) {
          break;
        } else if (n !== '---') {
          current += ' ' + n;
        }
        i++;
      }
      items.push(current);
      out.push(`<${kind}>${items.map((it) => `<li>${inlineMd(it, ctx)}</li>`).join('')}</${kind}>`);
      continue;
    }

    const para = [];
    while (i < lines.length) {
      const n = lines[i].trim();
      if (n === '' || n === '---' || /^#{1,4}\s/.test(n) || isFence(n) || isListStart(n) || n.startsWith('|') || /^>\s?/.test(n)) break;
      para.push(n);
      i++;
    }
    out.push(`<p>${inlineMd(para.join(' '), ctx)}</p>`);
  }
  return out.join('\n');
}

function latestDocDate(appDocsDir) {
  const dates = [];
  for (const it of DOC_WHITELIST) {
    try {
      const d = execFileSync('git', ['-C', appDocsDir, 'log', '-1', '--format=%cs', '--', it.file], { encoding: 'utf8' }).trim();
      if (/^\d{4}-\d{2}-\d{2}$/.test(d)) dates.push(d);
    } catch { /* file unknown or not a git repo */ }
  }
  return dates.length ? dates.sort().at(-1) : new Date().toISOString().slice(0, 10);
}

const DOCS_MAIN_FALLBACK = `
    <section class="page-hero">
      <div class="container">
        <p class="eyebrow">Documentation</p>
        <h1>Siegu docs</h1>
        <p class="sub">Documentation and guides for Siegu, the private local-first photo library.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
          <p>Siegu is a private, local-first photo library. Your photos and metadata stay on your device, organized and searchable with on-device AI. You own your library &mdash; no cloud, no uploads, no lock-in.</p>
          <p>The full documentation lives on GitHub, alongside the open-source code.</p>
          <p>Popular topics:</p>
          <ul>
            <li><a href="https://github.com/denzyldick/siegu/tree/main/docs" target="_blank" rel="noopener">Documentation home</a></li>
            <li><a href="https://github.com/denzyldick/siegu/blob/main/README.md" target="_blank" rel="noopener">Readme &amp; getting started</a></li>
            <li><a href="https://github.com/denzyldick/siegu" target="_blank" rel="noopener">Source code on GitHub</a></li>
          </ul>
        </div>
      </div>
    </section>`;

/* Main entry: returns { main, ok, lastMod } where main fits inside <main>.
   Falls back to the "docs live on GitHub" stub when the app docs are absent,
   so the landing always builds. */
export function buildDocsMain({ appDocsDir } = {}) {
  const docsDir = appDocsDir || process.env.SIEGU_APP_DOCS || join(ROOT, '..', 'siegu', 'docs');

  if (!existsSync(docsDir)) {
    return { main: DOCS_MAIN_FALLBACK, ok: false, lastMod: new Date().toISOString().slice(0, 10) };
  }

  /* Pre-scan titles so cross-file .md links can resolve to in-page anchors. */
  const anchorsMap = {};
  const files = DOC_WHITELIST.map((item) => {
    let raw = null;
    try { raw = readFileSync(join(docsDir, item.file), 'utf8'); } catch { return null; }
    const { title } = splitFile(item.file, raw, item.stopAt);
    const sid = slugify(title);
    anchorsMap[item.file.replace(/\.md$/, '').toLowerCase()] = sid;
    return { item, raw, sectionId: sid };
  }).filter(Boolean);

  let ok = true;
  const sections = [];
  const toc = [];
  const sharedUsedIds = new Set();

  for (const f of files) {
    const { item, raw, sectionId } = f;
    const ctx = {
      anchorsMap,
      usedIds: sharedUsedIds,
      toc: [],
      tocId: sectionId,
    };
    const body = blockify(splitFile(item.file, raw, item.stopAt), ctx);

    sections.push(`<section class="doc-section" id="${sectionId}">
        <h2 class="doc-title">${esc(item.label || f.title)}</h2>
${body}
      </section>`);

    const sub = ctx.toc
      .filter((e) => e.level === 3)
      .map((e) => `            <li><a href="${e.href}">${esc(e.hrefText)}</a></li>`)
      .join('\n');

    toc.push(`          <li>
            <a href="#${sectionId}">${esc(item.label || f.title)}</a>
${sub ? `            <ul class="docs-toc-sub">\n${sub}\n            </ul>` : ''}
          </li>`);
  }

  if (!files.length) {
    return { main: DOCS_MAIN_FALLBACK, ok: false, lastMod: new Date().toISOString().slice(0, 10) };
  }

  const tocHtml = `        <nav class="docs-toc" aria-label="Documentation sections">
          <h2>On this page</h2>
          <ul>${toc.join('\n')}
          </ul>
        </nav>`;

  return {
    main: `<section class="page-hero">
      <div class="container">
        <p class="eyebrow">Documentation</p>
        <h1>Siegu docs</h1>
        <p class="sub">Guides, sync &amp; sharing, configuration, and security &mdash; straight from the open-source project, updated with every release.</p>
      </div>
    </section>

    <section class="section page-body">
      <div class="container">
        <div class="narrow">
${tocHtml}
${sections.join('\n')}
          <p class="docs-more">Looking for the full developer documentation? <a href="https://github.com/denzyldick/siegu/tree/main/docs" target="_blank" rel="noopener">Browse the GitHub docs</a>.</p>
        </div>
      </div>
    </section>`,
    ok,
    lastMod: latestDocDate(docsDir),
  };
}

/* Standalone dry run */
if (import.meta.url === `file://${process.argv[1]}`) {
  const res = buildDocsMain();
  console.log(`ok=${res.ok} lastMod=${res.lastMod} bytes=${res.main.length}`);
  process.exit(0);
}