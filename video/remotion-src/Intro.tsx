import React from 'react';
import { useCurrentFrame, interpolate, spring, Img, staticFile, Sequence } from 'remotion';

// Copies of the site's shots
const SHOTS = {
  library: staticFile('library.webp'),
  album: staticFile('album.webp'),
  locations: staticFile('locations.webp'),
  viewer: staticFile('viewer.webp'),
  share: staticFile('share.webp'),
  banner: staticFile('banner.webp'),
};

// Gentle fade/slide helper for a block
const Reveal: React.FC<{ frame: number; start: number; dur?: number; delay?: number; from?: number; children: React.ReactNode; style?: React.CSSProperties }> = ({
  frame, start, dur = 30, delay = 0, from = 36, children, style,
}) => {
  const opacity = interpolate(frame, [start, start + delay, start + delay + Math.min(dur, 10)], [0, 0, 1], { extrapolateRight: 'clamp' });
  const y = interpolate(frame, [start, start + delay + dur], [from, 0], { extrapolateRight: 'clamp' });
  return <div style={{ opacity, transform: `translateY(${y}px)`, ...style }}>{children}</div>;
};

const Logo: React.FC<{ frame: number }> = ({ frame }) => {
  const o = interpolate(frame, [0, 18], [0, 1], { extrapolateRight: 'clamp' });
  const x = interpolate(frame, [0, 18], [-50, 0], { extrapolateRight: 'clamp' });
  return (
    <div style={{ position: 'absolute', top: 40, left: 60, display: 'flex', alignItems: 'center', gap: 12, opacity: o, transform: `translateX(${x}px)` }}>
      <Img src={staticFile('logo.png')} style={{ width: 40, height: 40, filter: 'invert(1)' }} />
      <span style={{ color: '#fff', fontSize: 22, fontWeight: 800 }}>siegu</span>
    </div>
  );
};

// Scene template: dark bg, faint glow, screenshot in a framed card + caption
const Scene: React.FC<{ shot: string; label: string; headline: string; sub?: string; frame: number; start: number; len: number; darkHead?: boolean }> = ({
  shot, label, headline, sub, frame, start, len, darkHead,
}) => {
  const local = frame - start;
  const o = interpolate(local, [0, 20], [0, 1], { extrapolateRight: 'clamp' });
  const scale = interpolate(local, [0, 26], [0.9, 1], { extrapolateRight: 'clamp' });
  const textO = interpolate(local, [24, 44], [0, 1], { extrapolateRight: 'clamp' });
  const out = interpolate(local, [len - 18, len], [1, 0], { extrapolateLeft: 'clamp' });
  return (
    <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(90% 80% at 50% 0%, rgba(34,197,94,0.10), transparent 60%)', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 26 }}>
      <div style={{ opacity: o * out, transform: `scale(${scale})`, border: '1px solid #27272a', borderRadius: 18, overflow: 'hidden', boxShadow: '0 40px 110px rgba(0,0,0,0.55)', width: 700 }}>
        <Img src={shot} style={{ width: '100%', display: 'block' }} />
      </div>
      <Reveal frame={local} start={22} delay={8} dur={16} style={{ textAlign: 'center', opacity: textO * out, maxWidth: 820 }}>
        <div style={{ color: '#22c55e', fontWeight: 800, fontSize: 18, textTransform: 'uppercase', letterSpacing: '0.16em', marginBottom: 10 }}>{label}</div>
        <div style={{ color: darkHead ? '#0b0b0b' : '#fff', fontWeight: 900, fontSize: 52, letterSpacing: '-0.02em', lineHeight: 1.08 }}>{headline}</div>
        {sub && <div style={{ color: '#a1a1aa', fontSize: 22, marginTop: 12 }}>{sub}</div>}
      </Reveal>
    </div>
  );
};

export const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  return (
    <div style={{ width: '100%', height: '100%', backgroundColor: '#0b0b0b', fontFamily: 'system-ui, sans-serif', overflow: 'hidden', position: 'relative' }}>
      <Logo frame={frame} />

      {/* Scene 1 — open banner */}
      <Sequence from={0} durationInFrames={95}>
        <Scene shot={SHOTS.library} label="The library" headline="A home for every memory" sub="Clean, calm and fast — your entire photo history, something you enjoy browsing." frame={frame} start={0} len={95} />
      </Sequence>

      {/* Scene 2 — search */}
      <Sequence from={85} durationInFrames={85}>
        <Scene shot={SHOTS.album} label="Find anything" headline="“sunsets at the beach”" sub="Natural-language search with on-device AI. No uploads." frame={frame} start={85} len={85} />
      </Sequence>

      {/* Scene 3 — organize */}
      <Sequence from={160} durationInFrames={85}>
        <Scene shot={SHOTS.locations} label="Auto-organized" headline="Faces, places, trips & events" sub="Clustered automatically — and kept private on your device." frame={frame} start={160} len={85} />
      </Sequence>

      {/* Scene 4 — share */}
      <Sequence from={235} durationInFrames={85}>
        <Scene shot={SHOTS.share} label="Privately share" headline="Share one link. Nothing else leaves." sub="End-to-end encrypted. Guests see only what you allow." frame={frame} start={235} len={85} />
      </Sequence>

      {/* Scene 5 — stat card / promise */}
      <Sequence from={310} durationInFrames={100}>
        <Scene shot={SHOTS.banner} label="Private by design" headline="Your photos never leave your device" sub="No cloud. No uploads. No compromises." frame={frame} start={310} len={100} />
      </Sequence>

      {/* Scene 6 — CTA/end card */}
      <Sequence from={400} durationInFrames={120}>
        <EndCard frame={frame - 400} />
      </Sequence>
    </div>
  );
};

const EndCard: React.FC<{ frame: number }> = ({ frame }) => {
  const o = interpolate(frame, [0, 24], [0, 1], { extrapolateRight: 'clamp' });
  const y = interpolate(frame, [0, 24], [30, 0], { extrapolateRight: 'clamp' });
  const glow = interpolate(frame, [40, 80], [0, 1], { extrapolateRight: 'clamp' });
  return (
    <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(80% 70% at 50% 45%, rgba(34,197,94,0.16), transparent 65%)', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', textAlign: 'center' }}>
      <div style={{ opacity: o, transform: `translateY(${y}px)` }}>
        <div style={{ color: '#fff', fontWeight: 900, fontSize: 64, letterSpacing: '-0.03em' }}>Your photo library,</div>
        <div style={{ color: '#a1a1aa', fontWeight: 900, fontSize: 64, letterSpacing: '-0.03em' }}>privately yours</div>
        <div style={{ margin: '22px auto 0', width: 200, height: 3, background: '#22c55e', borderRadius: 2, transform: `scaleX(${glow})` }} />
        <div style={{ color: '#71717a', fontSize: 24, marginTop: 22 }}>Free forever · No account · Works offline</div>
        <div style={{ display: 'flex', gap: 16, justifyContent: 'center', marginTop: 40 }}>
          <span style={{ display: 'inline-block', background: '#fff', color: '#0b0b0b', padding: '16px 40px', borderRadius: 999, fontWeight: 700, fontSize: 20 }}>Download free</span>
          <span style={{ display: 'inline-block', border: '1px solid #3f3f46', color: '#fff', padding: '16px 40px', borderRadius: 999, fontWeight: 600, fontSize: 20 }}>siegu.io</span>
        </div>
      </div>
    </div>
  );
};
