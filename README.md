# siegu-landing

The public marketing website for **Siegu** — a private, local-first photo library.

A fast, modern, dependency-free static site. Monochrome aesthetic matching the
Siegu app (Outfit typeface, black-on-white / white-on-black), multi-language,
with a pricing section and a pluggable tracker/analytics hook.

## Run locally

```sh
npm run dev          # http://localhost:4174
PORT=8080 npm run dev
```

Zero dependencies — just Node. It serves `./public/` statically.

## Languages

`siegu-landing` is fully translated to the same 8 locales as the main app:

`en, nl, fr, es, pap (Papiamentu), de, it, pt`

The language switch is in the top-right of the header. The user's choice is
persisted in `localStorage` (`siegu_lang`); otherwise the browser language (or
English) is used.

Validate translations (every locale must define all `en.json` keys non-empty):

```sh
npm run check:translations
```

## Structure

```
public/
  index.html            # page shell (hero, features, showcase, pricing, FAQ, CTA, footer)
  css/styles.css        # monochrome theming (light/dark via prefers-color-scheme)
  js/main.js            # i18n loader, pricing toggle, FAQ accordion, theme, trackers
  locales/*.json        # one file per language
  logo.svg|png          # brand mark
  banner.png            # hero/showcase imagery
  screenshot.png        # app screenshot
scripts/
  serve.mjs             # zero-dep static server
  check-translations.js # locale completeness check
```

## Marketing / trackers

The site ships with a tracking hook ready for your analytics stack. Open
`public/js/main.js` → `trackersOn()` — it runs ~800ms after load and is the
single place to drop vendor snippets (Plausible, Fathom, GA, Meta Pixel,
Hotjar, …). Add a consent banner there if your stack requires one.

CTA clicks are tagged with `data-track` attributes and logged to the console /
`window.dataLayer` as `{ name, locale }`, ready to feed an analytics event.

## Pointing the Siegu app here

When running locally, the Siegu app's landing links are driven by env vars
(see `src/services/appConfig.ts` in the main repo):

- `VITE_APP_LANDING_URL=http://127.0.0.1:4174/` — "siegu.io" landing links
- `VITE_APP_GITHUB_URL=https://github.com/denzyldick/siegu`
- `VITE_APP_DOCS_URL=https://github.com/denzyldick/siegu/tree/main/docs`

Build/run the app with the first override to make its landing links open this
local site.
