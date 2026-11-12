import { Fragment } from "react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "../lib/format";
import { CATEGORY_META } from "../lib/meta";
import type { CategoryId, Item } from "../lib/types";
import { useStore } from "../store";
import { FileIcon } from "./icons";

const DAY = 24 * 3600;

function useAgeText() {
  const { t } = useTranslation();
  return (secs: number) => {
    if (secs < DAY) return t("age.today");
    if (secs < 60 * DAY) return t("age.days", { count: Math.floor(secs / DAY) });
    if (secs < 730 * DAY) return t("age.months", { count: Math.floor(secs / (30 * DAY)) });
    return t("age.years", { count: Math.floor(secs / (365 * DAY)) });
  };
}

const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const dirName = (p: string) => p.slice(0, Math.max(0, p.length - baseName(p).length - 1));

/**
 * Büyük ve yinelenen dosyalar için liste. Hiçbir şey önceden seçili değildir; öneriler rozetle
 * gösterilir ve "Önerilenleri Seç" ile işaretlenir.
 */
export function UserFileList({ id, items }: { id: CategoryId; items: Item[] }) {
  const { t, i18n } = useTranslation();
  const { selected, toggle, selectRecommended } = useStore();
  const sel = selected[id];
  const ageText = useAgeText();
  const { color } = CATEGORY_META[id];
  const fmt = (n: number) => formatBytes(n, i18n.language);

  const recommended = items.filter((i) => i.recommended);
  const allRecOn = recommended.length > 0 && recommended.every((i) => sel.has(i.path));
  const grouped = id === "duplicates";

  const cols = "grid-cols-[28px_1fr_170px_64px_84px]";

  return (
    <div role="table" className="text-xs">
      <div className={`aqua-list-header sticky top-0 z-10 grid ${cols} items-center py-1 pr-2 font-medium`}>
        <span className="grid place-items-center">
          <input
            type="checkbox"
            checked={allRecOn}
            disabled={recommended.length === 0}
            onChange={() => selectRecommended(id, !allRecOn)}
            aria-label={t("win.selectRecommended")}
            title={t("win.selectRecommended")}
          />
        </span>
        <span>{t("win.colName")}</span>
        <span>{t("win.colReason")}</span>
        <span className="text-right">{t("win.colAge")}</span>
        <span className="text-right">{t("win.colSize")}</span>
      </div>

      {items.map((item, idx) => {
        const on = sel.has(item.path);
        const newGroup = grouped && (idx === 0 || items[idx - 1].group !== item.group);
        return (
          <Fragment key={item.path}>
            {newGroup && (
              <div className="border-b border-[#d0d0d0] bg-[#e4e9f2] px-3 py-0.5 text-[11px] font-medium text-[#445]">
                {t("win.groupHeader", {
                  count: items.filter((i) => i.group === item.group).length,
                  size: fmt(item.size_bytes),
                })}
              </div>
            )}
            <div
              role="row"
              onClick={() => toggle(id, item.path)}
              title={item.path}
              className={`aqua-row grid ${cols} cursor-default items-center py-1 pr-2 ${on ? "selected" : ""}`}
            >
              <span className="grid place-items-center">
                <input
                  type="checkbox"
                  checked={on}
                  onChange={() => toggle(id, item.path)}
                  onClick={(e) => e.stopPropagation()}
                  aria-label={baseName(item.path)}
                />
              </span>
              <span className="flex min-w-0 items-center gap-2">
                <span className="shrink-0">
                  <FileIcon size={18} />
                </span>
                <span className="min-w-0">
                  <span className="block truncate">{baseName(item.path)}</span>
                  <span className={`block truncate text-[10px] ${on ? "text-white/70" : "text-[#888]"}`}>{dirName(item.path)}</span>
                </span>
              </span>
              <span className="flex min-w-0 flex-col gap-0.5 pr-2">
                <span className="flex items-center gap-1">
                  {item.keep ? (
                    <span className="rounded bg-[#3b82f6] px-1 text-[10px] text-white">{t("win.keepCopy")}</span>
                  ) : item.recommended ? (
                    <span className="rounded bg-[#2fa62a] px-1 text-[10px] text-white">{t("win.recommended")}</span>
                  ) : null}
                  <span className="truncate">{item.reason ? t(`reason.${item.reason}`) : ""}</span>
                </span>
                {!grouped && id !== "recycle_bin" && (
                  <span className="h-1 overflow-hidden rounded-full bg-black/15">
                    <span className="block h-full rounded-full" style={{ width: `${item.score}%`, background: item.recommended ? "#2fa62a" : color }} />
                  </span>
                )}
              </span>
              <span className="text-right tabular-nums">{ageText(item.age_secs)}</span>
              <span className="text-right tabular-nums">{fmt(item.size_bytes)}</span>
            </div>
          </Fragment>
        );
      })}
    </div>
  );
}
