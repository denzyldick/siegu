# Siegu Video Assets

The animated trailer for Siegu, generated with Remotion.

## Files

| File | Description |
|------|-------------|
| `siegu-intro.mp4` | Full trailer (17s, 1920×1080, 30fps, H.264, silent) |
| `video-cover.jpg` | Poster / hero fallback (1280×720) |
| `frame-1.jpg` | Scene: The library |
| `frame-2.jpg` | Scene: Auto-organized |
| `frame-3.jpg` | Scene: Private by design |
| `remotion-src/` | Remotion project source |

## The trailer scenes (silent)

1. **The library** — "A home for every memory" (library screenshot)
2. **Find anything** — "sunsets at the beach" — natural-language search (album shot)
3. **Auto-organized** — faces, places, trips & events (locations shot)
4. **Privately share** — "Share one link. Nothing else leaves." (share shot)
5. **Private by design** — "Your photos never leave your device" (banner shot)
6. **End card** — "Your photo library, privately yours" + CTA row

Each scene fades/slides in on the dark `#0b0b0b` brand background with a green
(`#22c55e`) accent. All timing lives in `remotion-src/Intro.tsx` via
`Sequence` + `interpolate()`.

## Re-render

```bash
cd remotion-src
npm install
npx remotion render SieguIntro ../siegu-intro.mp4 --codec h264
```

## Edit

- `remotion-src/Intro.tsx` — scenes, timing, text, colors
- `remotion-src/Root.tsx` — `durationInFrames` (currently 520 = 17.3s at 30fps)

## Deploy

`siegu-intro.mp4` + `video-cover.jpg` + `frame-*.jpg` are copied into
`public/video/`, which the static build deploys to gh-pages. The homepage hero
loops the trailer (`autoplay muted loop`); `/video.html` hosts the full player.
