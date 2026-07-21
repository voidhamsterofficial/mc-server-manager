// Shared display formatting helpers.

// Two ladders on purpose, because the two things being measured count
// differently. Disk sizes are decimal (a 1 MB file is 1,000,000 bytes, the
// convention every file manager and drive vendor uses), while memory is
// binary — a server told to take 2048 MB should read back as 2.00 GB, not
// 2.15 GB.
const BYTES_PER_KB = 1000;
const BYTES_PER_MB = 1000 * BYTES_PER_KB;
const BYTES_PER_GB = 1000 * BYTES_PER_MB;

const MEMORY_BYTES_PER_MB = 1024 * 1024;
const MEMORY_BYTES_PER_GB = 1024 * MEMORY_BYTES_PER_MB;

/** A file or backup size, in decimal units: 512 B, 4 KB, 1.5 MB, 2.30 GB. */
export function formatFileSize(bytes: number): string {
  if (bytes >= BYTES_PER_GB) {
    return `${(bytes / BYTES_PER_GB).toFixed(2)} GB`;
  }
  if (bytes >= BYTES_PER_MB) {
    return `${(bytes / BYTES_PER_MB).toFixed(1)} MB`;
  }
  if (bytes >= BYTES_PER_KB) {
    return `${Math.round(bytes / BYTES_PER_KB)} KB`;
  }
  return `${Math.round(bytes)} B`;
}

/** A memory figure, in binary units, so it lines up with the MB a server was
 *  configured with (and with the `-Xmx` flag it's launched under). */
export function formatMemory(bytes: number): string {
  if (bytes >= MEMORY_BYTES_PER_GB) {
    return `${(bytes / MEMORY_BYTES_PER_GB).toFixed(2)} GB`;
  }
  return `${Math.round(bytes / MEMORY_BYTES_PER_MB)} MB`;
}

export function formatUptime(totalSeconds: number): string {
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = totalSeconds % 60;

  if (hours > 0) {
    return `${hours}h ${minutes}m`;
  }
  if (minutes > 0) {
    return `${minutes}m ${seconds}s`;
  }
  return `${seconds}s`;
}

export function formatDateTime(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString();
}
