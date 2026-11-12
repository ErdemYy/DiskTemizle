import { create } from "zustand";
import i18n, { type Lang } from "./i18n";
import {
  cancelScan,
  cleanItems,
  diskInfo,
  journalEntries,
  scanCategory,
  undoClean,
  type DiskInfo,
  type JournalEntry,
  type ScanProgress,
} from "./lib/api";
import { formatBytes as fmt } from "./lib/format";
import { CATEGORIES, QUICK_CATEGORIES, isUserFiles, type CategoryId, type Report, type ScanResult } from "./lib/types";

export type WinId = CategoryId | "settings" | "about" | "history";
export type Phase = "open" | "closing" | "minimizing" | "minimized";
export type Wallpaper = "aqua" | "graphite";
export type ViewMode = "icon" | "list";

export const MENUBAR_H = 22;
export const DOCK_SPACE = 84;

export interface Win {
  id: WinId;
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
  phase: Phase;
  zoomed: boolean;
  view: ViewMode;
  minDx: number;
  minDy: number;
}

const SIZES: Record<WinId, [number, number]> = {
  temp: [660, 460],
  browser: [660, 460],
  dev_cache: [660, 460],
  dev_artifacts: [660, 460],
  large_files: [740, 480],
  duplicates: [740, 480],
  history: [780, 460],
  settings: [460, 330],
  about: [340, 270],
};

export interface Notice {
  kind: "success" | "error" | "info";
  message: string;
  detail?: string;
  /** Geri alınabilir (Geri Dönüşüm'e taşınan) yollar. */
  undoPaths?: string[];
  undoCategory?: CategoryId;
}

interface State {
  windows: Partial<Record<WinId, Win>>;
  topZ: number;
  wallpaper: Wallpaper;
  lang: Lang;
  scans: Partial<Record<CategoryId, ScanResult>>;
  scanning: Partial<Record<CategoryId, boolean>>;
  progress: Partial<Record<CategoryId, ScanProgress>>;
  selected: Record<CategoryId, Set<string>>;
  cleaning: boolean;
  notice: Notice | null;
  disk: DiskInfo | null;
  history: JournalEntry[];

  setProgress: (p: ScanProgress) => void;
  loadDisk: () => Promise<void>;
  loadHistory: () => Promise<void>;
  undoEntry: (path: string, category: string) => Promise<void>;
  openWindow: (id: WinId) => void;
  closeWindow: (id: WinId) => void;
  minimizeWindow: (id: WinId, dx: number, dy: number) => void;
  finishPhase: (id: WinId) => void;
  focusWindow: (id: WinId) => void;
  moveWindow: (id: WinId, x: number, y: number) => void;
  resizeWindow: (id: WinId, w: number, h: number) => void;
  toggleZoom: (id: WinId) => void;
  setWinView: (id: WinId, v: ViewMode) => void;

  setWallpaper: (w: Wallpaper) => void;
  setLang: (l: Lang) => void;
  scan: (c: CategoryId) => Promise<void>;
  scanAll: () => Promise<void>;
  toggle: (c: CategoryId, path: string) => void;
  toggleAll: (c: CategoryId, on: boolean) => void;
  selectRecommended: (c: CategoryId, on: boolean) => void;
  clean: (c: CategoryId, permanent: boolean) => Promise<boolean>;
  undo: () => Promise<void>;
  dismissNotice: () => void;
}

const store = (k: string): string | null => {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
};
const save = (k: string, v: string) => {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* depolama yok */
  }
};

const emptySelection = (): Record<CategoryId, Set<string>> => ({
  temp: new Set(),
  browser: new Set(),
  dev_cache: new Set(),
  dev_artifacts: new Set(),
  large_files: new Set(),
  duplicates: new Set(),
});

const errText = (e: unknown) => (e instanceof Error ? e.message : String(e));
const isCategory = (id: WinId): id is CategoryId => (CATEGORIES as string[]).includes(id);

/** Görünür pencereler arasında en üsttekini döndürür. */
export function activeWindowId(windows: State["windows"]): WinId | null {
  let best: Win | null = null;
  for (const w of Object.values(windows)) {
    if (w && w.phase === "open" && (!best || w.z > best.z)) best = w;
  }
  return best?.id ?? null;
}

export const useStore = create<State>((set, get) => ({
  windows: {},
  topZ: 10,
  wallpaper: store("wallpaper") === "graphite" ? "graphite" : "aqua",
  lang: i18n.language === "tr" ? "tr" : "en",
  scans: {},
  scanning: {},
  progress: {},
  selected: emptySelection(),
  cleaning: false,
  notice: null,
  disk: null,
  history: [],

  loadHistory: async () => {
    try {
      set({ history: await journalEntries(500) });
    } catch (e) {
      set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: errText(e) } });
    }
  },

  undoEntry: async (path, category) => {
    try {
      const report = await undoClean([path]);
      const o = report.outcomes[0];
      if (o?.action === "restored") {
        set({ notice: { kind: "info", message: i18n.t("notice.restored") } });
        if (isCategory(category as WinId) && get().scans[category as CategoryId]) void get().scan(category as CategoryId);
      } else {
        set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: o?.detail ?? undefined } });
      }
    } catch (e) {
      set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: errText(e) } });
    }
    void get().loadDisk();
    await get().loadHistory();
  },

  setProgress: (p) => set((s) => ({ progress: { ...s.progress, [p.category]: p } })),

  loadDisk: async () => {
    try {
      set({ disk: await diskInfo() });
    } catch {
      /* disk bilgisi isteğe bağlı: ikon etiketsiz görünür */
    }
  },

  openWindow: (id) => {
    const { windows, topZ } = get();
    const z = topZ + 1;
    const existing = windows[id];
    if (existing) {
      set({ topZ: z, windows: { ...windows, [id]: { ...existing, z, phase: "open" } } });
      return;
    }
    const n = Object.keys(windows).length;
    const [w, defaultH] = SIZES[id];
    const vw = typeof window === "undefined" ? 1100 : window.innerWidth;
    const vh = typeof window === "undefined" ? 740 : window.innerHeight;
    // Pencere, menü çubuğu ile Dock arasındaki görünür alana sığmalı.
    const h = Math.max(240, Math.min(defaultH, vh - MENUBAR_H - 28 - DOCK_SPACE - n * 30));
    const win: Win = {
      id,
      w,
      h,
      x: Math.max(24, Math.min(vw - w - 140, 70 + n * 30)),
      y: MENUBAR_H + 28 + n * 30,
      z,
      phase: "open",
      zoomed: false,
      view: isCategory(id) && isUserFiles(id) ? "list" : "icon",
      minDx: 0,
      minDy: 400,
    };
    set({ topZ: z, windows: { ...windows, [id]: win } });
    if (isCategory(id) && !get().scans[id]) void get().scan(id);
  },

  closeWindow: (id) =>
    set((s) => {
      const w = s.windows[id];
      if (!w) return {};
      // Pencere kapanınca süren tarama boşuna disk/CPU harcamasın.
      if (isCategory(id) && s.scanning[id]) void cancelScan(id);
      if (w.phase === "minimized") {
        const { [id]: _gone, ...rest } = s.windows;
        return { windows: rest };
      }
      return { windows: { ...s.windows, [id]: { ...w, phase: "closing" } } };
    }),

  minimizeWindow: (id, minDx, minDy) =>
    set((s) => {
      const w = s.windows[id];
      return w ? { windows: { ...s.windows, [id]: { ...w, phase: "minimizing", minDx, minDy } } } : {};
    }),

  finishPhase: (id) =>
    set((s) => {
      const w = s.windows[id];
      if (!w) return {};
      if (w.phase === "closing") {
        const { [id]: _gone, ...rest } = s.windows;
        return { windows: rest };
      }
      if (w.phase === "minimizing") return { windows: { ...s.windows, [id]: { ...w, phase: "minimized" } } };
      return {};
    }),

  focusWindow: (id) =>
    set((s) => {
      const w = s.windows[id];
      if (!w || w.phase !== "open") return {};
      const z = s.topZ + 1;
      return { topZ: z, windows: { ...s.windows, [id]: { ...w, z } } };
    }),

  moveWindow: (id, x, y) =>
    set((s) => {
      const w = s.windows[id];
      if (!w) return {};
      const maxX = window.innerWidth - 80;
      const maxY = window.innerHeight - 40;
      return {
        windows: {
          ...s.windows,
          [id]: { ...w, x: Math.max(80 - w.w, Math.min(maxX, x)), y: Math.max(MENUBAR_H, Math.min(maxY, y)) },
        },
      };
    }),

  resizeWindow: (id, w, h) =>
    set((s) => {
      const win = s.windows[id];
      return win ? { windows: { ...s.windows, [id]: { ...win, w: Math.max(380, w), h: Math.max(240, h) } } } : {};
    }),

  toggleZoom: (id) =>
    set((s) => {
      const w = s.windows[id];
      return w ? { windows: { ...s.windows, [id]: { ...w, zoomed: !w.zoomed } } } : {};
    }),

  setWinView: (id, view) =>
    set((s) => {
      const w = s.windows[id];
      return w ? { windows: { ...s.windows, [id]: { ...w, view } } } : {};
    }),

  setWallpaper: (wallpaper) => {
    save("wallpaper", wallpaper);
    set({ wallpaper });
  },

  setLang: (lang) => {
    save("lang", lang);
    void i18n.changeLanguage(lang);
    set({ lang });
  },

  scan: async (c) => {
    if (get().scanning[c]) return;
    set((s) => ({ scanning: { ...s.scanning, [c]: true } }));
    try {
      const result = await scanCategory(c);
      if (!result) return; // iptal edildi; önceki sonuç korunur
      set((s) => ({
        scans: { ...s.scans, [c]: result },
        // Kullanımda olabilecek öğeler varsayılan olarak seçili gelmez. Kullanıcı dosyalarında
        // (büyük/yinelenen) hiçbir şey önceden seçilmez; öneriler açıkça "Önerilenleri Seç" ile işaretlenir.
        selected: {
          ...s.selected,
          [c]: new Set(isUserFiles(c) ? [] : result.items.filter((i) => !i.recently_modified).map((i) => i.path)),
        },
      }));
    } catch (e) {
      set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: errText(e) } });
    } finally {
      set((s) => {
        const { [c]: _done, ...progress } = s.progress;
        return { scanning: { ...s.scanning, [c]: false }, progress };
      });
    }
  },

  scanAll: async () => {
    await Promise.all(QUICK_CATEGORIES.map((c) => get().scan(c)));
  },

  toggle: (c, path) =>
    set((s) => {
      const next = new Set(s.selected[c]);
      if (!next.delete(path)) {
        // Yinelenenlerde bir grubun tüm kopyaları seçilemez: en az biri kalmalı (Rust da denetler).
        if (c === "duplicates") {
          const items = s.scans[c]?.items ?? [];
          const group = items.find((i) => i.path === path)?.group;
          const others = items.filter((i) => i.group === group && i.path !== path);
          if (group != null && others.length > 0 && others.every((i) => next.has(i.path))) return {};
        }
        next.add(path);
      }
      return { selected: { ...s.selected, [c]: next } };
    }),

  toggleAll: (c, on) =>
    set((s) => ({
      selected: {
        ...s.selected,
        // Yinelenenlerde "tümü" saklanacak kopyaları içermez: her gruptan biri kalmalı.
        [c]: new Set(
          on ? (s.scans[c]?.items ?? []).filter((i) => c !== "duplicates" || !i.keep).map((i) => i.path) : [],
        ),
      },
    })),

  selectRecommended: (c, on) =>
    set((s) => {
      const rec = (s.scans[c]?.items ?? []).filter((i) => i.recommended).map((i) => i.path);
      const next = new Set(s.selected[c]);
      for (const p of rec) {
        if (on) next.add(p);
        else next.delete(p);
      }
      return { selected: { ...s.selected, [c]: next } };
    }),

  clean: async (c, permanent) => {
    const paths = [...get().selected[c]];
    if (paths.length === 0) return false;
    set({ cleaning: true });
    try {
      const report: Report = await cleanItems(c, paths, permanent);
      applyReport(c, report);
      void get().loadDisk();
      void get().loadHistory();
      const trashed = report.outcomes.filter((o) => o.action === "trashed").map((o) => o.path);
      const skipped = report.outcomes.filter((o) => o.action === "skipped" || o.action === "failed").length;
      set({
        notice: {
          kind: "success",
          message: i18n.t("notice.freed", { size: fmt(report.freed_bytes, i18n.language) }),
          detail: skipped > 0 ? i18n.t("notice.skipped", { count: skipped }) : undefined,
          undoPaths: trashed.length > 0 ? trashed : undefined,
          undoCategory: c,
        },
      });
      return true;
    } catch (e) {
      set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: errText(e) } });
      return false;
    } finally {
      set({ cleaning: false });
    }
  },

  undo: async () => {
    const n = get().notice;
    if (!n?.undoPaths || !n.undoCategory) return;
    const cat = n.undoCategory;
    try {
      await undoClean(n.undoPaths);
      set({ notice: { kind: "info", message: i18n.t("notice.restored") } });
      await get().scan(cat);
      void get().loadDisk();
      void get().loadHistory();
    } catch (e) {
      set({ notice: { kind: "error", message: i18n.t("notice.error"), detail: errText(e) } });
    }
  },

  dismissNotice: () => set({ notice: null }),
}));

/** Temizlenen öğeleri taramadan düşer ve toplamı günceller. */
function applyReport(c: CategoryId, report: Report) {
  const gone = new Set(
    report.outcomes.filter((o) => o.action === "trashed" || o.action === "deleted").map((o) => o.path.toLowerCase()),
  );
  useStore.setState((s) => {
    const scan = s.scans[c];
    if (!scan) return {};
    const items = scan.items.filter((i) => !gone.has(i.path.toLowerCase()));
    const keep = new Set(items.map((i) => i.path));
    return {
      scans: { ...s.scans, [c]: { ...scan, items, total_bytes: items.reduce((t, i) => t + i.size_bytes, 0) } },
      selected: { ...s.selected, [c]: new Set([...s.selected[c]].filter((p) => keep.has(p))) },
    };
  });
}
