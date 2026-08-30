# Demo datasets

Bundled photo + video datasets used to seed a `siegu web` demo host so visitors
on the landing page can open the web app pre-loaded with a sample library. Each
subdirectory is one categorized dataset (one demo "album" when seeded). Seeding
also adds a combined **"My Photos"** album that contains every asset, so the
library reads like one person's camera roll.

## Categories

| Directory    | Album name       | Contents                  |
| ------------ | ---------------- | ------------------------- |
| `landscapes/`| Landscapes       | 10 photos                 |
| `people/`    | People & Faces   | 10 photos                 |
| `cities/`    | Cities & Travel  | 10 photos                 |
| `food/`      | Food & Still Life| 10 photos                 |
| `videos/`    | Videos           | 6 short clips + posters   |

## Seeding

```sh
siegu seed-demo           # seeds all categories into the current config dir
siegu seed-demo --demos landscapes,people
siegu seed-demo --config /path/to/config
```

The command copies the images into `<config>/demo/<category>/`, registers that
directory, indexes the photos into `siegu.db`, and creates one `album` per
category plus a combined "My Photos" album. The resulting library is served by
`siegu web` and the frontend opens straight to the album matched by the
`?demo=` query parameter.

## Licensing

The photographs in this folder are sourced from [picsum.photos](https://picsum.photos),
which serves photographs from [Unsplash](https://unsplash.com). Unsplash's
license grants a free, perpetual, non-exclusive, worldwide, non-transferable
right to use the photos for commercial and non-commercial purposes without
permission or attribution (attribution is appreciated but not required), and
does not permit compiling the photos to replicate a similar/competing service.

See `ATTRIBUTION.md` for the resolved fixed seeds used per image.

The video clips in `videos/` are generated synthetically (ffmpeg test patterns)
so they carry no third-party licensing; each clip ships with a poster JPEG used
as its library thumbnail.

If you prefer a curated set with explicit per-image CC0 provenance, drop your
images into these directories (keep the `*.jpg` extension) and re-run
`siegu seed-demo` — the category list and album names stay the same.
