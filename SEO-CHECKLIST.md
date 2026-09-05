# Siegu — SEO indexing checklist

Goal: get `https://denzyldick.github.io/siegu/` (the landing site) — not the
`github.com/denzyldick/siegu` repo — to rank #1 for "siegu", and to let Google
pick up the real subpages (Pricing / FAQ / Docs / Download) as sitelinks.

## 1. Claim the site in Google Search Console

- Open <https://search.google.com/search-console> and add the property
  `https://denzyldick.github.io/siegu/` (URL prefix).
- Verify ownership. GitHub Pages serves custom `<meta>` tags fine, so use the
  **HTML tag** method: paste the `<meta name="google-site-verification" ...>`
  tag into `<head>` of `public/index.html`, then redeploy. (Don't pick the
  CNAME/DNS method — you don't control DNS for `*.github.io`.)
- Repeat the same claim for `https://siegu.io/` later **via DNS TXT record**
  in your VPS provider's panel, once the Caddy deployment is the primary host.
- Add the owner email/phone as a second owner so you're not locked out.

## 2. Submit the sitemap

- In Search Console **Sitemaps → Add a new sitemap**, enter `sitemap.xml`.
  It exists at `https://denzyldick.github.io/siegu/sitemap.xml` and lists all
  6 URLs (index, pricing, faq, docs, download, compare) with lastmod.
- Check back in a few days that it reports no "Couldn't fetch" errors.

## 3. Request indexing per page

- Use the **URL Inspection** tool for each of:
  - `https://denzyldick.github.io/siegu/`
  - `https://denzyldick.github.io/siegu/pricing.html`
  - `https://denzyldick.github.io/siegu/faq.html`
  - `https://denzyldick.github.io/siegu/download.html`
  - `https://denzyldick.github.io/siegu/docs.html`
  - `https://denzyldick.github.io/siegu/compare.html`
- Click **Request indexing** on each. New/near-empty sites need this nudge;
  otherwise pages can sit in "Discovered – currently not Indexed" for weeks.

## 4. Fix the repo-outranking-the-site problem

- Today Google shows `github.com/denzyldick/siegu` for "siegu" searches. The
  fix is a **strongly-linked, content-rich landing URL**:
  - `compare.html` gives the site real non-brand content ("Google Photos
    alternative") and internal links to pricing/download.
  - Add a **backlink** from the GitHub repo to the site: in the repo's
    `README.md` (and GitHub social preview / About field), link
    `https://denzyldick.github.io/siegu/` as the official homepage. The repo
    currently has more authority than the site; make it point at the site.
  - Keep canonical tags (`<link rel="canonical">` already present on every
    page) pointing at the GitHub Pages URLs.

## 5. Brand-result extras (after indexing)

- **Sitelinks** (the row under the main result) are chosen by Google
  algorithmically, but you improve the odds with: clean `<title>` + consistent
  anchor text (Pricing / FAQ / Docs / Download), the sitemap, and having those
  pages linked from the homepage nav and footer. Don't "pin" any page in the
  index as a workaround — it frequently doesn't help real Sitelinks.
- Once `siegu.io` serves HTTPS via Caddy/Let's Encrypt, migrate Search Console
  to that property (or verify both) and update the canonical base + sitemap to
  `https://siegu.io/...`, then request indexing again.
- Add a `linked_from` hint: keep the "Live demo" links `rel="noopener"` and
  indexable so Google also understands the demo URL.

## 6. Ongoing

- After any deploy with content changes, bump `lastmod` in
  `public/sitemap.xml` and request indexing for changed pages.
- Watch Search Console **Performance → Queries** for "siegu" clicks once a
  month. The goal: site (not repo) is the entry, sitelinks appear within a
  few weeks of indexing.
- Re-run `npm run check:translations` before deploys; a broken JSON file
  breaks every page's i18n (`applyTexts`), which search engines notice.