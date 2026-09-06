# Siegu Video Assets

## Files

| File | Description |
|------|-------------|
| `video-cover.png` | Static video thumbnail (1920×1080) |
| `siegu-intro.mp4` | Animated intro (5s, 1920×1080, 30fps, H.264) |
| `frame-00.png` | Frame 0 (start) |
| `frame-60.png` | Frame 60 (2s — title visible) |
| `frame-120.png` | Frame 120 (4s — banner + CTA) |
| `remotion-src/` | Source code (Remotion project) |

## Re-render

```bash
cd video/remotion-src
npm install
npx remotion render SieguIntro ../siegu-intro.mp4 --codec h264
```

## Duration & timing

- 0–0.8s: Logo fades in from left
- 0.7–1.5s: "Siegu" title slides in from right
- 1.2–2.2s: Green accent line grows, tagline rises
- 1.8–2.7s: Banner screenshot fades in + scales up
- 3.0–3.7s: CTA row appears
- 4.0–5.0s: Hold / fade out

## Editing

All animation timing lives in `remotion-src/Intro.tsx` via `interpolate()` calls.
Change `durationInFrames` in `remotion-src/Root.tsx` to adjust length.
