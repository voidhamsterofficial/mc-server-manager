// Controller for the app-wide custom context menu.

export interface MenuItem {
  label: string;
  emoji?: string;
  danger?: boolean;
  disabled?: boolean;
  action: () => void;
}

export type MenuEntry = MenuItem | "separator";

/** Estimated menu metrics used to keep the menu inside the viewport. */
const MENU_WIDTH = 230;
const ITEM_HEIGHT = 34;
const SEPARATOR_HEIGHT = 9;
const MENU_PADDING = 12;

class ContextMenuStore {
  open = $state(false);
  x = $state(0);
  y = $state(0);
  entries = $state<MenuEntry[]>([]);

  show(event: MouseEvent, entries: MenuEntry[]): void {
    event.preventDefault();
    event.stopPropagation();

    const height =
      entries.reduce(
        (sum, entry) => sum + (entry === "separator" ? SEPARATOR_HEIGHT : ITEM_HEIGHT),
        0,
      ) + MENU_PADDING;

    this.entries = entries;
    this.x = Math.min(event.clientX, window.innerWidth - MENU_WIDTH - 8);
    this.y = Math.min(event.clientY, window.innerHeight - height - 8);
    this.open = true;
  }

  close(): void {
    this.open = false;
  }
}

export const contextMenuStore = new ContextMenuStore();
