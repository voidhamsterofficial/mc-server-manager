// Transitions that only animate what genuinely appears.

import { onMount } from "svelte";
import { fade } from "svelte/transition";
import type { TransitionConfig } from "svelte/transition";

const DEFAULT_DURATION_MS = 120;

/**
 * A fade that stays silent for whatever is already on screen when a view is
 * first shown, and animates only what appears afterwards.
 *
 * Svelte replays intro transitions for every element inside a block when that
 * block is created. The server tabs are `{#if activeTab === …}` branches, so
 * each switch recreated the whole subtree and re-faded every row at once —
 * flicking between tabs looked like the UI was glitching. Content that was
 * already there should simply be there; a backup finishing or a player
 * joining while you watch still fades in.
 *
 * Call once during component setup and use the result as `in:introFade`.
 */
export function createIntroFade(durationMs = DEFAULT_DURATION_MS) {
  // Not reactive on purpose: a transition function reads this when the
  // element is created, which is exactly when the answer matters.
  let hasMounted = false;
  onMount(() => {
    hasMounted = true;
  });

  return function introFade(node: Element): TransitionConfig {
    if (!hasMounted) {
      return { duration: 0 };
    }
    return fade(node, { duration: durationMs });
  };
}
