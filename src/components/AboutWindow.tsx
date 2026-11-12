import { ShieldCheck } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { Win } from "../store";
import { AquaTile } from "./icons";
import { Window } from "./Window";

export function AboutWindow({ win }: { win: Win }) {
  const { t } = useTranslation();
  return (
    <Window win={win} title={t("menu.about")} resizable={false}>
      <div className="aqua-pinstripes flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-6 text-center">
        <AquaTile color="#3b82f6" glyph={ShieldCheck} size={72} />
        <div className="text-base font-bold">{t("app.name")}</div>
        <div className="text-xs text-[#555]">
          {t("app.version")} 0.1.0
        </div>
        <p className="mt-1 text-xs text-[#333]">{t("about.body")}</p>
      </div>
    </Window>
  );
}
