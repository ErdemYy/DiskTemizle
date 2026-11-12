import type { Wallpaper as WP } from "../store";

const PALETTE: Record<WP, { a: string; b: string; c: string; glow: string }> = {
  aqua: { a: "#5aa7ee", b: "#2a67c9", c: "#143f96", glow: "#ffffff" },
  graphite: { a: "#8d939c", b: "#555a63", c: "#25282e", glow: "#e8ecf2" },
};

/** Aqua esintili duvar kağıdı; tamamen vektörel, ek dosya gerektirmez. */
export function Wallpaper({ kind, className }: { kind: WP; className?: string }) {
  const p = PALETTE[kind];
  return (
    <svg className={className} viewBox="0 0 1600 900" preserveAspectRatio="xMidYMid slice" aria-hidden>
      <defs>
        <radialGradient id={`bg-${kind}`} cx="35%" cy="30%" r="95%">
          <stop offset="0" stopColor={p.a} />
          <stop offset=".55" stopColor={p.b} />
          <stop offset="1" stopColor={p.c} />
        </radialGradient>
        <linearGradient id={`sw-${kind}`} x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stopColor={p.glow} stopOpacity=".0" />
          <stop offset=".45" stopColor={p.glow} stopOpacity=".55" />
          <stop offset="1" stopColor={p.glow} stopOpacity=".0" />
        </linearGradient>
        <filter id={`bl-${kind}`}>
          <feGaussianBlur stdDeviation="1.6" />
        </filter>
      </defs>
      <rect width="1600" height="900" fill={`url(#bg-${kind})`} />
      <g filter={`url(#bl-${kind})`} fill="none" strokeLinecap="round">
        <path d="M-40 640 C 260 360, 520 520, 760 330 S 1240 -40, 1660 60" stroke={`url(#sw-${kind})`} strokeWidth="46" opacity=".55" />
        <path d="M-60 760 C 300 520, 620 700, 900 470 S 1360 180, 1680 300" stroke={`url(#sw-${kind})`} strokeWidth="14" opacity=".7" />
        <path d="M220 940 C 420 560, 640 360, 1000 260 S 1500 140, 1700 -20" stroke={`url(#sw-${kind})`} strokeWidth="26" opacity=".4" />
      </g>
      <ellipse cx="1180" cy="860" rx="520" ry="260" fill={p.glow} opacity=".08" />
    </svg>
  );
}
