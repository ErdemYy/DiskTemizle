import { AlertTriangle, LayoutGrid, List, Loader2, RefreshCw, ShieldCheck, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "../lib/format";
import { CATEGORY_META } from "../lib/meta";
import type { CategoryId, Item } from "../lib/types";
import { PERMANENT_ALLOWED, isIrreversible, isUserFiles, usesFileList } from "../lib/types";
import { useStore, type Win } from "../store";
import { AquaTile, FileIcon, FolderIcon } from "./icons";
import { UserFileList } from "./UserFileList";
import { Window } from "./Window";

function labelOf(path: string) {
  const parts = path.split(/[\\/]/).filter(Boolean);
  return { name: parts[parts.length - 1] ?? path, parent: parts[parts.length - 2] ?? "" };
}

/** Ayırıcı karakterlerden sonra satır kırılmasına izin verir (uzun dosya adları için). */
const breakable = (s: string) => s.replace(/([_.\-])/g, "$1​");

const looksLikeFile = (i: Item) => i.file_count === 1 && /\.[a-z0-9]{1,5}$/i.test(labelOf(i.path).name);

export function CategoryWindow({ id, win }: { id: CategoryId; win: Win }) {
  const { t, i18n } = useTranslation();
  const { scans, scanning, progress, selected, scan, toggle, toggleAll, setWinView, selectRecommended } = useStore();
  const [confirming, setConfirming] = useState(false);
  // Büyük/yinelenen dosyalar ve Geri Dönüşüm Kutusu ortak "öneri listesi" görünümünü kullanır.
  const userFiles = usesFileList(id);

  const prog = progress[id];
  const progText = !prog
    ? t("win.scanning")
    : prog.phase === "discover"
      ? t("win.discovering", { count: prog.done })
      : t("win.measuring", { done: prog.done, total: prog.total });
  // Arama aşamasında toplam bilinmez: dolu, hareketli çubuk. Boyut aşamasında gerçek oran.
  const progPct = prog?.phase === "measure" && prog.total > 0 ? (prog.done / prog.total) * 100 : 100;

  const result = scans[id];
  const busy = !!scanning[id];
  const sel = selected[id];
  const items = result?.items ?? [];
  const selBytes = items.filter((i) => sel.has(i.path)).reduce((s, i) => s + i.size_bytes, 0);
  const allOn = items.length > 0 && sel.size === items.length;
  const { icon: Glyph, color } = CATEGORY_META[id];
  const fmt = (n: number) => formatBytes(n, i18n.language);

  // Menü çubuğundaki "Seçilenleri Temizle…" komutu.
  useEffect(() => {
    const h = (e: Event) => (e as CustomEvent).detail === id && setConfirming(true);
    window.addEventListener("clean-request", h);
    return () => window.removeEventListener("clean-request", h);
  }, [id]);

  return (
    <Window win={win} title={t(`cat.${id}.name`)}>
      {/* Araç çubuğu */}
      <div className="flex shrink-0 items-center gap-3 border-b border-[#b4b4b4] bg-gradient-to-b from-[#f2f2f2] to-[#dcdcdc] px-3 py-1.5">
        <button className="aqua-btn small flex items-center gap-1.5" onClick={() => void scan(id)} disabled={busy} title={t("win.rescan")}>
          {busy ? <Loader2 size={13} className="animate-spin" /> : <RefreshCw size={13} />}
          {result ? t("win.rescan") : t("win.scan")}
        </button>
        {userFiles ? (
          <button
            className="aqua-btn small"
            disabled={!items.some((i) => i.recommended)}
            onClick={() => selectRecommended(id, true)}
          >
            {t("win.selectRecommended")}
          </button>
        ) : (
          <div className="aqua-seg" role="radiogroup">
            <button role="radio" aria-checked={win.view === "icon"} aria-label={t("menu.asIcons")} onClick={() => setWinView(id, "icon")}>
              <LayoutGrid size={13} />
            </button>
            <button role="radio" aria-checked={win.view === "list"} aria-label={t("menu.asList")} onClick={() => setWinView(id, "list")}>
              <List size={13} />
            </button>
          </div>
        )}
        <div className="flex-1" />
        <span className="text-xs text-[#444]">{result ? t("win.items", { count: items.length }) : ""}</span>
      </div>

      {/* İçerik */}
      <div className="relative min-h-0 flex-1 overflow-auto bg-white">
        {busy && !result && (
          <Center>
            <div className="w-56">
              <div className="aqua-progress">
                <i style={{ width: `${progPct}%`, transition: "width .2s" }} />
              </div>
              <div className="mt-2 text-center text-xs text-[#555] tabular-nums">{progText}</div>
            </div>
          </Center>
        )}
        {!result && !busy && <Center><span className="text-xs text-[#666]">{t("win.notScanned")}</span></Center>}
        {result && items.length === 0 && <Center><span className="text-xs text-[#666]">{t("win.empty")}</span></Center>}

        {items.length > 0 && userFiles && <UserFileList id={id} items={items} />}

        {items.length > 0 && !userFiles && win.view === "icon" && (
          <div className="grid grid-cols-[repeat(auto-fill,minmax(104px,1fr))] gap-1 p-3">
            {items.map((item) => {
              const on = sel.has(item.path);
              const { name, parent } = labelOf(item.path);
              return (
                <button
                  key={item.path}
                  onClick={() => toggle(id, item.path)}
                  title={item.path}
                  aria-pressed={on}
                  className="flex flex-col items-center gap-1 rounded-md p-2 text-center"
                >
                  <div className={`relative rounded-md p-1.5 ${on ? "bg-[#3b82f6]/25 ring-1 ring-[#3b82f6]/60" : ""}`}>
                    {looksLikeFile(item) ? <FileIcon size={48} /> : <FolderIcon color={color} glyph={Glyph} size={56} />}
                    {item.recently_modified && (
                      <span title={t("win.inUse")} className="absolute -top-1 -right-1 grid size-4 place-items-center rounded-full bg-[#f59e0b] text-white shadow">
                        <AlertTriangle size={10} />
                      </span>
                    )}
                  </div>
                  <span
                    className={`line-clamp-2 max-w-full rounded px-1.5 text-[11px] leading-tight [overflow-wrap:anywhere] ${on ? "bg-[#3b82f6] text-white" : ""}`}
                  >
                    {breakable(name)}
                  </span>
                  <span className="text-[10px] text-[#666]">{parent && <>{parent} · </>}{fmt(item.size_bytes)}</span>
                </button>
              );
            })}
          </div>
        )}

        {items.length > 0 && !userFiles && win.view === "list" && (
          <div role="table" className="text-xs">
            <div className="aqua-list-header sticky top-0 z-10 grid grid-cols-[28px_1fr_90px_70px] items-center py-1 pr-2 font-medium">
              <span className="grid place-items-center">
                <input type="checkbox" checked={allOn} onChange={() => toggleAll(id, !allOn)} aria-label={t("menu.selectAll")} />
              </span>
              <span>{t("win.colName")}</span>
              <span className="text-right">{t("win.colSize")}</span>
              <span className="text-right">{t("win.colFiles")}</span>
            </div>
            {items.map((item) => {
              const on = sel.has(item.path);
              const { name, parent } = labelOf(item.path);
              return (
                <div
                  key={item.path}
                  role="row"
                  onClick={() => toggle(id, item.path)}
                  title={item.path}
                  className={`aqua-row grid cursor-default grid-cols-[28px_1fr_90px_70px] items-center py-1 pr-2 ${on ? "selected" : ""}`}
                >
                  <span className="grid place-items-center">
                    <input type="checkbox" checked={on} onChange={() => toggle(id, item.path)} onClick={(e) => e.stopPropagation()} aria-label={name} />
                  </span>
                  <span className="flex min-w-0 items-center gap-2">
                    <span className="shrink-0">{looksLikeFile(item) ? <FileIcon size={16} /> : <FolderIcon color={color} size={18} />}</span>
                    <span className="truncate">{name}</span>
                    {parent && <span className={`truncate ${on ? "text-white/70" : "text-[#888]"}`}>{parent}</span>}
                    {item.recently_modified && (
                      <span className="flex shrink-0 items-center gap-1 rounded bg-[#f59e0b] px-1 text-[10px] text-white">
                        <AlertTriangle size={9} />
                        {t("win.inUse")}
                      </span>
                    )}
                  </span>
                  <span className="text-right tabular-nums">{fmt(item.size_bytes)}</span>
                  <span className="text-right tabular-nums">{item.file_count}</span>
                </div>
              );
            })}
          </div>
        )}

        {confirming && <ConfirmSheet id={id} count={sel.size} bytes={selBytes} onClose={() => setConfirming(false)} />}
      </div>

      {/* Durum çubuğu */}
      <div className="flex shrink-0 items-center justify-between gap-3 border-t border-[#b4b4b4] bg-gradient-to-b from-[#f2f2f2] to-[#dcdcdc] px-3 py-1.5">
        <span className="truncate text-xs text-[#333]">
          {busy
            ? progText
            : result && result.hidden_recent > 0
              ? t("win.hiddenRecent", { count: result.hidden_recent })
              : result && result.rejected > 0
                ? t("win.rejected", { count: result.rejected })
                : t("win.selected", { count: sel.size, size: fmt(selBytes) })}
        </span>
        <button className="aqua-btn primary small" disabled={sel.size === 0} onClick={() => setConfirming(true)}>
          {t("win.clean")}
        </button>
      </div>
    </Window>
  );
}

function Center({ children }: { children: React.ReactNode }) {
  return <div className="absolute inset-0 grid place-items-center">{children}</div>;
}

/** Pencerenin başlık çubuğundan aşağı kayan Aqua "sheet" onay iletişim kutusu. */
function ConfirmSheet({ id, count, bytes, onClose }: { id: CategoryId; count: number; bytes: number; onClose: () => void }) {
  const { t, i18n } = useTranslation();
  const { clean, cleaning } = useStore();
  const [permanent, setPermanent] = useState(false);
  const irreversible = isIrreversible(id);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !cleaning && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [cleaning, onClose]);

  const submit = async () => {
    if (await clean(id, permanent)) onClose();
  };

  return (
    <div className="absolute inset-0 z-30 flex items-start justify-center bg-black/10" onClick={() => !cleaning && onClose()}>
      <div
        role="alertdialog"
        aria-labelledby="sheet-title"
        onClick={(e) => e.stopPropagation()}
        className="sheet aqua-pinstripes w-[380px] rounded-b-lg border border-t-0 border-black/40 p-4 shadow-[0_10px_28px_rgba(0,0,0,.45)]"
      >
        <div className="flex gap-4">
          <AquaTile color={irreversible ? "#c0392b" : "#3b82f6"} glyph={irreversible ? TriangleAlert : ShieldCheck} size={52} />
          <div className="min-w-0 flex-1">
            <h2 id="sheet-title" className="text-[13px] font-bold">
              {t(irreversible ? "confirm.binTitle" : "confirm.title", { count })}
            </h2>
            <p className="mt-1 text-xs text-[#333]">
              {t("confirm.summary", { size: formatBytes(bytes, i18n.language) })} {t(irreversible ? "confirm.binWarn" : "confirm.safe")}
            </p>
            {isUserFiles(id) && <p className="mt-1.5 text-xs font-medium text-[#8a4b00]">{t("win.userFilesNote")}</p>}
            {PERMANENT_ALLOWED[id] && (
              <label className="mt-3 flex cursor-pointer items-start gap-2 text-xs">
                <input type="checkbox" checked={permanent} onChange={(e) => setPermanent(e.target.checked)} className="mt-0.5" />
                <span>
                  {t("confirm.permanent")}
                  {permanent && (
                    <span className="mt-0.5 flex items-center gap-1 font-medium text-[#c0392b]">
                      <TriangleAlert size={12} />
                      {t("confirm.permanentWarn")}
                    </span>
                  )}
                </span>
              </label>
            )}
          </div>
        </div>
        <div className="mt-4 flex justify-end gap-2">
          <button className="aqua-btn" onClick={onClose} disabled={cleaning} autoFocus={irreversible}>
            {t("confirm.cancel")}
          </button>
          {/* Geri alınamayan işlemde odak "Vazgeç" düğmesindedir; yanlışlıkla Enter ile silinmez. */}
          <button
            className={`aqua-btn ${irreversible ? "danger" : "primary pulse"} flex items-center gap-1.5`}
            onClick={() => void submit()}
            disabled={cleaning}
            autoFocus={!irreversible}
          >
            {cleaning && <Loader2 size={13} className="animate-spin" />}
            {cleaning ? t("confirm.working") : t(irreversible ? "confirm.binConfirm" : "confirm.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
