import { ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Lang } from "../i18n";
import { useStore, type Wallpaper as WP, type Win } from "../store";
import { Wallpaper } from "./Wallpaper";
import { Window } from "./Window";

export function SettingsWindow({ win }: { win: Win }) {
  const { t } = useTranslation();
  const { lang, setLang, wallpaper, setWallpaper } = useStore();

  return (
    <Window win={win} title={t("settings.title")} resizable={false}>
      <div className="aqua-pinstripes min-h-0 flex-1 space-y-4 overflow-auto p-5 text-[13px]">
        <Row label={t("settings.language")}>
          <div className="aqua-seg" role="radiogroup" aria-label={t("settings.language")}>
            {(["tr", "en"] as Lang[]).map((l) => (
              <button key={l} role="radio" aria-checked={lang === l} onClick={() => setLang(l)}>
                {l === "tr" ? "Türkçe" : "English"}
              </button>
            ))}
          </div>
        </Row>

        <Row label={t("settings.wallpaper")}>
          <div className="flex gap-3" role="radiogroup" aria-label={t("settings.wallpaper")}>
            {(["aqua", "graphite"] as WP[]).map((w) => (
              <button
                key={w}
                role="radio"
                aria-checked={wallpaper === w}
                onClick={() => setWallpaper(w)}
                className="flex flex-col items-center gap-1"
              >
                <span
                  className={`block h-14 w-24 overflow-hidden rounded border ${wallpaper === w ? "border-[#3b82f6] ring-2 ring-[#3b82f6]" : "border-black/40"}`}
                >
                  <Wallpaper kind={w} className="h-full w-full" />
                </span>
                <span className="text-xs">{t(`settings.${w}`)}</span>
              </button>
            ))}
          </div>
        </Row>

        <div className="flex items-start gap-3 rounded-md border border-black/25 bg-white/70 p-3">
          <ShieldCheck size={18} className="mt-0.5 shrink-0 text-[#2fa62a]" />
          <div>
            <div className="font-bold">{t("settings.safetyTitle")}</div>
            <p className="mt-0.5 text-xs text-[#333]">{t("settings.safetyBody")}</p>
          </div>
        </div>
      </div>
    </Window>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-4">
      <span className="font-medium">{label}</span>
      {children}
    </div>
  );
}
