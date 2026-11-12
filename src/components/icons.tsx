import type { LucideIcon } from "lucide-react";
import { useId } from "react";

/** Aqua tarzı cam yüzeyli kare ikon (Dock ve uygulama ikonları için). */
export function AquaTile({ color, glyph: Glyph, size }: { color: string; glyph: LucideIcon; size: number }) {
  return (
    <div
      className="relative grid shrink-0 place-items-center"
      style={{
        width: size,
        height: size,
        borderRadius: size * 0.22,
        background: `linear-gradient(color-mix(in srgb, ${color} 55%, #fff), ${color} 55%, color-mix(in srgb, ${color} 78%, #000))`,
        boxShadow: "inset 0 1px 0 rgba(255,255,255,.7), inset 0 -2px 4px rgba(0,0,0,.25), 0 1px 3px rgba(0,0,0,.45)",
      }}
    >
      <div
        className="absolute"
        style={{
          top: "3%",
          left: "7%",
          right: "7%",
          height: "46%",
          borderRadius: `${size * 0.2}px ${size * 0.2}px ${size * 0.45}px ${size * 0.45}px / ${size * 0.2}px ${size * 0.2}px ${size * 0.2}px ${size * 0.2}px`,
          background: "linear-gradient(rgba(255,255,255,.7), rgba(255,255,255,.08))",
        }}
      />
      <Glyph
        size={size * 0.5}
        color="#fff"
        strokeWidth={2}
        style={{ position: "relative", filter: "drop-shadow(0 1px 1px rgba(0,0,0,.4))" }}
      />
    </div>
  );
}

/** Finder tarzı klasör ikonu; ortasında kategori simgesi. */
export function FolderIcon({ color, glyph: Glyph, size = 48 }: { color: string; glyph?: LucideIcon; size?: number }) {
  const id = useId();
  return (
    <svg width={size} height={size * 0.875} viewBox="0 0 64 56" aria-hidden style={{ filter: "drop-shadow(0 2px 2px rgba(0,0,0,.35))" }}>
      <defs>
        <linearGradient id={`${id}f`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" style={{ stopColor: `color-mix(in srgb, ${color} 55%, #fff)` }} />
          <stop offset="1" style={{ stopColor: color }} />
        </linearGradient>
      </defs>
      <path
        d="M4 10a4 4 0 0 1 4-4h14l6 6h28a4 4 0 0 1 4 4v30a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4z"
        style={{ fill: `color-mix(in srgb, ${color} 70%, #000)` }}
      />
      <path d="M4 20a4 4 0 0 1 4-4h48a4 4 0 0 1 4 4v26a4 4 0 0 1-4 4H8a4 4 0 0 1-4-4z" fill={`url(#${id}f)`} />
      <path d="M5 20a3 3 0 0 1 3-3h48a3 3 0 0 1 3 3v6H5z" fill="#fff" opacity=".28" />
      {Glyph && <Glyph x={22} y={24} width={20} height={20} color="#fff" strokeWidth={2.2} style={{ opacity: 0.9 }} />}
    </svg>
  );
}

/** Masaüstündeki sabit disk ikonu; doluluk oranı ön paneldeki ışıkla anlatılır. */
export function DriveIcon({ size = 52 }: { size?: number }) {
  const id = useId();
  return (
    <svg width={size} height={size * 0.78} viewBox="0 0 64 50" aria-hidden style={{ filter: "drop-shadow(0 2px 2px rgba(0,0,0,.4))" }}>
      <defs>
        <linearGradient id={`${id}m`} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0" stopColor="#f4f6f8" />
          <stop offset=".5" stopColor="#c9cfd6" />
          <stop offset="1" stopColor="#8d949d" />
        </linearGradient>
      </defs>
      <rect x="2" y="6" width="60" height="38" rx="6" fill={`url(#${id}m)`} stroke="#6f767f" />
      <rect x="3" y="7" width="58" height="12" rx="5" fill="#fff" opacity=".45" />
      <rect x="10" y="30" width="30" height="5" rx="2.5" fill="#5c626b" opacity=".55" />
      <circle cx="52" cy="32.5" r="3" fill="#53d769" stroke="#2c8f3f" />
    </svg>
  );
}

/** Tek dosya için basit belge ikonu. */
export function FileIcon({ size = 48 }: { size?: number }) {
  return (
    <svg width={size * 0.8} height={size} viewBox="0 0 48 60" aria-hidden style={{ filter: "drop-shadow(0 2px 2px rgba(0,0,0,.3))" }}>
      <path d="M4 4a2 2 0 0 1 2-2h26l12 12v42a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2z" fill="#fafafa" stroke="#9aa0a6" />
      <path d="M32 2v10a2 2 0 0 0 2 2h10z" fill="#dfe3e8" stroke="#9aa0a6" />
      <g stroke="#b5bbc2" strokeWidth="2">
        <path d="M11 26h26M11 32h26M11 38h20" />
      </g>
    </svg>
  );
}
