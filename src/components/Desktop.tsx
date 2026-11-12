import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { formatBytes } from "../lib/format";
import { CATEGORY_META } from "../lib/meta";
import { CATEGORIES } from "../lib/types";
import { useStore } from "../store";
import { DriveIcon, FolderIcon } from "./icons";
import { Wallpaper } from "./Wallpaper";

const LABEL_SHADOW = { textShadow: "0 1px 3px rgba(0,0,0,.9), 0 0 2px rgba(0,0,0,.6)" };

/** Duvar kağıdı ve sağ kenardaki Finder tarzı ikonlar: önce disk, sonra kategori klasörleri. */
export function Desktop() {
  const { t, i18n } = useTranslation();
  const { wallpaper, scans, scanning, openWindow, disk, loadDisk } = useStore();
  const [selected, setSelected] = useState<string | null>(null);

  useEffect(() => {
    void loadDisk();
  }, [loadDisk]);

  const pick = (key: string) => (e: React.PointerEvent) => {
    e.stopPropagation();
    setSelected(key);
  };

  return (
    <div className="absolute inset-0" onPointerDown={() => setSelected(null)}>
      <Wallpaper kind={wallpaper} className="absolute inset-0 h-full w-full" />
      {/* İki sütun, sütun sırasıyla dolar (sitedeki gibi): disk + ilk üç kategori solda, kalanlar sağda. */}
      <div className="absolute top-8 right-4 grid grid-flow-col grid-rows-4 gap-x-1 gap-y-3">
        <button
          onPointerDown={pick("disk")}
          className="flex w-[92px] flex-col items-center gap-1 rounded-md p-1.5 text-center"
          aria-label={t("desk.disk", { label: disk?.label ?? "C:" })}
        >
          <span className={`rounded-md p-1 ${selected === "disk" ? "bg-white/25" : ""}`}>
            <DriveIcon size={56} />
          </span>
          <span
            className={`rounded px-1.5 text-[12px] leading-tight font-bold text-white ${selected === "disk" ? "bg-[#2a5bd7]" : ""}`}
            style={selected === "disk" ? undefined : LABEL_SHADOW}
          >
            {t("desk.disk", { label: disk?.label ?? "C:" })}
          </span>
          <span className="text-[10px] text-white/85" style={{ textShadow: "0 1px 2px rgba(0,0,0,.8)" }}>
            {disk ? t("desk.free", { size: formatBytes(disk.free_bytes, i18n.language) }) : ""}
          </span>
        </button>

        {CATEGORIES.map((c) => {
          const { icon, color } = CATEGORY_META[c];
          const scan = scans[c];
          const on = selected === c;
          return (
            <button
              key={c}
              onPointerDown={pick(c)}
              onDoubleClick={() => openWindow(c)}
              onKeyDown={(e) => e.key === "Enter" && openWindow(c)}
              className="flex w-[92px] flex-col items-center gap-1 rounded-md p-1.5 text-center"
              aria-label={t(`cat.${c}.name`)}
            >
              <span className={`rounded-md p-1 ${on ? "bg-white/25" : ""}`}>
                <FolderIcon color={color} glyph={icon} size={52} />
              </span>
              <span
                className={`rounded px-1.5 text-[12px] leading-tight font-bold text-white ${on ? "bg-[#2a5bd7]" : ""}`}
                style={on ? undefined : LABEL_SHADOW}
              >
                {t(`cat.${c}.name`)}
              </span>
              <span className="text-[10px] text-white/85" style={{ textShadow: "0 1px 2px rgba(0,0,0,.8)" }}>
                {scanning[c] ? t("win.scanning") : scan ? formatBytes(scan.total_bytes, i18n.language) : ""}
              </span>
            </button>
          );
        })}
      </div>
    </div>
  );
}
