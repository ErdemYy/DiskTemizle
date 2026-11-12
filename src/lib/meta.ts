import { Code2, Copy, FileArchive, Globe, HardDrive, Package, Trash2, type LucideIcon } from "lucide-react";
import type { CategoryId } from "./types";

export const CATEGORY_META: Record<CategoryId, { icon: LucideIcon; color: string }> = {
  temp: { icon: HardDrive, color: "var(--c-temp)" },
  recycle_bin: { icon: Trash2, color: "var(--c-bin)" },
  browser: { icon: Globe, color: "var(--c-browser)" },
  dev_cache: { icon: Package, color: "var(--c-dev-cache)" },
  dev_artifacts: { icon: Code2, color: "var(--c-dev-artifacts)" },
  large_files: { icon: FileArchive, color: "var(--c-large)" },
  duplicates: { icon: Copy, color: "var(--c-dup)" },
};
