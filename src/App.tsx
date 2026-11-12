import { useCallback, useEffect, useState } from "react";
import { AboutWindow } from "./components/AboutWindow";
import { CategoryWindow } from "./components/CategoryWindow";
import { Desktop } from "./components/Desktop";
import { Dock } from "./components/Dock";
import { HistoryWindow } from "./components/HistoryWindow";
import { MenuBar } from "./components/MenuBar";
import { Notice } from "./components/Notice";
import { SettingsWindow } from "./components/SettingsWindow";
import { Splash } from "./components/Splash";
import { onScanProgress } from "./lib/api";
import { CATEGORIES, type CategoryId } from "./lib/types";
import { useStore } from "./store";

export default function App() {
  const [booted, setBooted] = useState(false);
  const done = useCallback(() => setBooted(true), []);
  const windows = useStore((s) => s.windows);

  useEffect(() => {
    void useStore.getState().loadExcluded();
  }, []);

  useEffect(() => {
    // Rust tarafındaki canlı tarama ilerlemesi.
    let stop = () => {};
    let disposed = false;
    void onScanProgress((p) => useStore.getState().setProgress(p)).then((un) => {
      if (disposed) un();
      else stop = un;
    });
    return () => {
      disposed = true;
      stop();
    };
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!(e.ctrlKey || e.metaKey)) return;
      const s = useStore.getState();
      if (e.key.toLowerCase() === "r") {
        e.preventDefault();
        void s.scanAll();
      } else if (e.key === ",") {
        e.preventDefault();
        s.openWindow("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  return (
    <div className="fixed inset-0 overflow-hidden">
      <Desktop />
      {/* Pencereler kendi yığın bağlamında kalır; menü çubuğu ve Dock her zaman üstte. */}
      <div className="pointer-events-none absolute inset-0 isolate">
        <div className="pointer-events-auto contents">
          {Object.values(windows).map((w) => {
            if (!w) return null;
            if ((CATEGORIES as string[]).includes(w.id)) return <CategoryWindow key={w.id} id={w.id as CategoryId} win={w} />;
            if (w.id === "settings") return <SettingsWindow key={w.id} win={w} />;
            if (w.id === "history") return <HistoryWindow key={w.id} win={w} />;
            return <AboutWindow key={w.id} win={w} />;
          })}
        </div>
      </div>
      <MenuBar />
      <Dock />
      <Notice />
      {!booted && <Splash onDone={done} />}
    </div>
  );
}
