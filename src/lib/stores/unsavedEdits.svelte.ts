// Tracks in-progress edits that navigating away would throw away.
//
// The file editor lives several components deep and is destroyed outright when
// the selected server changes, so it can't intercept that itself. It registers
// what's unsaved here instead, and the one place navigation happens asks
// before going anywhere.

import { confirmStore } from "./confirm.svelte";

class UnsavedEditsStore {
  /** What is currently unsaved (e.g. a file name), or null when nothing is. */
  description = $state<string | null>(null);

  /** Called by an editor as its content diverges from what's on disk. */
  set(description: string | null): void {
    this.description = description;
  }

  /** Drops the record without asking — for when the edits were saved, or the
   *  user already agreed to discard them. */
  clear(): void {
    this.description = null;
  }

  /**
   * Whether it's safe to navigate away, asking the user when it isn't.
   * Clears the record once they agree, so a second prompt can't stack up.
   */
  async confirmLeave(): Promise<boolean> {
    if (this.description === null) {
      return true;
    }
    const choice = await confirmStore.ask({
      title: "Discard unsaved changes?",
      body: `You've edited ${this.description} without saving. Leaving now throws those changes away.`,
      confirmLabel: "Discard changes",
      variant: "danger",
    });
    if (choice !== "confirm") {
      return false;
    }
    this.clear();
    return true;
  }
}

export const unsavedEditsStore = new UnsavedEditsStore();
