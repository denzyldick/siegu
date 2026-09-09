import React from 'react';
import { useCurrentFrame, interpolate, Img, staticFile, Sequence } from 'remotion';

// Real app screenshots (captured from the demo host against Siegu's current UI)
const SHOTS = {
  library: staticFile('library.webp'),
  devices: staticFile('devices.webp'),
  search: staticFile('search.webp'),
  spacesaver: staticFile('spacesaver.webp'),
  collections: staticFile('collections.webp'),
  viewer: staticFile('viewer.webp'),
};

const GREEN = '#22c55e';
const BORDER = '#27272a';
const MUTED = '#a1a1aa';

const Reveal: React.FC<{
  frame: number;
  start: number;
  delay?: number;
  dur?: number;
  from?: number;
  children: React.ReactNode;
  style?: React.CSSProperties;
}> = ({ frame, start, delay = 0, dur = 26, from = 34, children, style }) => {
  const mid = start + Math.max(delay, 1);
  const opacity = interpolate(frame, [start, mid, mid + Math.min(dur, 8)], [0, 0, 1], { extrapolateRight: 'clamp' });
  const y = interpolate(frame, [start, mid + dur], [from, 0], { extrapolateRight: 'clamp' });
  return <div style={{ opacity, transform: `translateY(${y}px)`, ...style }}>{children}</div>;
};

const Logo: React.FC<{ frame: number }> = ({ frame }) => {
  const o = interpolate(frame, [0, 20], [0, 1], { extrapolateRight: 'clamp' });
  const x = interpolate(frame, [0, 20], [-46, 0], { extrapolateRight: 'clamp' });
  return (
    <div style={{ position: 'absolute', top: 40, left: 60, zIndex: 30, display: 'flex', alignItems: 'center', gap: 12, opacity: o, transform: `translateX(${x}px)` }}>
      <Img src={staticFile('logo.png')} style={{ width: 40, height: 40, filter: 'invert(1)' }} />
      <span style={{ color: '#fff', fontSize: 22, fontWeight: 800 }}>siegu</span>
    </div>
  );
};

// Scene template: screenshot inside a framed card + kicker/headline/sub.
const Scene: React.FC<{
  shot?: string;
  label: string;
  headline: string;
  sub?: string;
  frame: number;
  start: number;
  len: number;
  wide?: boolean;
  blurForHook?: boolean;
}> = ({ shot, label, headline, sub, frame, start, len, wide = false, blurForHook = false }) => {
  const local = frame - start;
  const o = interpolate(local, [0, 18], [0, 1], { extrapolateRight: 'clamp' });
  const scale = interpolate(local, [0, 24], [0.94, 1], { extrapolateRight: 'clamp' });
  const textO = interpolate(local, [22, 44], [0, 1], { extrapolateRight: 'clamp' });
  const out = interpolate(local, [len - 16, len], [1, 0], { extrapolateLeft: 'clamp' });
  return (
    <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(90% 80% at 50% 0%, rgba(34,197,94,0.10), transparent 60%)', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', gap: 26 }}>
      {shot ? (
        <div style={{ opacity: o * out, transform: `scale(${scale})`, border: `1px solid ${BORDER}`, borderRadius: 18, overflow: 'hidden', boxShadow: '0 40px 110px rgba(0,0,0,0.55)', width: wide ? 960 : 700, filter: blurForHook ? 'blur(30px) brightness(0.5)' : undefined }}>
          <Img src={shot} style={{ width: '100%', display: 'block' }} />
        </div>
      ) : null}
      <Reveal frame={local} start={20} delay={6} dur={18} style={{ textAlign: 'center', opacity: textO * out, maxWidth: 860 }}>
        <div style={{ color: GREEN, fontWeight: 800, fontSize: 18, textTransform: 'uppercase', letterSpacing: '0.16em', marginBottom: 10 }}>{label}</div>
        <div style={{ color: '#fff', fontWeight: 900, fontSize: 54, letterSpacing: '-0.02em', lineHeight: 1.06 }}>{headline}</div>
        {sub && <div style={{ color: MUTED, fontSize: 22, marginTop: 12 }}>{sub}</div>}
      </Reveal>
    </div>
  );
};

// Problem hook: the "where did that photo go?" beat.
const Hook: React.FC<{ frame: number; start: number; len: number }> = ({ frame, start, len }) => {
  const local = frame - start;
  const out = interpolate(local, [len - 16, len], [1, 0], { extrapolateLeft: 'clamp' });
  const lines = [
    { t: 0, c: 'Your photos are everywhere.' },
    { t: 22, c: 'Phone. Laptop. Drive. Cloud.' },
    { t: 46, c: 'And "I\u2019ll sort it later" never happens.' },
  ];
  return (
    <div style={{ position: 'absolute', inset: 0, background: 'radial-gradient(90% 80% at 50% 0%, rgba(34,197,94,0.08), transparent 60%)', display: 'flex', flexDirection: 'column', alignItems: 'center', justifyContent: 'center', opacity: out }}>
      {lines.map((l) => {
        const o = interpolate(local, [l.t, l.t + 16], [0, 1], { extrapolateRight: 'clamp' });
        const blur = interpolate(local, [l.t, l.t + 14], [4, 0], { extrapolateRight: 'clamp' });
        return (
          <div key={l.t}>
            <Reveal frame={local} start={l.t} dur={16} from={22}>
              <div style={{ fontSize: 62, fontWeight: 900, letterSpacing: '-0.03em', color: '#fff', filter: `blur(${blur}px)`, lineHeight: 1.5, textAlign: 'center' }}>{l.c}</div>
            </Reveal>
          </div>
        );
      })}
      <Reveal frame={local} start={66} dur={18} style={{ position: 'absolute', bottom: 90 }}>
        <div style={{ color: GREEN, fontWeight: 800, textTransform: 'uppercase', letterSpacing: '0.2em', fontSize: 18 }}>What if one private home could fix that?</div>
      </Reveal>
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
        <div style={{ color: '#fff', fontWeight: 900, fontSize: 66, letterSpacing: '-0.03em' }}>One private library,</div>
        <div style={{ color: MUTED, fontWeight: 900, fontSize: 66, letterSpacing: '-0.03em' }}>on every device you own</div>
        <div style={{ margin: '22px auto 0', width: 200, height: 3, background: GREEN, borderRadius: 2, transform: `scaleX(${glow})` }} />
        <div style={{ color: '#71717a', fontSize: 24, marginTop: 22 }}>Free forever · No account · No cloud · Works offline</div>
        <div style={{ display: 'flex', gap: 16, justifyContent: 'center', marginTop: 40 }}>
          <span style={{ display: 'inline-block', background: '#fff', color: '#0b0b0b', padding: '16px 40px', borderRadius: 999, fontWeight: 700, fontSize: 20 }}>Download free</span>
          <span style={{ display: 'inline-block', border: '1px solid #3f3f46', color: '#fff', padding: '16px 40px', borderRadius: 999, fontWeight: 600, fontSize: 20 }}>siegu.io</span>
        </div>
      </div>
    </div>
  );
};

export const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const FPS = 30;
  const hook = FPS * 3;
  const lib = FPS * 4;
  const sync = FPS * 4;
  const find = FPS * 4;
  const reclaim = FPS * 4;
  const organize = FPS * 4;
  const privacy = FPS * 4;
  const end = FPS * 4.5;
  let s = 0;
  const scenes: Array<{ from: number; len: number; body: React.ReactNode }> = [
    { from: s, len: hook, body: <Hook frame={frame} start={s} len={hook} /> },
    { from: (s += hook - 12), len: lib, body: <Scene shot={SHOTS.library} label="It starts with one home" headline="Every photo, finally in one place" sub="All your devices' photos, brought together under one roof." frame={frame} start={s} len={lib} wide /> },
    { from: (s += lib - 14), len: sync, body: <Scene shot={SHOTS.devices} label="Device-to-device sync" headline="On your phone, on your laptop" sub="Snap on one device, and it travels home to the rest — automatically." frame={frame} start={s} len={sync} wide /> },
    { from: (s += sync - 14), len: find, body: <Scene shot={SHOTS.search} label="Find anything" headline="“sunsets from the beach”" sub="Natural-language search, fully on-device. No uploads." frame={frame} start={s} len={find} wide /> },
    { from: (s += find - 14), len: reclaim, body: <Scene shot={SHOTS.spacesaver} label="Reclaim gigabytes" headline="That 4 GB of duplicates? Gone." sub="Exact, perceptual and AI duplicate detection — kept private, deleted local." frame={frame} start={s} len={reclaim} wide /> },
    { from: (s += reclaim - 14), len: organize, body: <Scene shot={SHOTS.collections} label="Automatically organized" headline="People, places, albums — as they arrive" sub="Siegu sorts itself, so you never have to face the backlog." frame={frame} start={s} len={organize} wide /> },
    { from: (s += organize - 14), len: privacy, body: <Scene shot={SHOTS.viewer} label="Private by design" headline="Your photos never leave your device" sub="No cloud. No uploads. No compromised memories." frame={frame} start={s} len={privacy} wide /> },
    { from: (s += privacy - 10), len: end, body: <EndCard frame={frame - (s + 8)} /> },
  ];
  return (
    <div style={{ width: '100%', height: '100%', backgroundColor: '#0b0b0b', fontFamily: 'system-ui, sans-serif', overflow: 'hidden', position: 'relative' }}>
      <Logo frame={frame} />
      {scenes.map((sc, i) => (
        <Sequence key={i} from={sc.from} durationInFrames={sc.len}>
          {sc.body}
        </Sequence>
      ))}
    </div>
  );
};