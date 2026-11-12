import { ShieldCheck } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { CATEGORIES, isUserFiles } from "../lib/types";
import { activeWindowId, useStore } from "../store";

interface Item {
  label: string;
  shortcut?: string;
  onClick?: () => void;
  disabled?: boolean;
  separator?: boolean;
  check?: boolean;
}
interface Menu {
  key: string;
  label: string;
  bold?: boolean;
  logo?: boolean;
  items: Item[];
}

function useClock(lang: string) {
  const fmt = () => new Date().toLocaleString(lang, { weekday: "short", hour: "2-digit", minute: "2-digit" });
  const [t, setT] = useState(fmt);
  useEffect(() => {
    setT(fmt());
    const id = setInterval(() => setT(fmt()), 20000);
    return () => clearInterval(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [lang]);
  return t;
}

export function MenuBar() {
  const { t, i18n } = useTranslation();
  const s = useStore();
  const [open, setOpen] = useState<string | null>(null);
  const clock = useClock(i18n.language);

  const activeId = activeWindowId(s.windows);
  const activeCat = activeId && (CATEGORIES as string[]).includes(activeId) ? (activeId as (typeof CATEGORIES)[number]) : null;
  const activeView = activeId ? s.windows[activeId]?.view : undefined;

  const menus: Menu[] = useMemo(
    () => [
      {
        key: "logo",
        label: "",
        logo: true,
        items: [
          { label: t("menu.about"), onClick: () => s.openWindow("about") },
          { label: "", separator: true },
          { label: t("menu.settings"), shortcut: "Ctrl+,", onClick: () => s.openWindow("settings") },
        ],
      },
      {
        key: "app",
        label: t("app.name"),
        bold: true,
        items: [
          { label: t("menu.scanAll"), shortcut: "Ctrl+R", onClick: () => void s.scanAll() },
          { label: "", separator: true },
          { label: t("menu.settings"), shortcut: "Ctrl+,", onClick: () => s.openWindow("settings") },
        ],
      },
      {
        key: "file",
        label: t("menu.file"),
        items: [
          { label: t("menu.rescan"), disabled: !activeCat, onClick: () => activeCat && void s.scan(activeCat) },
          { label: t("menu.cleanSelected"), disabled: !activeCat || s.selected[activeCat].size === 0, onClick: () => window.dispatchEvent(new CustomEvent("clean-request", { detail: activeCat })) },
        ],
      },
      {
        key: "view",
        label: t("menu.view"),
        items: [
          { label: t("menu.asIcons"), check: activeView === "icon", disabled: !activeCat, onClick: () => activeId && s.setWinView(activeId, "icon") },
          { label: t("menu.asList"), check: activeView === "list", disabled: !activeCat, onClick: () => activeId && s.setWinView(activeId, "list") },
          { label: "", separator: true },
          { label: t("menu.selectRecommended"), disabled: !activeCat || !isUserFiles(activeCat), onClick: () => activeCat && s.selectRecommended(activeCat, true) },
          { label: t("menu.selectAll"), disabled: !activeCat, onClick: () => activeCat && s.toggleAll(activeCat, true) },
          { label: t("menu.selectNone"), disabled: !activeCat, onClick: () => activeCat && s.toggleAll(activeCat, false) },
        ],
      },
      {
        key: "go",
        label: t("menu.go"),
        items: [
          ...CATEGORIES.map((c) => ({ label: t(`cat.${c}.name`), onClick: () => s.openWindow(c) })),
          { label: "", separator: true },
          { label: t("menu.history"), onClick: () => s.openWindow("history") },
        ],
      },
    ],
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [t, s.windows, s.selected, activeCat, activeId, activeView],
  );

  useEffect(() => {
    if (!open) return;
    const close = (e: PointerEvent) => {
      if (!(e.target as HTMLElement).closest("[data-menubar]")) setOpen(null);
    };
    window.addEventListener("pointerdown", close);
    return () => window.removeEventListener("pointerdown", close);
  }, [open]);

  return (
    <div
      data-menubar
      className="aqua-menubar fixed inset-x-0 top-0 z-[100] flex items-stretch justify-between"
      style={{ height: "var(--menubar-h)" }}
    >
      <div className="flex items-stretch">
        {menus.map((m) => (
          <div key={m.key} className="relative">
            <button
              className={`aqua-menu-item ${open === m.key ? "active" : ""} ${m.bold ? "font-bold" : ""}`}
              onClick={() => setOpen(open === m.key ? null : m.key)}
              onPointerEnter={() => open && setOpen(m.key)}
              aria-haspopup="menu"
              aria-expanded={open === m.key}
            >
              {m.logo ? <ShieldCheck size={15} strokeWidth={2.4} /> : m.label}
            </button>
            {open === m.key && (
              <div role="menu" className="aqua-dropdown absolute top-full left-0">
                {m.items.map((it, i) =>
                  it.separator ? (
                    <div key={i} className="my-1 h-px bg-black/15" />
                  ) : (
                    <button
                      key={i}
                      role="menuitem"
                      disabled={it.disabled}
                      className={`aqua-dropdown-item w-full text-left ${it.disabled ? "disabled" : ""}`}
                      onClick={() => {
                        setOpen(null);
                        it.onClick?.();
                      }}
                    >
                      <span>
                        {it.check ? "✓ " : ""}
                        {it.label}
                      </span>
                      {it.shortcut && <span className="opacity-60">{it.shortcut}</span>}
                    </button>
                  ),
                )}
              </div>
            )}
          </div>
        ))}
      </div>
      <div className="flex items-center px-3 text-[13px]">{clock}</div>
    </div>
  );
}
