import { CheckCircle2, Info, X, XCircle } from "lucide-react";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useStore } from "../store";

const ICON = { success: CheckCircle2, info: Info, error: XCircle } as const;
const TONE = { success: "text-[#2fa62a]", info: "text-[#3b82f6]", error: "text-[#c0392b]" } as const;

/** Sağ üstte beliren Aqua bildirim kartı. */
export function Notice() {
  const { t } = useTranslation();
  const { notice, undo, dismissNotice } = useStore();

  // Geri alınabilir bildirim daha uzun kalır; hata bildirimi kullanıcı kapatana dek durur.
  useEffect(() => {
    if (!notice || notice.kind === "error") return;
    const id = setTimeout(dismissNotice, notice.undoPaths ? 15000 : 5000);
    return () => clearTimeout(id);
  }, [notice, dismissNotice]);

  if (!notice) return null;
  const Icon = ICON[notice.kind];

  return (
    <div
      role="status"
      aria-live="polite"
      className="aqua-window aqua-pinstripes window-opening fixed top-9 right-4 z-[95] flex w-72 items-start gap-3 p-3"
    >
      <Icon size={22} className={`mt-0.5 shrink-0 ${TONE[notice.kind]}`} />
      <div className="min-w-0 flex-1">
        <div className="text-[13px] font-bold">{notice.message}</div>
        {notice.detail && <div className="mt-0.5 text-xs break-words text-[#555]">{notice.detail}</div>}
        {notice.undoPaths && (
          <button className="aqua-btn small mt-2" onClick={() => void undo()}>
            {t("notice.undo")}
          </button>
        )}
      </div>
      <button onClick={dismissNotice} aria-label={t("notice.close")} className="text-[#555] hover:text-black">
        <X size={14} />
      </button>
    </div>
  );
}
