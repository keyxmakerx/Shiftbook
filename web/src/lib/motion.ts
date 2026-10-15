// Motion helpers. The Motion setting scales durations (Still = none, Calm = 1, Lively = 1.35)
// and the system's reduced-motion setting turns animation off whatever the setting says.

import type { Motion } from './api';

export function motionScale(motion: Motion): number {
  return motion === 'still' ? 0 : motion === 'lively' ? 1.35 : 1;
}

let current: Motion = 'calm';

/** Remember the current setting (the shell calls this when preferences load or change). */
export function setMotion(motion: Motion): void {
  current = motion;
}

function systemReduced(): boolean {
  return typeof matchMedia === 'function' && matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** The scale to use now: 0 when nothing should move. */
export function scale(): number {
  return systemReduced() ? 0 : motionScale(current);
}

/** Ease-out cubic, 0..1 → 0..1. */
export function easeOut(t: number): number {
  const k = Math.min(1, Math.max(0, t));
  return 1 - Math.pow(1 - k, 3);
}

/** The value `progress` of the way from `from` to `to`, eased. */
export function tweenAt(from: number, to: number, progress: number): number {
  return from + (to - from) * easeOut(progress);
}

/**
 * Count a number from `from` to `to`, calling `draw` each frame (numbers count up instead of
 * jumping). Draws the end value at once when nothing should move. Returns a cancel function.
 */
export function countTo(from: number, to: number, draw: (v: number) => void, baseMs = 450): () => void {
  const k = scale();
  if (k === 0 || from === to || typeof requestAnimationFrame !== 'function') {
    draw(to);
    return () => {};
  }
  const start = performance.now();
  const duration = baseMs * k;
  let frame = 0;
  const step = (now: number) => {
    const p = (now - start) / duration;
    draw(p >= 1 ? to : tweenAt(from, to, p));
    if (p < 1) frame = requestAnimationFrame(step);
  };
  frame = requestAnimationFrame(step);
  return () => cancelAnimationFrame(frame);
}

/** Run a Web Animation scaled by the motion setting; does nothing when still. */
export function animate(
  el: Element,
  frames: Keyframe[],
  baseMs: number,
  easing = 'cubic-bezier(.2,.9,.25,1.08)',
): Animation | null {
  const k = scale();
  if (k === 0 || typeof el.animate !== 'function') return null;
  return el.animate(frames, { duration: baseMs * k, easing, fill: 'none' });
}
