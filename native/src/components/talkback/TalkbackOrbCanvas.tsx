import React, { useEffect, useRef } from 'react';
import type { AgentVisualState } from './TalkbackAgent';

interface TalkbackOrbCanvasProps {
  visualState: AgentVisualState;
  /** Microphone input level, 0–1 */
  micLevel?: number;
  /** Speaker output level, 0–1 */
  outputLevel?: number;
  size?: number;
}

interface Particle {
  x: number;
  y: number;
  radius: number;
  baseAngle: number;
  distance: number;
  speed: number;
  phase: number;
  opacity: number;
}

/** Color palette definitions per visual state (RGB) */
const STATE_COLORS: Record<
  AgentVisualState,
  {
    primary: [number, number, number];
    secondary: [number, number, number];
    glow: [number, number, number];
    core: [number, number, number];
  }
> = {
  idle: {
    primary: [16, 185, 129], // Emerald
    secondary: [20, 140, 110],
    glow: [16, 185, 129],
    core: [71, 85, 105], // Slate 600 - calm, muted
  },
  listening: {
    primary: [16, 185, 129], // Emerald 500
    secondary: [52, 211, 153], // Emerald 400
    glow: [16, 185, 129],
    core: [16, 185, 129],
  },
  thinking: {
    primary: [129, 140, 248], // Indigo 400
    secondary: [192, 132, 252], // Purple 400
    glow: [99, 102, 241], // Indigo 500
    core: [129, 140, 248],
  },
  speaking: {
    primary: [20, 184, 166], // Teal 500
    secondary: [52, 211, 153], // Emerald 400
    glow: [20, 184, 166],
    core: [45, 212, 191], // Teal 400
  },
  interrupted: {
    primary: [245, 158, 11], // Amber 500
    secondary: [251, 191, 36],
    glow: [245, 158, 11],
    core: [245, 158, 11],
  },
  error: {
    primary: [239, 68, 68], // Red 500
    secondary: [248, 113, 113],
    glow: [239, 68, 68],
    core: [239, 68, 68],
  },
};

/**
 * Living Canvas-based conversational agent presence.
 *
 * Concepts:
 * - Liquid Core + Wave Conduit
 * - Directional Energy:
 *   • Idle: Quiet, peaceful presence with very subtle breathing & soft ambient glow.
 *   • Listening: Voice ripples flow inward toward the core from the user.
 *   • Thinking: Energy gathers & swirls inward in computational vortex.
 *   • Speaking: Acoustic harmonic waves radiate outward in sync with TTS audio.
 */
export const TalkbackOrbCanvas: React.FC<TalkbackOrbCanvasProps> = ({
  visualState,
  micLevel = 0,
  outputLevel = 0,
  size = 180,
}) => {
  const canvasRef = useRef<HTMLCanvasElement | null>(null);

  // References to smoothly interpolated parameters across rAF frames
  const paramsRef = useRef({
    energy: 0.04,
    rotation: 0,
    rotationSpeed: 0.001,
    pulsePhase: 0,
    waveDistortion: 0,
    glowOpacity: 0.12,
    flowPhase: 0,
    // Color interpolation [r, g, b]
    currentPrimary: [...STATE_COLORS.idle.primary] as [number, number, number],
    currentSecondary: [...STATE_COLORS.idle.secondary] as [number, number, number],
    currentGlow: [...STATE_COLORS.idle.glow] as [number, number, number],
    currentCore: [...STATE_COLORS.idle.core] as [number, number, number],
  });

  const particlesRef = useRef<Particle[]>([]);

  // Initialize particle set
  useEffect(() => {
    const count = 24;
    const particles: Particle[] = [];
    for (let i = 0; i < count; i++) {
      particles.push({
        x: 0,
        y: 0,
        radius: 1 + Math.random() * 1.5,
        baseAngle: (i / count) * Math.PI * 2 + (Math.random() - 0.5) * 0.3,
        distance: 0.5 + Math.random() * 0.45,
        speed: (0.15 + Math.random() * 0.35) * (Math.random() > 0.5 ? 1 : -1),
        phase: Math.random() * Math.PI * 2,
        opacity: 0.15 + Math.random() * 0.4,
      });
    }
    particlesRef.current = particles;
  }, []);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext('2d', { alpha: true });
    if (!ctx) return;

    let animFrame = 0;
    let lastTime = performance.now();
    let isHidden = false;
    let prefersReducedMotion = false;

    if (typeof window !== 'undefined') {
      prefersReducedMotion = window.matchMedia('(prefers-reduced-motion: reduce)').matches;
    }

    const onVisibilityChange = () => {
      isHidden = document.hidden;
    };
    document.addEventListener('visibilitychange', onVisibilityChange);

    const render = (now: number) => {
      animFrame = requestAnimationFrame(render);
      if (isHidden) return;

      const delta = Math.min((now - lastTime) / 1000, 0.1);
      lastTime = now;

      // Determine active audio intensity based on state
      const targetAudioLevel =
        visualState === 'listening'
          ? Math.min(1, micLevel * 4.5)
          : visualState === 'speaking'
          ? Math.min(1, outputLevel * 4.2)
          : 0;

      const targetColors = STATE_COLORS[visualState] || STATE_COLORS.idle;
      const params = paramsRef.current;

      // Smooth color transitions (LERP)
      const colorLerpSpeed = visualState === 'idle' ? 0.04 : 0.08;
      for (let i = 0; i < 3; i++) {
        params.currentPrimary[i] += (targetColors.primary[i] - params.currentPrimary[i]) * colorLerpSpeed;
        params.currentSecondary[i] += (targetColors.secondary[i] - params.currentSecondary[i]) * colorLerpSpeed;
        params.currentGlow[i] += (targetColors.glow[i] - params.currentGlow[i]) * colorLerpSpeed;
        params.currentCore[i] += (targetColors.core[i] - params.currentCore[i]) * colorLerpSpeed;
      }

      // State-specific behavior targets
      let targetEnergy = 0.03;
      let targetRotationSpeed = 0.0008; // Extremely slow rotation for idle
      let targetWaveDistortion = 0.008;
      let targetGlowOpacity = 0.12;
      let flowSpeed = 0.02; // Inward (negative) or outward (positive)

      switch (visualState) {
        case 'idle':
          targetEnergy = 0.03;
          targetRotationSpeed = 0.0006;
          targetWaveDistortion = 0.006;
          targetGlowOpacity = 0.10;
          flowSpeed = 0.01;
          break;
        case 'listening':
          targetEnergy = 0.15 + targetAudioLevel * 0.85;
          targetRotationSpeed = 0.003 + targetAudioLevel * 0.008;
          targetWaveDistortion = 0.08 + targetAudioLevel * 0.4;
          targetGlowOpacity = 0.25 + targetAudioLevel * 0.45;
          flowSpeed = -(0.04 + targetAudioLevel * 0.12); // Waves move inward
          break;
        case 'thinking':
          targetEnergy = 0.6;
          targetRotationSpeed = 0.03; // Smooth computation swirl
          targetWaveDistortion = 0.16;
          targetGlowOpacity = 0.4;
          flowSpeed = -0.08; // Energy gathering inward
          break;
        case 'speaking':
          targetEnergy = 0.28 + targetAudioLevel * 0.72;
          targetRotationSpeed = 0.008 + targetAudioLevel * 0.015;
          targetWaveDistortion = 0.12 + targetAudioLevel * 0.5;
          targetGlowOpacity = 0.35 + targetAudioLevel * 0.5;
          flowSpeed = 0.06 + targetAudioLevel * 0.18; // Waves move outward
          break;
        case 'interrupted':
          targetEnergy = 0.22;
          targetRotationSpeed = 0.002;
          targetWaveDistortion = 0.04;
          targetGlowOpacity = 0.25;
          flowSpeed = 0;
          break;
        case 'error':
          targetEnergy = 0.3;
          targetRotationSpeed = 0.001;
          targetWaveDistortion = 0.08;
          targetGlowOpacity = 0.3;
          flowSpeed = 0;
          break;
      }

      if (prefersReducedMotion) {
        targetRotationSpeed = 0;
        targetWaveDistortion = 0;
        flowSpeed = 0;
      }

      // Exponential easing of parameters
      const easeFactor = visualState === 'idle' ? 0.05 : 0.12;
      params.energy += (targetEnergy - params.energy) * easeFactor;
      params.rotationSpeed += (targetRotationSpeed - params.rotationSpeed) * easeFactor;
      params.rotation += params.rotationSpeed * (delta * 60);
      params.waveDistortion += (targetWaveDistortion - params.waveDistortion) * easeFactor;
      params.glowOpacity += (targetGlowOpacity - params.glowOpacity) * easeFactor;
      
      // Pulse phase: slower and calmer when idle (0.016 vs active 0.03+)
      const pulseSpeed = visualState === 'idle' ? 0.016 : 0.03 + params.energy * 0.05;
      params.pulsePhase += pulseSpeed * (delta * 60);
      params.flowPhase += flowSpeed * (delta * 60);

      // Canvas dimensions & HiDPI scaling
      const dpr = window.devicePixelRatio || 1;
      const width = size;
      const height = size;
      if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
      }

      ctx.save();
      ctx.scale(dpr, dpr);
      ctx.clearRect(0, 0, width, height);

      const cx = width / 2;
      const cy = height / 2;
      const baseRadius = width * 0.36;

      const [pr, pg, pb] = params.currentPrimary.map(Math.round);
      const [sr, sg, sb] = params.currentSecondary.map(Math.round);
      const [gr, gg, gb] = params.currentGlow.map(Math.round);
      const [cr, cg, cb] = params.currentCore.map(Math.round);

      // ── Layer 1: Ambient Volumetric Glow ─────────────────────────────────
      const glowScale = 1 + Math.sin(params.pulsePhase * 0.7) * (visualState === 'idle' ? 0.03 : 0.08) + params.energy * 0.3;
      const glowRadius = baseRadius * 1.35 * glowScale;
      const glowGrad = ctx.createRadialGradient(cx, cy, baseRadius * 0.15, cx, cy, glowRadius);
      glowGrad.addColorStop(0, `rgba(${gr}, ${gg}, ${gb}, ${params.glowOpacity * 0.85})`);
      glowGrad.addColorStop(0.5, `rgba(${gr}, ${gg}, ${gb}, ${params.glowOpacity * 0.3})`);
      glowGrad.addColorStop(1, `rgba(${gr}, ${gg}, ${gb}, 0)`);

      ctx.beginPath();
      ctx.arc(cx, cy, glowRadius, 0, Math.PI * 2);
      ctx.fillStyle = glowGrad;
      ctx.fill();

      // ── Layer 2: Wave Conduit & Harmonic Rings ───────────────────────────
      const ringCount = visualState === 'idle' ? 2 : 3;
      for (let r = 0; r < ringCount; r++) {
        const ringBaseScale = 0.65 + r * 0.22;
        // Directional wave offset based on flowPhase
        const flowOffset = Math.sin(params.flowPhase + r * 1.2) * 0.04 * params.energy;
        const currentRingRadius = baseRadius * (ringBaseScale + flowOffset);
        const segments = visualState === 'idle' ? 48 : 72;
        const ringRotation = params.rotation * (r % 2 === 0 ? 1 : -1) * (1 + r * 0.2);

        ctx.beginPath();
        for (let s = 0; s <= segments; s++) {
          const theta = (s / segments) * Math.PI * 2;
          // Harmonic wave distortion along ring circumference
          const harmonic =
            Math.sin(theta * (3 + r) + params.pulsePhase * 1.5 + ringRotation) *
            Math.cos(theta * 2 - params.flowPhase) *
            params.waveDistortion *
            currentRingRadius *
            0.35;

          const radius = currentRingRadius + harmonic;
          const px = cx + Math.cos(theta) * radius;
          const py = cy + Math.sin(theta) * radius;

          if (s === 0) ctx.moveTo(px, py);
          else ctx.lineTo(px, py);
        }
        ctx.closePath();

        const baseAlpha = visualState === 'idle' ? 0.12 / (r + 1) : (0.2 + (r === 1 ? 0.3 : 0.15) + params.energy * 0.4) / (r + 1);
        ctx.strokeStyle = `rgba(${pr}, ${pg}, ${pb}, ${Math.min(0.9, baseAlpha)})`;
        ctx.lineWidth = visualState === 'idle' ? 1.0 : 1.2 + (r === 1 ? 0.8 : 0) + params.energy * 1.5;
        if (r === 2 && visualState === 'thinking') {
          ctx.setLineDash([4, 6]);
        } else {
          ctx.setLineDash([]);
        }
        ctx.stroke();
      }
      ctx.setLineDash([]);

      // ── Layer 3: Particles ───────────────────────────────────────────────
      // In idle, only show a few subtle floating dust motes
      const activeParticleCount = visualState === 'idle' ? 8 : particlesRef.current.length;
      const particles = particlesRef.current;
      for (let i = 0; i < activeParticleCount; i++) {
        const p = particles[i];
        const pSpeed = visualState === 'idle' ? p.speed * 0.2 : p.speed;
        p.baseAngle += pSpeed * params.rotationSpeed * 2.0 * (delta * 60);

        // Breathing & directional flow displacement
        const breath = Math.sin(params.pulsePhase + p.phase) * (visualState === 'idle' ? 0.02 : 0.07);
        const flowDrift = Math.sin(params.flowPhase + p.phase) * (visualState === 'idle' ? 0 : params.energy * 0.15);
        const excitedDistance = (p.distance + breath + flowDrift) * baseRadius;

        const waveOffset = Math.sin(p.baseAngle * 4 + params.pulsePhase) * params.waveDistortion * 8;
        const rad = excitedDistance + waveOffset;

        p.x = cx + Math.cos(p.baseAngle + params.rotation * 0.4) * rad;
        p.y = cy + Math.sin(p.baseAngle + params.rotation * 0.4) * rad;

        const pAlpha = visualState === 'idle'
          ? Math.min(0.3, p.opacity * 0.4)
          : Math.min(0.9, p.opacity * (0.3 + params.energy * 0.7));

        ctx.beginPath();
        ctx.arc(p.x, p.y, p.radius * (visualState === 'idle' ? 0.8 : 1 + params.energy * 0.4), 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${sr}, ${sg}, ${sb}, ${pAlpha})`;
        ctx.fill();
      }

      // ── Layer 4: Living Liquid Core ──────────────────────────────────────
      // Very calm breathing when idle (period ~4.5-5s, ±2.5% scale)
      const coreBreath = Math.sin(params.pulsePhase * 0.9) * (visualState === 'idle' ? 0.025 : 0.06);
      const coreRadius = baseRadius * 0.32 * (1 + coreBreath + params.energy * 0.32);

      // Core Outer Diffuse Halo
      const coreHaloGrad = ctx.createRadialGradient(cx, cy, coreRadius * 0.2, cx, cy, coreRadius * 1.5);
      coreHaloGrad.addColorStop(0, `rgba(${cr}, ${cg}, ${cb}, ${visualState === 'idle' ? 0.5 : 0.8})`);
      coreHaloGrad.addColorStop(0.6, `rgba(${pr}, ${pg}, ${pb}, ${visualState === 'idle' ? 0.2 : 0.35})`);
      coreHaloGrad.addColorStop(1, `rgba(${pr}, ${pg}, ${pb}, 0)`);

      ctx.beginPath();
      ctx.arc(cx, cy, coreRadius * 1.5, 0, Math.PI * 2);
      ctx.fillStyle = coreHaloGrad;
      ctx.fill();

      // Core Organic Deformed Body (Liquid boundary)
      const coreSegments = 36;
      ctx.beginPath();
      for (let c = 0; c <= coreSegments; c++) {
        const theta = (c / coreSegments) * Math.PI * 2;
        const meniscus =
          visualState === 'idle'
            ? 0
            : Math.sin(theta * 3 + params.pulsePhase * 1.8) *
              Math.cos(theta * 2 - params.flowPhase) *
              params.waveDistortion *
              coreRadius *
              0.15;
        const r = coreRadius + meniscus;
        const px = cx + Math.cos(theta) * r;
        const py = cy + Math.sin(theta) * r;

        if (c === 0) ctx.moveTo(px, py);
        else ctx.lineTo(px, py);
      }
      ctx.closePath();

      // Core Internal Volumetric Gradient
      const coreSolidGrad = ctx.createRadialGradient(
        cx - coreRadius * 0.25,
        cy - coreRadius * 0.25,
        1,
        cx,
        cy,
        coreRadius,
      );
      coreSolidGrad.addColorStop(0, `rgba(255, 255, 255, ${visualState === 'idle' ? 0.75 : 0.95})`);
      coreSolidGrad.addColorStop(0.35, `rgba(${cr}, ${cg}, ${cb}, ${visualState === 'idle' ? 0.7 : 0.9})`);
      coreSolidGrad.addColorStop(0.85, `rgba(${pr}, ${pg}, ${pb}, ${visualState === 'idle' ? 0.5 : 0.7})`);
      coreSolidGrad.addColorStop(1, `rgba(${pr}, ${pg}, ${pb}, ${visualState === 'idle' ? 0.2 : 0.4})`);

      ctx.fillStyle = coreSolidGrad;
      ctx.fill();

      // Inner Core Specular Highlight
      ctx.beginPath();
      ctx.arc(
        cx - coreRadius * 0.28,
        cy - coreRadius * 0.28,
        coreRadius * 0.26,
        0,
        Math.PI * 2,
      );
      ctx.fillStyle = `rgba(255, 255, 255, ${visualState === 'idle' ? 0.4 : 0.65})`;
      ctx.fill();

      ctx.restore();
    };

    animFrame = requestAnimationFrame(render);

    return () => {
      cancelAnimationFrame(animFrame);
      document.removeEventListener('visibilitychange', onVisibilityChange);
    };
  }, [visualState, micLevel, outputLevel, size]);

  return (
    <canvas
      ref={canvasRef}
      style={{ width: size, height: size }}
      className="block pointer-events-none select-none transition-transform duration-500 ease-out"
      aria-hidden="true"
    />
  );
};
