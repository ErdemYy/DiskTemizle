import { ShieldCheck } from "lucide-react";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Wallpaper } from "./Wallpaper";

type Phase = "loading" | "ready" | "zoom";
const LOAD_MS = 1300;
const ZOOM_MS = 950;

/** Ekran içeriği: gerçek masaüstünün minyatürü (duvar kağıdı, iki pencere ve Dock). */
function MiniDesktop() {
  return (
    <div className="relative h-full w-full overflow-hidden rounded-[3px]">
      <Wallpaper kind="aqua" className="absolute inset-0 h-full w-full" />
      <div className="absolute inset-x-0 top-0 h-[7%] bg-white/85" />
      <div className="absolute top-[16%] left-[12%] h-[42%] w-[46%] rounded-[3px] border border-black/40 bg-white shadow-md">
        <div className="h-[16%] rounded-t-[2px] bg-gradient-to-b from-[#f4f4f4] to-[#c9c9c9]" />
        <div className="grid grid-cols-3 gap-1 p-1.5">
          {["#3b82f6", "#14b8a6", "#8b5cf6"].map((c) => (
            <div key={c} className="h-3 rounded-[2px]" style={{ background: c }} />
          ))}
        </div>
      </div>
      <div className="absolute top-[34%] left-[42%] h-[36%] w-[40%] rounded-[3px] border border-black/40 bg-[#ededed] shadow-md">
        <div className="h-[18%] rounded-t-[2px] bg-gradient-to-b from-[#f4f4f4] to-[#c9c9c9]" />
      </div>
      <div className="absolute bottom-[3%] left-1/2 flex -translate-x-1/2 gap-[3px] rounded-t bg-white/40 px-1.5 pt-1">
        {["#22a6e0", "#3b82f6", "#14b8a6", "#8b5cf6", "#f59e0b"].map((c) => (
          <div key={c} className="size-[9px] rounded-[2px]" style={{ background: c }} />
        ))}
      </div>
    </div>
  );
}

/** Yarı saydam iMac G3 çizimi (özgün vektör). Ekran, MiniDesktop için üstüne yerleştirilir. */
function Imac({ screenRef }: { screenRef: React.RefObject<HTMLDivElement | null> }) {
  const keys: React.ReactNode[] = [];
  for (let r = 0; r < 4; r++) {
    for (let c = 0; c < 17; c++) {
      keys.push(<rect key={`${r}-${c}`} x={52 + c * 18} y={512 + r * 19} width="15" height="14" rx="3" fill="#fff" stroke="#c4d0e4" strokeWidth=".8" />);
    }
  }
  return (
    <div className="relative" style={{ width: "min(430px, 56vh)", aspectRatio: "520 / 620" }}>
      <svg viewBox="0 0 520 620" className="absolute inset-0 h-full w-full" aria-hidden>
        <defs>
          <linearGradient id="imBody" x1="0" y1="0" x2="0" y2="1">
            <stop offset="0" stopColor="#fbfdff" />
            <stop offset=".55" stopColor="#d6e4f8" />
            <stop offset="1" stopColor="#a9c3ee" />
          </linearGradient>
          <linearGradient id="imBlue" x1="0" y1="0" x2="1" y2="0">
            <stop offset="0" stopColor="#1f4fc4" />
            <stop offset=".5" stopColor="#4a82ee" />
            <stop offset="1" stopColor="#1b44ad" />
          </linearGradient>
          <radialGradient id="imOrange" cx=".4" cy=".35" r=".8">
            <stop offset="0" stopColor="#ffc46b" />
            <stop offset="1" stopColor="#e8780c" />
          </radialGradient>
          <filter id="imBlur">
            <feGaussianBlur stdDeviation="8" />
          </filter>
        </defs>
        <ellipse cx="260" cy="470" rx="230" ry="16" fill="#000" opacity=".25" filter="url(#imBlur)" />
        {/* Gövde ve yan nervürler */}
        <rect x="40" y="20" width="440" height="385" rx="125" ry="115" fill="url(#imBody)" stroke="#9db6dc" />
        <path d="M44 130 C 22 150, 22 290, 48 340 C 66 372, 92 372, 100 356 L 100 150 C 86 128, 60 118, 44 130 Z" fill="url(#imBlue)" opacity=".92" />
        <path d="M476 130 C 498 150, 498 290, 472 340 C 454 372, 428 372, 420 356 L 420 150 C 434 128, 460 118, 476 130 Z" fill="url(#imBlue)" opacity=".92" />
        <g stroke="#fff" strokeOpacity=".35" strokeWidth="2">
          {[34, 42, 50, 58, 66, 74].map((x) => (
            <path key={x} d={`M${x} 170 V 320`} />
          ))}
          {[446, 454, 462, 470, 478, 486].map((x) => (
            <path key={x} d={`M${x} 170 V 320`} />
          ))}
        </g>
        {/* Çerçeve ve ekran gölgesi */}
        <rect x="104" y="46" width="312" height="256" rx="22" fill="#12151c" />
        <rect x="112" y="54" width="296" height="240" rx="16" fill="#1d222c" />
        {/* Çene: hoparlörler ve kontroller */}
        <rect x="76" y="318" width="58" height="46" rx="12" fill="url(#imBlue)" />
        <rect x="386" y="318" width="58" height="46" rx="12" fill="url(#imBlue)" />
        <g fill="#fff" opacity=".35">
          {[0, 1, 2].flatMap((r) => [0, 1, 2, 3].map((c) => <circle key={`l${r}${c}`} cx={90 + c * 13} cy={332 + r * 12} r="2" />))}
          {[0, 1, 2].flatMap((r) => [0, 1, 2, 3].map((c) => <circle key={`r${r}${c}`} cx={400 + c * 13} cy={332 + r * 12} r="2" />))}
        </g>
        <rect x="190" y="326" width="140" height="22" rx="11" fill="#e8f0fb" stroke="#9fb0c8" />
        <rect x="236" y="334" width="48" height="5" rx="2.5" fill="url(#imOrange)" />
        <circle cx="352" cy="356" r="6" fill="#2d63e8" stroke="#1b44ad" />
        <circle cx="378" cy="338" r="11" fill="url(#imOrange)" stroke="#b85a00" />
        <text x="260" y="316" textAnchor="middle" fontFamily="Georgia, serif" fontStyle="italic" fontSize="14" fill="#4a5568">
          DiskTemizle
        </text>
        {/* Sap ve ayak */}
        <path d="M184 402 V 430 Q184 450 204 450 H316 Q336 450 336 430 V 402" fill="none" stroke="#1d47b8" strokeWidth="14" strokeLinecap="round" />
        <ellipse cx="190" cy="462" rx="18" ry="8" fill="#e8eefb" stroke="#9fb0c8" />
        <ellipse cx="330" cy="462" rx="18" ry="8" fill="#e8eefb" stroke="#9fb0c8" />
        {/* Klavye */}
        <rect x="40" y="498" width="330" height="96" rx="12" fill="#e9f1fd" stroke="#8fa8d4" strokeWidth="1.5" />
        <rect x="40" y="580" width="330" height="14" rx="7" fill="url(#imBlue)" />
        {keys}
        <circle cx="52" cy="506" r="3" fill="url(#imOrange)" />
        {/* Fare ve kablosu */}
        <path d="M440 470 C 430 440, 480 420, 452 372" fill="none" stroke="#a9bcdc" strokeWidth="2" />
        <ellipse cx="440" cy="520" rx="40" ry="30" fill="url(#imBody)" stroke="#8fa8d4" strokeWidth="1.5" />
        <ellipse cx="440" cy="532" rx="30" ry="16" fill="#3a6df0" opacity=".28" />
        <circle cx="440" cy="528" r="5" fill="url(#imOrange)" />
      </svg>
      <div
        ref={screenRef}
        className="absolute"
        style={{ left: `${(120 / 520) * 100}%`, top: `${(62 / 620) * 100}%`, width: `${(280 / 520) * 100}%`, height: `${(224 / 620) * 100}%` }}
      >
        <MiniDesktop />
      </div>
    </div>
  );
}

/**
 * Açılış: siyah yükleme ekranı -> iMac sahnesi ("tıkla gir") -> ekrana yakınlaşma -> masaüstü.
 */
export function Splash({ onDone }: { onDone: () => void }) {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>("loading");
  const [progress, setProgress] = useState(0);
  const [zoom, setZoom] = useState<{ s: number; dx: number; dy: number; ox: number; oy: number } | null>(null);
  const sceneRef = useRef<HTMLDivElement>(null);
  const screenRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const start = performance.now();
    let raf = 0;
    const tick = (now: number) => {
      const p = Math.min(1, (now - start) / LOAD_MS);
      setProgress(p);
      if (p < 1) raf = requestAnimationFrame(tick);
      else setPhase("ready");
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, []);

  const enter = useCallback(() => {
    if (phase !== "ready" || !screenRef.current || !sceneRef.current) return;
    const sr = screenRef.current.getBoundingClientRect();
    const sc = sceneRef.current.getBoundingClientRect();
    const [vw, vh] = [window.innerWidth, window.innerHeight];
    const cx = sr.left + sr.width / 2;
    const cy = sr.top + sr.height / 2;
    setZoom({
      s: Math.max(vw / sr.width, vh / sr.height),
      dx: vw / 2 - cx,
      dy: vh / 2 - cy,
      ox: cx - sc.left,
      oy: cy - sc.top,
    });
    setPhase("zoom");
    window.setTimeout(onDone, ZOOM_MS);
  }, [phase, onDone]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => (e.key === "Enter" || e.key === " ") && enter();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enter]);

  const loading = phase === "loading";
  const zooming = phase === "zoom";

  return (
    <div className="fixed inset-0 z-[200] overflow-hidden" role="presentation">
      {/* Arka plan: siyahtan açık maviye geçer, yakınlaşırken solar */}
      <div
        className="absolute inset-0 transition-[opacity,background] duration-700"
        style={{
          background: loading ? "#000" : "radial-gradient(ellipse at 50% 35%, #7fd0ff, #2a9be8 55%, #0f6ec4)",
          opacity: zooming ? 0 : 1,
          transitionDuration: zooming ? `${ZOOM_MS * 0.6}ms` : "700ms",
        }}
      />

      {loading && (
        <div className="absolute inset-0 grid place-items-center" role="progressbar" aria-label={t("splash.loading")} aria-valuenow={Math.round(progress * 100)}>
          <div className="flex flex-col items-center gap-6 text-white">
            <ShieldCheck size={72} strokeWidth={1.6} />
            <div className="h-1 w-40 overflow-hidden rounded-full bg-white/20">
              <div className="h-full rounded-full bg-white" style={{ width: `${progress * 100}%` }} />
            </div>
          </div>
        </div>
      )}

      {!loading && (
        <button
          type="button"
          onClick={enter}
          aria-label={t("splash.enter")}
          className="absolute inset-0 flex cursor-pointer flex-col items-center justify-center gap-6 outline-none"
        >
          <div
            ref={sceneRef}
            style={{
              transform: zoom ? `translate(${zoom.dx}px, ${zoom.dy}px) scale(${zoom.s})` : "none",
              transformOrigin: zoom ? `${zoom.ox}px ${zoom.oy}px` : "center",
              transition: zooming ? `transform ${ZOOM_MS}ms cubic-bezier(.65,0,.9,.55)` : "none",
              animation: !zooming ? "imac-in .7s cubic-bezier(.2,.8,.25,1)" : undefined,
            }}
          >
            <Imac screenRef={screenRef} />
          </div>
          <span
            className="text-[12px] tracking-[0.35em] text-white/80 transition-opacity duration-300"
            style={{ opacity: zooming ? 0 : 1, animation: "enter-pulse 2s ease-in-out infinite" }}
          >
            {t("splash.enter")}
          </span>
        </button>
      )}
    </div>
  );
}
