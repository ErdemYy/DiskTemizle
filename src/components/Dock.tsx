import { History, Loader2, ScanSearch, Settings, type LucideIcon } from "lucide-react";
import { Fragment, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { CATEGORY_META } from "../lib/meta";
import { CATEGORIES } from "../lib/types";
import { useStore, type WinId } from "../store";
import { AquaTile } from "./icons";

const BASE = 48;
const MAX = 80;

/**
 * Büyütme eğrisi: imleç ikonun merkezindeyken MAX, uzaklaştıkça BASE'e iner.
 * Noktalar gerçek Dock'tan ölçülen komşu boyutlarına göre ayarlandı.
 */
const FALLOFF: [number, number][] = [
  [0, 1],
  [56, 0.625],
  [112, 0.253],
  [170, 0.09],
  [240, 0],
];

function falloff(distance: number): number {
  const d = Math.abs(distance);
  for (let i = 1; i < FALLOFF.length; i++) {
    const [d1, v1] = FALLOFF[i];
    if (d <= d1) {
      const [d0, v0] = FALLOFF[i - 1];
      return v0 + ((v1 - v0) * (d - d0)) / (d1 - d0);
    }
  }
  return 0;
}

interface DockItem {
  key: string;
  label: string;
  glyph: LucideIcon;
  color: string;
  winId?: WinId;
  onClick: () => void;
  busy?: boolean;
  separatorBefore?: boolean;
}

export function Dock() {
  const { t } = useTranslation();
  const { windows, openWindow, scanAll, scanning } = useStore();
  const [mouseX, setMouseX] = useState<number | null>(null);
  const [hover, setHover] = useState<string | null>(null);
  const [bouncing, setBouncing] = useState<string | null>(null);
  const restCenters = useRef<number[]>([]);
  const itemRefs = useRef<(HTMLDivElement | null)[]>([]);

  const scanningAny = CATEGORIES.some((c) => scanning[c]);

  const items: DockItem[] = [
    { key: "scan", label: t("dock.scanAll"), glyph: ScanSearch, color: "#22a6e0", onClick: () => void scanAll(), busy: scanningAny },
    ...CATEGORIES.map<DockItem>((c) => ({
      key: c,
      label: t(`cat.${c}.name`),
      glyph: CATEGORY_META[c].icon,
      color: CATEGORY_META[c].color,
      winId: c,
      onClick: () => openWindow(c),
    })),
    { key: "history", label: t("dock.history"), glyph: History, color: "#5b6b86", winId: "history", separatorBefore: true, onClick: () => openWindow("history") },
    { key: "settings", label: t("dock.settings"), glyph: Settings, color: "#8e949c", winId: "settings", onClick: () => openWindow("settings") },
  ];

  // Dinlenme konumları imleç dock'a girdiği anda (tüm ikonlar BASE iken) ölçülür.
  const measure = () => {
    restCenters.current = itemRefs.current.map((el) => {
      if (!el) return 0;
      const r = el.getBoundingClientRect();
      return r.left + r.width / 2;
    });
  };

  const sizeOf = (i: number) =>
    mouseX === null ? BASE : BASE + (MAX - BASE) * falloff(mouseX - (restCenters.current[i] ?? 0));

  const activate = (it: DockItem) => {
    setBouncing(it.key);
    window.setTimeout(() => setBouncing((b) => (b === it.key ? null : b)), 900);
    it.onClick();
  };

  return (
    <div className="pointer-events-none fixed inset-x-0 bottom-0 z-[90] flex justify-center">
      <div className="pointer-events-auto pb-2">
        <div
          className="aqua-dock-container flex items-end gap-2 px-4 pt-3 pb-2"
          style={{ height: 64 }}
          onPointerEnter={(e) => {
            measure();
            setMouseX(e.clientX);
          }}
          onPointerMove={(e) => setMouseX(e.clientX)}
          onPointerLeave={() => {
            setMouseX(null);
            setHover(null);
          }}
        >
          {items.map((it, i) => {
            const size = sizeOf(i);
            const running = it.winId && windows[it.winId];
            return (
              <Fragment key={it.key}>
                {it.separatorBefore && <div className="mx-2 h-10 w-px self-center rounded-full bg-white/30" />}
                <div
                  ref={(el) => {
                    itemRefs.current[i] = el;
                  }}
                  data-dock={it.winId ?? it.key}
                  className="relative flex flex-col items-center"
                  onPointerEnter={() => setHover(it.key)}
                  onPointerLeave={() => setHover((h) => (h === it.key ? null : h))}
                >
                  {hover === it.key && (
                    <div className="pointer-events-none absolute -top-10 z-20 rounded-full bg-black/60 px-3 py-1 text-xs whitespace-nowrap text-white backdrop-blur-sm">
                      {it.label}
                    </div>
                  )}
                  <button
                    type="button"
                    aria-label={it.label}
                    onClick={() => activate(it)}
                    className={`z-10 flex cursor-pointer items-center justify-center ${bouncing === it.key ? "dock-bounce" : ""}`}
                    style={{ width: size, height: size, transition: mouseX === null ? "width .15s, height .15s" : "none" }}
                  >
                    {it.busy ? (
                      <div className="relative grid place-items-center">
                        <AquaTile color={it.color} glyph={it.glyph} size={size} />
                        <Loader2 size={size * 0.3} className="absolute animate-spin text-white" />
                      </div>
                    ) : (
                      <AquaTile color={it.color} glyph={it.glyph} size={size} />
                    )}
                  </button>
                  <div
                    className="absolute -bottom-1.5 size-1 rounded-full bg-black/60"
                    style={{ opacity: running ? 1 : 0 }}
                  />
                </div>
              </Fragment>
            );
          })}
        </div>
      </div>
    </div>
  );
}
