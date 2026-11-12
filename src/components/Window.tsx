import { useTranslation } from "react-i18next";
import { DOCK_SPACE, MENUBAR_H, activeWindowId, useStore, type Win } from "../store";

interface Props {
  win: Win;
  title: string;
  children: React.ReactNode;
  resizable?: boolean;
  /** Pencere içi katman (ör. onay sayfası) için taşıyıcı olarak görünür. */
  className?: string;
}

const PHASE_CLASS: Record<Win["phase"], string> = {
  open: "window-opening",
  closing: "window-closing",
  minimizing: "window-minimizing",
  minimized: "hidden",
};

export function Window({ win, title, children, resizable = true, className }: Props) {
  const { t } = useTranslation();
  const { focusWindow, moveWindow, resizeWindow, closeWindow, minimizeWindow, toggleZoom, finishPhase, windows } = useStore();
  const active = activeWindowId(windows) === win.id;

  const onTitleDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if ((e.target as HTMLElement).closest("[data-traffic]")) return;
    focusWindow(win.id);
    if (win.zoomed) return;
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const [sx, sy, ox, oy] = [e.clientX, e.clientY, win.x, win.y];
    const move = (ev: PointerEvent) => moveWindow(win.id, ox + ev.clientX - sx, oy + ev.clientY - sy);
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  };

  const onResizeDown = (e: React.PointerEvent<HTMLDivElement>) => {
    e.stopPropagation();
    focusWindow(win.id);
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const [sx, sy, ow, oh] = [e.clientX, e.clientY, win.w, win.h];
    const move = (ev: PointerEvent) => resizeWindow(win.id, ow + ev.clientX - sx, oh + ev.clientY - sy);
    const up = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", up);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", up);
  };

  const minimize = () => {
    // Küçülme animasyonu pencerenin Dock'taki ikonuna doğru akar.
    const target = document.querySelector(`[data-dock="${win.id}"]`)?.getBoundingClientRect();
    const dx = target ? target.left + target.width / 2 - (win.x + win.w / 2) : 0;
    const dy = target ? target.top - (win.y + win.h / 2) : window.innerHeight * 0.55;
    minimizeWindow(win.id, dx, dy);
  };

  const style: React.CSSProperties = win.zoomed
    ? { left: 8, top: MENUBAR_H + 6, width: "calc(100% - 16px)", height: `calc(100% - ${MENUBAR_H + 6 + DOCK_SPACE}px)`, zIndex: win.z }
    : { left: win.x, top: win.y, width: win.w, height: win.h, zIndex: win.z };
  Object.assign(style, { "--min-dx": `${win.minDx}px`, "--min-dy": `${win.minDy}px` });

  return (
    <div
      className={`absolute flex flex-col ${PHASE_CLASS[win.phase]} aqua-window ${active ? "" : "inactive inactive-win"} ${className ?? ""}`}
      style={style}
      onPointerDown={() => focusWindow(win.id)}
      onAnimationEnd={() => (win.phase === "closing" || win.phase === "minimizing") && finishPhase(win.id)}
      role="dialog"
      aria-label={title}
    >
      <div className={`aqua-titlebar shrink-0 ${active ? "active" : ""}`} onPointerDown={onTitleDown} onDoubleClick={() => toggleZoom(win.id)}>
        <div className="traffics absolute top-1/2 left-2 z-10 flex -translate-y-1/2 gap-2" data-traffic>
          <button className="traffic close" aria-label={t("notice.close")} onClick={() => closeWindow(win.id)}>
            ×
          </button>
          <button className="traffic min" aria-label="Minimize" onClick={minimize}>
            −
          </button>
          <button className="traffic zoom" aria-label="Zoom" onClick={() => toggleZoom(win.id)}>
            +
          </button>
        </div>
        <div className="aqua-title">{title}</div>
      </div>
      <div className="relative flex min-h-0 flex-1 flex-col">{children}</div>
      {resizable && !win.zoomed && (
        <div
          onPointerDown={onResizeDown}
          className="absolute right-0 bottom-0 z-20 h-4 w-4 cursor-nwse-resize"
          style={{ background: "linear-gradient(135deg, transparent 55%, rgba(0,0,0,.35) 55% 60%, transparent 60% 70%, rgba(0,0,0,.35) 70% 75%, transparent 75%)" }}
        />
      )}
    </div>
  );
}
