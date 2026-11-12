export type CategoryId = "temp" | "browser" | "dev_cache" | "dev_artifacts" | "large_files" | "duplicates";

export const CATEGORIES: CategoryId[] = ["temp", "browser", "dev_cache", "dev_artifacts", "large_files", "duplicates"];

/** "Tümünü Tara" yalnızca hızlı kategorileri tarar; kullanıcı dosyası bulucuları tüm profili gezdiği için kendi pencerelerinden başlar. */
export const QUICK_CATEGORIES: CategoryId[] = ["temp", "browser", "dev_cache", "dev_artifacts"];

/** Kullanıcının kendi dosyalarını listeleyen kategoriler: hiçbir şey önceden seçilmez, yalnızca Geri Dönüşüm'e taşınır. */
export const USER_FILE_CATEGORIES: CategoryId[] = ["large_files", "duplicates"];
export const isUserFiles = (c: CategoryId) => USER_FILE_CATEGORIES.includes(c);

/** Kalıcı silme yalnızca yeniden üretilebilir önbelleklerde sunulur (Rust tarafı da aynısını uygular). */
export const PERMANENT_ALLOWED: Record<CategoryId, boolean> = {
  temp: true,
  browser: true,
  dev_cache: true,
  dev_artifacts: false,
  large_files: false,
  duplicates: false,
};

export interface Item {
  path: string;
  size_bytes: number;
  file_count: number;
  recently_modified: boolean;
  age_secs: number;
  /** Büyük dosya bulucu: temizleme önerisi puanı (0-100). */
  score: number;
  /** Öneri gerekçesi kodu: installer, archive, duplicate, ... */
  reason: string | null;
  recommended: boolean;
  /** Yinelenen dosya grubu. */
  group: number | null;
  /** Yinelenen grupta saklanması önerilen kopya. */
  keep: boolean;
}

export interface ScanResult {
  category: CategoryId;
  items: Item[];
  total_bytes: number;
  rejected: number;
  /** Yaş eşiğinden yeni olduğu için listelenmeyen öğe sayısı (yalnızca geçici dosyalar). */
  hidden_recent: number;
}

export type Action = "trashed" | "deleted" | "skipped" | "failed" | "restored";

export interface Outcome {
  path: string;
  action: Action;
  bytes: number;
  detail: string | null;
}

export interface Report {
  outcomes: Outcome[];
  freed_bytes: number;
}
