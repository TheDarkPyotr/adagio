import type { CustomPaletteDto } from './tauri';

// ── Colour math ───────────────────────────────────────────────────────────────

function hexToRgb(hex: string): [number, number, number] {
  const h = hex.replace('#', '');
  return [
    parseInt(h.slice(0, 2), 16),
    parseInt(h.slice(2, 4), 16),
    parseInt(h.slice(4, 6), 16),
  ];
}

function rgbToHex(r: number, g: number, b: number): string {
  return '#' + [r, g, b].map(v => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0')).join('');
}

/** Linear interpolation between two hex colours.  t=0 → c1, t=1 → c2. */
function mix(c1: string, c2: string, t: number): string {
  const [r1, g1, b1] = hexToRgb(c1);
  const [r2, g2, b2] = hexToRgb(c2);
  return rgbToHex(r1 + (r2 - r1) * t, g1 + (g2 - g1) * t, b1 + (b2 - b1) * t);
}

/** True when `cream` is a dark colour (dark-mode palette). */
function isDarkBg(cream: string): boolean {
  const [r, g, b] = hexToRgb(cream);
  return (r * 299 + g * 587 + b * 114) / 1000 < 128;
}

// ── Token derivation ──────────────────────────────────────────────────────────

/**
 * Derive the full set of CSS custom-property values from 3 seed colours.
 * Returns a map of `{ '--cream': '#…', '--ink': '#…', … }`.
 */
export function deriveTokens(
  cream: string,
  ink: string,
  accent: string,
): Record<string, string> {
  const dark = isDarkBg(cream);
  const white = '#ffffff';
  const black = '#000000';
  // For light mode, derivatives get DARKER (mix toward ink).
  // For dark mode, derivatives get LIGHTER (mix toward white).
  const towards = dark ? white : ink;
  const [ri, gi, bi] = hexToRgb(ink);

  return {
    '--cream':      cream,
    '--cream-2':    mix(cream, towards, dark ? 0.08 : 0.07),
    '--cream-3':    mix(cream, towards, dark ? 0.20 : 0.17),
    '--paper':      mix(cream, white,   dark ? 0.04 : 0.38),
    '--paper-2':    mix(cream, towards, dark ? 0.06 : 0.04),
    '--sand':       mix(cream, towards, dark ? 0.32 : 0.26),
    '--ink':        ink,
    '--ink-2':      mix(ink, cream, 0.10),
    '--ink-soft':   mix(ink, cream, 0.42),
    '--ink-muted':  mix(ink, cream, 0.62),
    '--hairline':   `rgba(${ri},${gi},${bi},0.10)`,
    '--hairline-2': `rgba(${ri},${gi},${bi},0.05)`,
    '--clay':       accent,
    '--clay-2':     mix(accent, dark ? cream : ink, 0.18),
    '--clay-soft':  mix(accent, cream, 0.30),
    '--forest':     mix(ink, cream, 0.22),
    '--forest-soft':mix(ink, cream, 0.38),
    '--good':       dark ? '#7aa86a' : '#4f8a4b',
    '--warn':       dark ? '#d4a04a' : '#b8862a',
    '--danger':     dark ? '#d96650' : '#a83a22',
  };
}

// ── Application ───────────────────────────────────────────────────────────────

/**
 * Apply a custom palette by injecting its derived CSS variables directly onto
 * `<html>` as inline style properties.  These take precedence over the
 * `[data-palette="…"]` stylesheet rules.
 */
export function applyCustomPaletteTokens(palette: CustomPaletteDto): void {
  const tokens = deriveTokens(palette.cream, palette.ink, palette.accent);
  const root = document.documentElement;
  // Use a custom data attribute so we can clear tokens later.
  root.dataset.palette = 'custom';
  for (const [prop, value] of Object.entries(tokens)) {
    root.style.setProperty(prop, value);
  }
}

/** Remove all custom palette inline properties, reverting to a data-palette class. */
export function clearCustomPaletteTokens(nextPalette: string): void {
  const root = document.documentElement;
  const PROPS = [
    '--cream','--cream-2','--cream-3','--paper','--paper-2','--sand',
    '--ink','--ink-2','--ink-soft','--ink-muted',
    '--hairline','--hairline-2',
    '--clay','--clay-2','--clay-soft',
    '--forest','--forest-soft',
    '--good','--warn','--danger',
  ];
  for (const p of PROPS) root.style.removeProperty(p);
  root.dataset.palette = nextPalette;
}
