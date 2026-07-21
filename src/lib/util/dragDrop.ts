// Subscribing to files dragged in from the OS file manager. Tauri delivers
// these on a single webview-wide stream rather than per-element, so each
// consumer listens for as long as it's mounted and decides for itself whether
// a given drop is meant for it.

import { getCurrentWebview, type DragDropEvent } from "@tauri-apps/api/webview";
import type { UnlistenFn } from "@tauri-apps/api/event";

export interface FileDropOptions {
  /** Called as the drag enters and leaves, for showing a drop target. */
  onHoverChange: (isOver: boolean) => void;
  /** Called with the dropped files' absolute paths. */
  onDrop: (paths: string[]) => void;
  /** Return false to ignore drags entirely (e.g. a modal is covering the
   *  drop zone, or an import is already running). */
  isAccepting?: () => boolean;
}

/**
 * Watches the webview's OS drag-drop stream. Returns a teardown function that
 * can be returned straight from an `$effect`, and which is safe to call before
 * the listener has finished registering.
 */
export function watchFileDrops(options: FileDropOptions): () => void {
  let unlisten: UnlistenFn | null = null;
  let isStale = false;

  function handle(event: DragDropEvent) {
    if (options.isAccepting?.() === false) {
      return;
    }
    if (event.type === "enter" || event.type === "over") {
      options.onHoverChange(true);
      return;
    }
    options.onHoverChange(false);
    if (event.type === "drop") {
      options.onDrop(event.paths);
    }
  }

  void getCurrentWebview()
    .onDragDropEvent((event) => handle(event.payload))
    .then((stop) => {
      // The effect may already have been torn down while this was in flight.
      if (isStale) {
        stop();
        return;
      }
      unlisten = stop;
    })
    .catch(() => {
      // Nothing to clean up, and a webview without a drag stream just means
      // no drag-and-drop — every affected surface has another way in.
    });

  return () => {
    isStale = true;
    unlisten?.();
  };
}

/** Keeps only the paths ending in `extension` (matched case-insensitively). */
export function filterByExtension(paths: string[], extension: string): string[] {
  const suffix = extension.toLowerCase();
  const matching = paths.filter((path) => path.toLowerCase().endsWith(suffix));
  return matching;
}
