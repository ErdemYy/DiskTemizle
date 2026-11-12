import { Loader2, RefreshCw, Undo2 } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import type { JournalEntry } from "../lib/api";
import { formatBytes } from "../lib/format";
import { CATEGORY_META } from "../lib/meta";
import type { CategoryId } from "../lib/types";
import { CATEGORIES } from "../lib/types";
import { useStore, type Win } from "../store";
import { Window } from "./Window";

type Filter = "all" | "cleaned" | "problems";

const ACTION_TONE: Record<JournalEntry["action"], string> = {
  trashed: "bg-[#3b82f6]",
  deleted: "bg-[#c0392b]",
  skipped: "bg-[#8a8f98]",
  failed: "bg-[#d97706]",
  restored: "bg-[#2fa62a]",
};

const isCategory = (c: string): c is CategoryId => (CATEGORIES as string[]).includes(c);
const baseName = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() ?? p;
const dirName = (p: string) => p.slice(0, Math.max(0, p.length - baseName(p).length - 1));

/** İşlem günlüğü: ne zaman, neyin, neden taşındığı/silindiği. Geri alınabilir kayıtlarda "Geri Al" vardır. */
export function HistoryWindow({ win }: { win: Win }) {
  const { t, i18n } = useTranslation();
  const { history, loadHistory, undoEntry } = useStore();
  const [filter, setFilter] = useState<Filter>("all");
  const [loading, setLoading] = useState(true);
  const [busyPath, setBusyPath] = useState<string | null>(null);

  const reload = async () => {
    setLoading(true);
    await loadHistory();
    setLoading(false);
  };
  useEffect(() => {
    void reload();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const rows = useMemo(
    () =>
      history.filter((e) =>
        filter === "all" ? true : filter === "cleaned" ? e.action === "trashed" || e.action === "deleted" : e.action === "skipped" || e.action === "failed",
      ),
    [history, filter],
  );
  const freed = rows.filter((e) => e.action === "trashed" || e.action === "deleted").reduce((s, e) => s + e.bytes, 0);
  const fmt = (n: number) => formatBytes(n, i18n.language);
  const when = (ts: number) => new Date(ts * 1000).toLocaleString(i18n.language, { dateStyle: "short", timeStyle: "short" });

  const undo = async (e: JournalEntry) => {
    setBusyPath(e.path);
    await undoEntry(e.path, e.category);
    setBusyPath(null);
  };

  const cols = "grid-cols-[112px_140px_150px_1fr_76px_92px]";

  return (
    <Window win={win} title={t("hist.title")}>
      <div className="flex shrink-0 items-center gap-3 border-b border-[#b4b4b4] bg-gradient-to-b from-[#f2f2f2] to-[#dcdcdc] px-3 py-1.5">
        <button className="aqua-btn small flex items-center gap-1.5" onClick={() => void reload()} disabled={loading}>
          {loading ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
          {t("hist.refresh")}
        </button>
        <div className="aqua-seg" role="radiogroup">
          {(["all", "cleaned", "problems"] as Filter[]).map((f) => (
            <button key={f} role="radio" aria-checked={filter === f} onClick={() => setFilter(f)}>
              {t(f === "all" ? "hist.filterAll" : f === "cleaned" ? "hist.filterCleaned" : "hist.filterProblems")}
            </button>
          ))}
        </div>
        <div className="flex-1" />
        <span className="text-xs text-[#444]">{t("hist.count", { count: rows.length })}</span>
      </div>

      <div className="relative min-h-0 flex-1 overflow-auto bg-white">
        {rows.length === 0 && !loading && (
          <div className="absolute inset-0 grid place-items-center text-xs text-[#666]">{t("hist.empty")}</div>
        )}
        {rows.length > 0 && (
          <div role="table" className="text-xs">
            <div className={`aqua-list-header sticky top-0 z-10 grid ${cols} items-center py-1 pr-2 pl-3 font-medium`}>
              <span>{t("hist.colTime")}</span>
              <span>{t("hist.colCategory")}</span>
              <span>{t("hist.colAction")}</span>
              <span>{t("hist.colFile")}</span>
              <span className="text-right">{t("win.colSize")}</span>
              <span />
            </div>
            {rows.map((e, i) => {
              const cat = isCategory(e.category) ? CATEGORY_META[e.category] : null;
              const Icon = cat?.icon;
              return (
                <div key={`${e.ts}-${i}-${e.path}`} role="row" title={e.detail ? `${e.path}\n${e.detail}` : e.path} className={`aqua-row grid ${cols} items-center py-1.5 pr-2 pl-3`}>
                  <span className="tabular-nums text-[#444]">{when(e.ts)}</span>
                  <span className="flex min-w-0 items-center gap-1.5">
                    {Icon && <Icon size={13} style={{ color: cat?.color }} className="shrink-0" />}
                    <span className="truncate">{isCategory(e.category) ? t(`cat.${e.category}.name`) : t("hist.undoCategory")}</span>
                  </span>
                  <span>
                    <span className={`rounded px-1.5 py-0.5 text-[10px] text-white ${ACTION_TONE[e.action]}`}>{t(`hist.action.${e.action}`)}</span>
                  </span>
                  <span className="min-w-0">
                    <span className="block truncate">{baseName(e.path)}</span>
                    <span className="block truncate text-[10px] text-[#888]">{e.detail ? `${dirName(e.path)} · ${e.detail}` : dirName(e.path)}</span>
                  </span>
                  <span className="text-right tabular-nums">{e.bytes > 0 ? fmt(e.bytes) : ""}</span>
                  <span className="text-right">
                    {e.undoable && (
                      <button className="aqua-btn small ml-auto flex items-center gap-1 whitespace-nowrap" onClick={() => void undo(e)} disabled={busyPath === e.path} aria-label={`${t("hist.undo")} ${baseName(e.path)}`}>
                        {busyPath === e.path ? <Loader2 size={11} className="animate-spin" /> : <Undo2 size={11} />}
                        {t("hist.undo")}
                      </button>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="flex shrink-0 items-center justify-between border-t border-[#b4b4b4] bg-gradient-to-b from-[#f2f2f2] to-[#dcdcdc] px-3 py-1.5 text-xs text-[#333]">
        <span>{filter === "problems" ? "" : t("hist.freedTotal", { size: fmt(freed) })}</span>
      </div>
    </Window>
  );
}
