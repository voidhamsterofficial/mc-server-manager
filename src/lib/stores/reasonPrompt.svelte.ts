// A small promise-based prompt for an optional free-text reason, shared by the
// kick/ban actions. `ask()` resolves to the entered reason (empty string when
// the user confirms without typing one) or `null` when they cancel.

export interface ReasonRequest {
  title: string;
  actionLabel: string;
  variant: "primary" | "soft" | "danger";
  placeholder?: string;
}

const DEFAULT_PLACEHOLDER = "Reason (optional)…";

class ReasonPromptStore {
  open = $state(false);
  title = $state("");
  actionLabel = $state("");
  variant = $state<"primary" | "soft" | "danger">("primary");
  placeholder = $state(DEFAULT_PLACEHOLDER);
  value = $state("");

  private resolver: ((reason: string | null) => void) | null = null;

  ask(request: ReasonRequest): Promise<string | null> {
    this.title = request.title;
    this.actionLabel = request.actionLabel;
    this.variant = request.variant;
    this.placeholder = request.placeholder ?? DEFAULT_PLACEHOLDER;
    this.value = "";
    this.open = true;
    return new Promise((resolve) => {
      this.resolver = resolve;
    });
  }

  /** Confirm — resolves with the trimmed reason (may be empty). */
  confirm(): void {
    this.finish(this.value.trim());
  }

  /** Cancel — resolves with null so callers can abort the action. */
  cancel(): void {
    this.finish(null);
  }

  private finish(result: string | null): void {
    this.open = false;
    const resolve = this.resolver;
    this.resolver = null;
    resolve?.(result);
  }
}

export const reasonPromptStore = new ReasonPromptStore();
