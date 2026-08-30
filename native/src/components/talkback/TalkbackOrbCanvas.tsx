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

/** Color palette definitions per visual state */
const STATE_COLORS: Record<
  AgentVisualState,
  {
    primary: [number, number, number]; // RGB
    secondary: [number, number, number];
    glow: [number, number, number];
    core: [number, number, number];
  }
> = {
  idle: {
    primary: [16, 185, 129], // Emerald
    secondary: [20, 140, 110],
    glow: [16, 185, 129],
    core: [100, 116, 139], // Muted slate
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
 * Implements:
 * - Continuous ambient breathing & orbital particle drift (never frozen).
 * - Real-time microphone audio reactive waves while listening.
 * - Dynamic spinning vortex energy while thinking.
 * - Voice acoustic wave pulses directly driven by output audio while speaking.
 * - Seamless spring-interpolated transitions between all states.
 * - Throttling & pausing when document is hidden or prefers-reduced-motion is on.
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
    energy: 0.1,
    coreScale: 1.0,
    haloScale: 1.0,
    rotation: 0,
    rotationSpeed: 0.005,
    pulsePhase: 0,
    waveDistortion: 0,
    glowOpacity: 0.25,
    // Color interpolation [r, g, b]
    currentPrimary: [...STATE_COLORS.idle.primary] as [number, number, number],
    currentSecondary: [...STATE_COLORS.idle.secondary] as [number, number, number],
    currentGlow: [...STATE_COLORS.idle.glow] as [number, number, number],
    currentCore: [...STATE_COLORS.idle.core] as [number, number, number],
  });

  const particlesRef = useRef<Particle[]>([]);

  // Initialize particle set
  useEffect(() => {
    const count = 36;
    const particles: Particle[] = [];
    for (let i = 0; i < count; i++) {
      particles.push({
        x: 0,
        y: 0,
        radius: 1 + Math.random() * 1.8,
        baseAngle: (i / count) * Math.PI * 2 + (Math.random() - 0.5) * 0.4,
        distance: 0.55 + Math.random() * 0.4,
        speed: (0.4 + Math.random() * 0.6) * (Math.random() > 0.5 ? 1 : -1),
        phase: Math.random() * Math.PI * 2,
        opacity: 0.3 + Math.random() * 0.6,
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
          ? Math.min(1, micLevel * 5)
          : visualState === 'speaking'
          ? Math.min(1, outputLevel * 4.5)
          : 0;

      const targetColors = STATE_COLORS[visualState] || STATE_COLORS.idle;
      const params = paramsRef.current;

      // Smooth color transitions (LERP)
      const colorLerpSpeed = 0.08;
      for (let i = 0; i < 3; i++) {
        params.currentPrimary[i] += (targetColors.primary[i] - params.currentPrimary[i]) * colorLerpSpeed;
        params.currentSecondary[i] += (targetColors.secondary[i] - params.currentSecondary[i]) * colorLerpSpeed;
        params.currentGlow[i] += (targetColors.glow[i] - params.currentGlow[i]) * colorLerpSpeed;
        params.currentCore[i] += (targetColors.core[i] - params.currentCore[i]) * colorLerpSpeed;
      }

      // State-specific behavior targets
      let targetEnergy = 0.08;
      let targetRotationSpeed = 0.006;
      let targetWaveDistortion = 0;
      let targetGlowOpacity = 0.2;

      switch (visualState) {
        case 'idle':
          targetEnergy = 0.08;
          targetRotationSpeed = 0.005;
          targetWaveDistortion = 0.02;
          targetGlowOpacity = 0.18;
          break;
        case 'listening':
          targetEnergy = 0.25 + targetAudioLevel * 0.75;
          targetRotationSpeed = 0.01 + targetAudioLevel * 0.02;
          targetWaveDistortion = 0.1 + targetAudioLevel * 0.35;
          targetGlowOpacity = 0.3 + targetAudioLevel * 0.5;
          break;
        case 'thinking':
          targetEnergy = 0.65;
          targetRotationSpeed = 0.045; // Rapid orbital spin
          targetWaveDistortion = 0.18;
          targetGlowOpacity = 0.45;
          break;
        case 'speaking':
          targetEnergy = 0.35 + targetAudioLevel * 0.65;
          targetRotationSpeed = 0.015 + targetAudioLevel * 0.03;
          targetWaveDistortion = 0.15 + targetAudioLevel * 0.45;
          targetGlowOpacity = 0.4 + targetAudioLevel * 0.5;
          break;
        case 'interrupted':
          targetEnergy = 0.3;
          targetRotationSpeed = 0.003;
          targetWaveDistortion = 0.05;
          targetGlowOpacity = 0.3;
          break;
        case 'error':
          targetEnergy = 0.4;
          targetRotationSpeed = 0.002;
          targetWaveDistortion = 0.12;
          targetGlowOpacity = 0.4;
          break;
      }

      if (prefersReducedMotion) {
        targetRotationSpeed = 0;
        targetWaveDistortion = 0;
      }

      // Exponential easing of parameters
      params.energy += (targetEnergy - params.energy) * 0.12;
      params.rotationSpeed += (targetRotationSpeed - params.rotationSpeed) * 0.1;
      params.rotation += params.rotationSpeed * (delta * 60);
      params.waveDistortion += (targetWaveDistortion - params.waveDistortion) * 0.15;
      params.glowOpacity += (targetGlowOpacity - params.glowOpacity) * 0.1;
      params.pulsePhase += (0.04 + params.energy * 0.06) * (delta * 60);

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
      const glowScale = 1 + Math.sin(params.pulsePhase * 0.8) * 0.08 + params.energy * 0.35;
      const glowRadius = baseRadius * 1.35 * glowScale;
      const glowGrad = ctx.createRadialGradient(cx, cy, baseRadius * 0.2, cx, cy, glowRadius);
      glowGrad.addColorStop(0, `rgba(${gr}, ${gg}, ${gb}, ${params.glowOpacity * 0.85})`);
      glowGrad.addColorStop(0.5, `rgba(${gr}, ${gg}, ${gb}, ${params.glowOpacity * 0.35})`);
      glowGrad.addColorStop(1, `rgba(${gr}, ${gg}, ${gb}, 0)`);

      ctx.beginPath();
      ctx.arc(cx, cy, glowRadius, 0, Math.PI * 2);
      ctx.fillStyle = glowGrad;
      ctx.fill();

      // ── Layer 2: Concentric Harmonic Wave Rings ──────────────────────────
      const ringCount = 3;
      for (let r = 0; r < ringCount; r++) {
        const ringScale = 0.65 + r * 0.22;
        const currentRingRadius = baseRadius * ringScale;
        const segments = 64;
        const ringRotation = params.rotation * (r % 2 === 0 ? 1 : -1) * (1 + r * 0.3);

        ctx.beginPath();
        for (let s = 0; s <= segments; s++) {
          const theta = (s / segments) * Math.PI * 2;
          // Harmonic wave distortion along ring circumference
          const harmonic =
            Math.sin(theta * (3 + r) + params.pulsePhase * 2 + ringRotation) *
            Math.cos(theta * 2 - params.pulsePhase) *
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

        const ringAlpha = (0.2 + (r === 1 ? 0.3 : 0.15) + params.energy * 0.4) / (r + 1);
        ctx.strokeStyle = `rgba(${pr}, ${pg}, ${pb}, ${Math.min(1, ringAlpha)})`;
        ctx.lineWidth = 1.2 + (r === 1 ? 0.8 : 0) + params.energy * 1.5;
        if (r === 2 && visualState === 'thinking') {
          ctx.setLineDash([4, 6]);
        } else {
          ctx.setLineDash([]);
        }
        ctx.stroke();
      }
      ctx.setLineDash([]);

      // ── Layer 3: Dynamic Orbiting Particles ──────────────────────────────
      const particles = particlesRef.current;
      for (let i = 0; i < particles.length; i++) {
        const p = particles[i];
        p.baseAngle += p.speed * params.rotationSpeed * 1.8 * (delta * 60);

        // Breathing/audio excitation on particle distance
        const breath = Math.sin(params.pulsePhase + p.phase) * 0.08;
        const excitedDistance = (p.distance + breath + params.energy * 0.2) * baseRadius;

        // Wave displacement
        const waveOffset = Math.sin(p.baseAngle * 4 + params.pulsePhase * 2) * params.waveDistortion * 10;
        const rad = excitedDistance + waveOffset;

        p.x = cx + Math.cos(p.baseAngle + params.rotation * 0.5) * rad;
        p.y = cy + Math.sin(p.baseAngle + params.rotation * 0.5) * rad;

        const pAlpha = Math.min(1, p.opacity * (0.4 + params.energy * 0.6));
        ctx.beginPath();
        ctx.arc(p.x, p.y, p.radius * (1 + params.energy * 0.5), 0, Math.PI * 2);
        ctx.fillStyle = `rgba(${sr}, ${sg}, ${sb}, ${pAlpha})`;
        ctx.fill();
      }

      // ── Layer 4: Living Core Orb ─────────────────────────────────────────
      const coreBreath = Math.sin(params.pulsePhase * 1.2) * 0.06;
      const coreRadius = baseRadius * 0.32 * (1 + coreBreath + params.energy * 0.35);

      // Core Outer Halo
      const coreHaloGrad = ctx.createRadialGradient(cx, cy, coreRadius * 0.2, cx, cy, coreRadius * 1.6);
      coreHaloGrad.addColorStop(0, `rgba(${cr}, ${cg}, ${cb}, 0.8)`);
      coreHaloGrad.addColorStop(0.6, `rgba(${pr}, ${pg}, ${pb}, 0.35)`);
      coreHaloGrad.addColorStop(1, `rgba(${pr}, ${pg}, ${pb}, 0)`);

      ctx.beginPath();
      ctx.arc(cx, cy, coreRadius * 1.6, 0, Math.PI * 2);
      ctx.fillStyle = coreHaloGrad;
      ctx.fill();

      // Core Solid Center
      const coreSolidGrad = ctx.createRadialGradient(cx - coreRadius * 0.25, cy - coreRadius * 0.25, 1, cx, cy, coreRadius);
      coreSolidGrad.addColorStop(0, `rgba(255, 255, 255, 0.95)`);
      coreSolidGrad.addColorStop(0.35, `rgba(${cr}, ${cg}, ${cb}, 0.9)`);
      coreSolidGrad.addColorStop(0.85, `rgba(${pr}, ${pg}, ${pb}, 0.7)`);
      coreSolidGrad.addColorStop(1, `rgba(${pr}, ${pg}, ${pb}, 0.4)`);

      ctx.beginPath();
      ctx.arc(cx, cy, coreRadius, 0, Math.PI * 2);
      ctx.fillStyle = coreSolidGrad;
      ctx.fill();

      // Inner Core Specular Highlight
      ctx.beginPath();
      ctx.arc(cx - coreRadius * 0.28, cy - coreRadius * 0.28, coreRadius * 0.28, 0, Math.PI * 2);
      ctx.fillStyle = `rgba(255, 255, 255, 0.65)`;
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
      className="block pointer-events-none select-none"
      aria-hidden="true"
    />
  );
};
