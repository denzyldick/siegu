# Demo image attribution

Each image in this bundle is a real photograph fetched from
[picsum.photos](https://picsum.photos) using a fixed `seed` (`<category><n>`),
which resolves to a stable photograph from [Unsplash](https://unsplash.com).

The Unsplash License (https://unsplash.com/license) applies: free to use for
commercial and non-commercial purposes, no permission needed and no
attribution required. We do not resell or redistribute these photographs as a
standalone photography service, which the license forbids.

A short, non-exhaustive note for each commit: the same images are reproducible
at any time by downloading
`https://picsum.photos/seed/<category><n>/800/600`; the resolved source photo
is internally identified by that seed.

The `videos/` clips are synthetic (generated with ffmpeg lavfi sources at
640×360, ~4-5 s) and are not photographs; the sibling `N.jpg` is a first-frame
poster used as the clip's thumbnail. They carry no third-party licensing.

If you would prefer explicit, per-image CC0/public-domain attribution, replace
the files in each `demos/<category>/` directory with your own curated images
and update this file. Nothing else in the pipeline depends on the specific
images — only the file extension (`*.jpg`) and the directory name matter.
