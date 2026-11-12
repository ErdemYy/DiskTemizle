const UNITS = ["B", "KB", "MB", "GB", "TB"];

export function formatBytes(bytes: number, locale = "tr"): string {
  if (bytes <= 0) return "0 B";
  const i = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), UNITS.length - 1);
  const v = bytes / 1024 ** i;
  const digits = i === 0 || v >= 100 ? 0 : v >= 10 ? 1 : 2;
  return `${v.toLocaleString(locale, { maximumFractionDigits: digits, minimumFractionDigits: 0 })} ${UNITS[i]}`;
}
