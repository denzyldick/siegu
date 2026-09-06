import React from 'react';
import { useCurrentFrame, interpolate, spring, Img, staticFile } from 'remotion';

export const Intro: React.FC = () => {
  const frame = useCurrentFrame();
  const { fps } = { fps: 30 };

  // Logo: fade + slide from left
  const logoOpacity = interpolate(frame, [0, 25], [0, 1], { extrapolateRight: 'clamp' });
  const logoX = interpolate(frame, [0, 25], [-60, 0], { extrapolateRight: 'clamp' });

  // "Siegu" text: fade + slide from right
  const titleOpacity = interpolate(frame, [20, 45], [0, 1], { extrapolateRight: 'clamp' });
  const titleX = interpolate(frame, [20, 45], [60, 0], { extrapolateRight: 'clamp' });

  // Tagline: fade + rise
  const tagOpacity = interpolate(frame, [40, 60], [0, 1], { extrapolateRight: 'clamp' });
  const tagY = interpolate(frame, [40, 60], [24, 0], { extrapolateRight: 'clamp' });

  // Banner: fade + scale
  const bannerOpacity = interpolate(frame, [55, 80], [0, 1], { extrapolateRight: 'clamp' });
  const bannerScale = interpolate(frame, [55, 80], [0.92, 1], { extrapolateRight: 'clamp' });

  // CTA: fade
  const ctaOpacity = interpolate(frame, [90, 110], [0, 1], { extrapolateRight: 'clamp' });

  // Green accent line: grow from center
  const lineWidth = interpolate(frame, [35, 65], [0, 280], { extrapolateRight: 'clamp' });

  return (
    <div style={{
      width: '100%',
      height: '100%',
      backgroundColor: '#0b0b0b',
      display: 'flex',
      flexDirection: 'column',
      alignItems: 'center',
      justifyContent: 'center',
      fontFamily: 'system-ui, -apple-system, sans-serif',
      overflow: 'hidden',
      position: 'relative',
    }}>
      {/* Subtle radial glow */}
      <div style={{
        position: 'absolute',
        width: 900,
        height: 500,
        background: 'radial-gradient(ellipse, rgba(34,197,94,0.14) 0%, transparent 70%)',
        borderRadius: '50%',
        top: '50%',
        left: '50%',
        transform: 'translate(-50%, -50%)',
      }} />

      {/* Logo */}
      <div style={{
        position: 'absolute',
        top: 72,
        left: 88,
        display: 'flex',
        alignItems: 'center',
        gap: 14,
        opacity: logoOpacity,
        transform: `translateX(${logoX}px)`,
      }}>
        <Img
          src={staticFile('logo.png')}
          style={{
            width: 52,
            height: 52,
            filter: 'invert(1)',
          }}
        />
        <span style={{
          color: '#fff',
          fontSize: 28,
          fontWeight: 800,
          letterSpacing: '-0.01em',
        }}>siegu</span>
      </div>

      {/* Banner screenshot */}
      <div style={{
        opacity: bannerOpacity,
        transform: `scale(${bannerScale})`,
        border: '1px solid #27272a',
        borderRadius: 16,
        overflow: 'hidden',
        boxShadow: '0 40px 100px rgba(0,0,0,0.5)',
        marginBottom: 40,
      }}>
        <Img
          src={staticFile('banner.webp')}
          style={{ width: 860 }}
        />
      </div>

      {/* Green accent line */}
      <div style={{
        width: lineWidth,
        height: 3,
        backgroundColor: '#22c55e',
        borderRadius: 2,
        marginBottom: 24,
      }} />

      {/* Title */}
      <h1 style={{
        color: '#fff',
        fontSize: 72,
        fontWeight: 900,
        letterSpacing: '-0.03em',
        margin: 0,
        opacity: titleOpacity,
        transform: `translateX(${titleX}px)`,
      }}>
        Your photo library,
      </h1>
      <h1 style={{
        color: '#a1a1aa',
        fontSize: 72,
        fontWeight: 900,
        letterSpacing: '-0.03em',
        margin: '0 0 16px',
        opacity: titleOpacity,
        transform: `translateX(${titleX}px)`,
      }}>
        privately yours
      </h1>

      {/* Tagline */}
      <p style={{
        color: '#71717a',
        fontSize: 22,
        fontWeight: 400,
        margin: 0,
        opacity: tagOpacity,
        transform: `translateY(${tagY}px)`,
      }}>
        No cloud. No uploads. No compromises.
      </p>

      {/* CTA */}
      <div style={{
        marginTop: 48,
        opacity: ctaOpacity,
        display: 'flex',
        gap: 16,
      }}>
        <span style={{
          backgroundColor: '#fff',
          color: '#0b0b0b',
          padding: '14px 32px',
          borderRadius: 999,
          fontWeight: 700,
          fontSize: 18,
        }}>Download free</span>
        <span style={{
          border: '1px solid #3f3f46',
          color: '#fff',
          padding: '14px 32px',
          borderRadius: 999,
          fontWeight: 600,
          fontSize: 18,
        }}>siegu.io</span>
      </div>
    </div>
  );
};
