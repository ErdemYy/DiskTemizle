import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { CategoryId, Item, Report, ScanResult } from "./types";

// VITE_MOCK=1 gerçek Tauri penceresinde bile sahte veri kullanır (arayüz geliştirirken gerçek diske dokunmadan).
const inTauri =
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window && import.meta.env.VITE_MOCK !== "1";

export interface DiskInfo {
  label: string;
  total_bytes: number;
  free_bytes: number;
}

export interface ScanProgress {
  category: CategoryId;
  /** discover: `done` şimdiye dek bulunan sayı, `total` bilinmez (0). measure: done / total. */
  phase: "discover" | "measure";
  done: number;
  total: number;
}

/** `null` = tarama iptal edildi. */
export const scanCategory = (id: CategoryId): Promise<ScanResult | null> =>
  inTauri ? invoke("scan_category", { id }) : mockScan(id);

export const cancelScan = (id: CategoryId): Promise<void> => (inTauri ? invoke("cancel_scan", { id }) : Promise.resolve());

export const onScanProgress = (cb: (p: ScanProgress) => void): Promise<() => void> =>
  inTauri ? listen<ScanProgress>("scan-progress", (e) => cb(e.payload)) : Promise.resolve(() => {});

export const cleanItems = (id: CategoryId, paths: string[], permanent: boolean): Promise<Report> =>
  inTauri ? invoke("clean_items", { id, paths, permanent }) : mockClean(paths, permanent || id === "recycle_bin");

export const undoClean = (paths: string[]): Promise<Report> =>
  inTauri ? invoke("undo_clean", { paths }) : mockUndo(paths);

export interface JournalEntry {
  ts: number;
  /** Kategori kimliği ya da geri alma kayıtları için "undo". */
  category: string;
  path: string;
  action: "trashed" | "deleted" | "skipped" | "failed" | "restored";
  bytes: number;
  detail: string | null;
  /** Geri Dönüşüm'e taşınmış ve sonradan geri yüklenmemiş son kayıt. */
  undoable: boolean;
}

export const journalEntries = (limit = 500): Promise<JournalEntry[]> =>
  inTauri ? invoke("journal_entries", { limit }) : mockJournal();

export const diskInfo = (): Promise<DiskInfo> =>
  inTauri ? invoke("disk_info") : Promise.resolve({ label: "C:", total_bytes: 476 * 1024 ** 3, free_bytes: 118 * 1024 ** 3 });

// --- Tarayıcıda (Tauri dışında) arayüzü geliştirmek için sahte veri ---

const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));
const MB = 1024 * 1024;

const MOCK: Record<Exclude<CategoryId, "large_files" | "duplicates" | "recycle_bin">, [string, number, number, boolean][]> = {
  temp: [
    ["C:\\Users\\demo\\AppData\\Local\\Temp\\installer_cache", 842 * MB, 312, false],
    ["C:\\Users\\demo\\AppData\\Local\\Temp\\crash_dumps", 410 * MB, 18, false],
    ["C:\\Users\\demo\\AppData\\Local\\Temp\\setup_log.txt", 2 * MB, 1, false],
    ["C:\\Users\\demo\\AppData\\Local\\Temp\\active_session.tmp", 12 * MB, 3, true],
  ],
  browser: [
    ["C:\\Users\\demo\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache", 1330 * MB, 5210, false],
    ["C:\\Users\\demo\\AppData\\Local\\Microsoft\\Edge\\User Data\\Default\\Code Cache", 240 * MB, 880, false],
  ],
  dev_cache: [
    ["C:\\Users\\demo\\AppData\\Local\\npm-cache", 1860 * MB, 14200, false],
    ["C:\\Users\\demo\\AppData\\Local\\pip\\Cache", 310 * MB, 960, false],
  ],
  dev_artifacts: [
    ["C:\\Users\\demo\\Projects\\shop\\node_modules", 920 * MB, 41000, false],
    ["C:\\Users\\demo\\Projects\\api\\target", 2480 * MB, 6200, false],
    ["C:\\Users\\demo\\Projects\\blog\\.next", 188 * MB, 900, true],
  ],
};

const removed = new Set<string>();

const DAY = 24 * 3600;
const fileItem = (path: string, size_bytes: number, age_days: number, extra: Partial<Item> = {}): Item => ({
  path,
  size_bytes,
  file_count: 1,
  recently_modified: false,
  age_secs: age_days * DAY,
  score: 0,
  reason: null,
  recommended: false,
  group: null,
  keep: false,
  ...extra,
});

/** Kullanıcı dosyası kategorileri için sahte veri (öneri puanları ve kopya grupları dahil). */
const MOCK_FILES: Record<"large_files" | "duplicates" | "recycle_bin", Item[]> = {
  recycle_bin: [
    fileItem("C:\\Users\\demo\\Documents\\eski_proje", 391 * MB, 28, { file_count: 812, reason: "in_bin" }),
    fileItem("C:\\Users\\demo\\Videos\\CapCut", 78 * MB, 8, { file_count: 40, reason: "in_bin" }),
    fileItem("C:\\Users\\demo\\Desktop\\rapor_taslak.docx", 3 * MB, 61, { reason: "in_bin", recommended: true }),
    fileItem("C:\\Users\\demo\\Desktop\\eski_sunum.pptx", 22 * MB, 94, { reason: "in_bin", recommended: true }),
    fileItem("C:\\Users\\demo\\Downloads\\setup_old.exe", 140 * MB, 180, { reason: "in_bin", recommended: true }),
  ],
  large_files: [
    fileItem("C:\\Users\\demo\\Downloads\\ubuntu-24.04.iso", 5.8 * 1024 * MB, 220, { score: 88, reason: "disk_image", recommended: true }),
    fileItem("C:\\Users\\demo\\Downloads\\Setup_Studio_2024.exe", 1.2 * 1024 * MB, 400, { score: 85, reason: "installer", recommended: true }),
    fileItem("C:\\Users\\demo\\Documents\\Logs\\app.dmp", 640 * MB, 95, { score: 78, reason: "dump_or_log", recommended: true }),
    fileItem("C:\\Users\\demo\\Videos\\tatil_2023.mp4", 3.1 * 1024 * MB, 500, { score: 40, reason: "media" }),
    fileItem("C:\\Users\\demo\\Documents\\tez_taslak.pdf", 180 * MB, 30, { score: 10, reason: "personal_doc" }),
  ],
  duplicates: [
    fileItem("C:\\Users\\demo\\Documents\\Fotoğraflar\\IMG_2041.jpg", 8 * MB, 300, { group: 1, keep: true, reason: "duplicate" }),
    fileItem("C:\\Users\\demo\\Downloads\\IMG_2041 (1).jpg", 8 * MB, 120, { group: 1, recommended: true, reason: "duplicate" }),
    fileItem("C:\\Users\\demo\\Desktop\\IMG_2041 - Kopya.jpg", 8 * MB, 60, { group: 1, recommended: true, reason: "duplicate" }),
    fileItem("C:\\Users\\demo\\Documents\\rapor_final.pdf", 14 * MB, 200, { group: 2, keep: true, reason: "duplicate" }),
    fileItem("C:\\Users\\demo\\Downloads\\rapor_final.pdf", 14 * MB, 20, { group: 2, recommended: true, reason: "duplicate" }),
  ],
};

async function mockScan(id: CategoryId): Promise<ScanResult> {
  await wait(600);
  if (id === "large_files" || id === "duplicates" || id === "recycle_bin") {
    const items = MOCK_FILES[id].filter((i) => !removed.has(i.path));
    // "Kazanılabilir": kutuda hepsi; bulucularda yalnızca önerilenler.
    const total_bytes = items
      .filter((i) => id === "recycle_bin" || i.recommended)
      .reduce((s, i) => s + i.size_bytes, 0);
    return { category: id, items, total_bytes, rejected: 0, hidden_recent: 0 };
  }
  const items = MOCK[id]
    .filter(([p]) => !removed.has(p))
    .map(([path, size_bytes, file_count, recently_modified]) =>
      fileItem(path, size_bytes, recently_modified ? 0 : 30, { file_count, recently_modified, age_secs: recently_modified ? 600 : 30 * DAY }),
    );
  return {
    category: id,
    items,
    total_bytes: items.reduce((s, i) => s + i.size_bytes, 0),
    rejected: 0,
    hidden_recent: id === "temp" ? 3 : 0,
  };
}

async function mockClean(paths: string[], permanent: boolean): Promise<Report> {
  await wait(700);
  const sizes = new Map<string, number>([
    ...Object.values(MOCK).flat().map(([p, size]): [string, number] => [p, size]),
    ...Object.values(MOCK_FILES).flat().map((i): [string, number] => [i.path, i.size_bytes]),
  ]);
  const outcomes = paths.map((path) => {
    const bytes = sizes.get(path) ?? 0;
    removed.add(path);
    mockLog.push({ ts: nowSec(), category: "temp", path, action: permanent ? "deleted" : "trashed", bytes, detail: null, undoable: !permanent });
    return { path, action: permanent ? ("deleted" as const) : ("trashed" as const), bytes, detail: null };
  });
  return { outcomes, freed_bytes: outcomes.reduce((s, o) => s + o.bytes, 0) };
}

/** Tarayıcıda sahte günlük: bu oturumdaki sahte temizlikler + birkaç eski kayıt. */
const mockLog: JournalEntry[] = [];
const nowSec = () => Math.floor(Date.now() / 1000);

async function mockJournal(): Promise<JournalEntry[]> {
  await wait(150);
  const seed: JournalEntry[] = [
    { ts: nowSec() - 3 * 86400, category: "temp", path: "C:\\Users\\demo\\AppData\\Local\\Temp\\old_setup", action: "trashed", bytes: 220 * MB, detail: null, undoable: true },
    { ts: nowSec() - 3 * 86400, category: "dev_artifacts", path: "C:\\Users\\demo\\Projects\\old\\node_modules", action: "trashed", bytes: 640 * MB, detail: null, undoable: true },
    { ts: nowSec() - 9 * 86400, category: "browser", path: "C:\\Users\\demo\\AppData\\Local\\Google\\Chrome\\User Data\\Default\\Cache", action: "deleted", bytes: 410 * MB, detail: null, undoable: false },
    { ts: nowSec() - 9 * 86400, category: "temp", path: "C:\\Windows\\Temp\\locked.tmp", action: "failed", bytes: 0, detail: "in use", undoable: false },
  ];
  const restored = new Set(mockLog.filter((e) => e.action === "restored").map((e) => e.path));
  const old = seed.map((e) => ({ ...e, undoable: e.undoable && !restored.has(e.path) }));
  return [...mockLog, ...old].sort((a, b) => b.ts - a.ts);
}

async function mockUndo(paths: string[]): Promise<Report> {
  await wait(400);
  paths.forEach((p) => {
    removed.delete(p);
    mockLog.forEach((e) => e.path === p && (e.undoable = false));
    mockLog.push({ ts: nowSec(), category: "undo", path: p, action: "restored", bytes: 0, detail: null, undoable: false });
  });
  return { outcomes: paths.map((path) => ({ path, action: "restored" as const, bytes: 0, detail: null })), freed_bytes: 0 };
}
