// Controller for the app-wide custom context menu.

import type { Component } from "svelte";

/** Semantic icon color for a menu item — mirrors the app's badge palette so
 *  menu icons carry the same at-a-glance meaning colorful emoji used to. */
export type MenuTone = "success" | "warning" | "info";

export interface MenuItem {
  label: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  icon?: Component<any>;
  /** Colors the icon strawberry-red; takes priority over `tone`. */
  danger?: boolean;
  tone?: MenuTone;
  disabled?: boolean;
  /** Nested items revealed on hover — e.g. the online players a command can
   *  be aimed at. An item with a submenu still runs its own `action` when
   *  clicked, so "Give item…" can prefill without picking a player. */
  submenu?: MenuItem[];
  /** Shown in place of an empty submenu (e.g. "nobody online"). */
  emptySubmenuLabel?: string;
  action: () => void;
}

export type MenuEntry = MenuItem | "separator";

/** Estimated menu metrics used to keep the menu inside the viewport. */
const MENU_WIDTH = 230;
const ITEM_HEIGHT = 34;
const SEPARATOR_HEIGHT = 9;
const MENU_PADDING = 12;
const VIEWPORT_MARGIN = 8;
const ANCHOR_GAP = 6;

/** Rough rendered height of the menu, used for placement before it exists. */
function estimateHeight(entries: MenuEntry[]): number {
  return (
    entries.reduce(
      (sum, entry) => sum + (entry === "separator" ? SEPARATOR_HEIGHT : ITEM_HEIGHT),
      0,
    ) + MENU_PADDING
  );
}

class ContextMenuStore {
  open = $state(false);
  x = $state(0);
  y = $state(0);
  /** When set, the menu is pinned by its bottom edge (distance from the
   *  viewport bottom) instead of its top — used for button popovers so the
   *  menu bottom lines up with the button whatever its height. */
  bottom = $state<number | null>(null);
  entries = $state<MenuEntry[]>([]);

  /** Opens at the cursor — for genuine right-click context menus. */
  show(event: MouseEvent, entries: MenuEntry[]): void {
    event.preventDefault();
    event.stopPropagation();

    const height = estimateHeight(entries);
    this.entries = entries;
    this.bottom = null;
    this.x = Math.min(event.clientX, window.innerWidth - MENU_WIDTH - VIEWPORT_MARGIN);
    this.y = Math.min(event.clientY, window.innerHeight - height - VIEWPORT_MARGIN);
    this.open = true;
  }

  /** Opens a menu-style button's menu so its bottom edge lines up with the
   *  bottom of `container` (e.g. the console panel), growing upward — so it
   *  reads as part of that panel rather than a floating popup. Horizontally it
   *  aligns to the clicked button. Falls back to the button if no container. */
  showAbove(event: MouseEvent, container: HTMLElement | null, entries: MenuEntry[]): void {
    event.preventDefault();
    event.stopPropagation();

    const button = event.currentTarget as HTMLElement;
    const buttonRect = button.getBoundingClientRect();
    const bottomEdge = (container ?? button).getBoundingClientRect().bottom;

    this.entries = entries;
    this.x = Math.min(
      Math.max(VIEWPORT_MARGIN, buttonRect.left),
      window.innerWidth - MENU_WIDTH - VIEWPORT_MARGIN,
    );
    // Pin the menu's bottom a small gap above the panel's bottom edge.
    this.bottom = window.innerHeight - bottomEdge + ANCHOR_GAP;
    this.open = true;
  }

  close(): void {
    this.open = false;
  }
}

export const contextMenuStore = new ContextMenuStore();
