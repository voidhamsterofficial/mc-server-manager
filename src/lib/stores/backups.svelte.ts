// Which servers are mid-backup, and how far along they are.
//
// This lives in a store rather than in the Backups tab because the tab is
// unmounted whenever you navigate away, while the backup keeps running. Held
// in the component, the progress bar vanished on leaving and the tab came
// back believing nothing was happening — offering to restore an archive that
// was still being written.

import type { BackupProgressEvent } from "../ipc/events";

interface BackupProgress {
  processedFiles: number;
  totalFiles: number;
}

class BackupsStore {
  private progressByServer = $state<Record<string, BackupProgress>>({});

  isBackingUp(serverId: string): boolean {
    const isRunning = this.progressByServer[serverId] !== undefined;
    return isRunning;
  }

  /** How far along, 0..1 — or null before the file count is known, which the
   *  progress bar shows as indeterminate. */
  fractionOf(serverId: string): number | null {
    const progress = this.progressByServer[serverId];
    if (progress === undefined || progress.totalFiles === 0) {
      return null;
    }
    const fraction = progress.processedFiles / progress.totalFiles;
    return fraction;
  }

  /**
   * Marks a backup as started. Called when the user asks for one, so the bar
   * appears immediately rather than waiting for the first progress event —
   * which only arrives once the server folder has been walked.
   */
  start(serverId: string): void {
    this.progressByServer[serverId] = { processedFiles: 0, totalFiles: 0 };
  }

  record(event: BackupProgressEvent): void {
    this.progressByServer[event.serverId] = {
      processedFiles: event.processedFiles,
      totalFiles: event.totalFiles,
    };
  }

  /** Clears the in-progress state, whether the backup succeeded or failed. */
  finish(serverId: string): void {
    delete this.progressByServer[serverId];
  }
}

export const backupsStore = new BackupsStore();
