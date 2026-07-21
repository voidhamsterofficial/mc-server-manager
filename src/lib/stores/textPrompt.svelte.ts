// A small promise-based prompt for a single line of text: kick/ban reasons,
// and names for new or renamed files. `ask()` resolves to the trimmed input
// (empty when optional and left blank) or `null` when the user cancels.

export interface TextPromptRequest {
  title: string;
  actionLabel: string;
  variant: "primary" | "soft" | "danger";
  placeholder?: string;
  /** Prefilled text — a file's current name when renaming, say. */
  initialValue?: string;
  /** When true, confirming with a blank box is refused. */
  required?: boolean;
  /** Optional line under the field. Belongs to the caller — what's helpful
   *  for a kick reason is nonsense for a file name. */
  hint?: string;
}

const DEFAULT_PLACEHOLDER = "Reason (optional)…";

class TextPromptStore {
  open = $state(false);
  title = $state("");
  actionLabel = $state("");
  variant = $state<"primary" | "soft" | "danger">("primary");
  placeholder = $state(DEFAULT_PLACEHOLDER);
  required = $state(false);
  hint = $state<string | null>(null);
  value = $state("");

  private resolver: ((text: string | null) => void) | null = null;

  ask(request: TextPromptRequest): Promise<string | null> {
    this.title = request.title;
    this.actionLabel = request.actionLabel;
    this.variant = request.variant;
    this.placeholder = request.placeholder ?? DEFAULT_PLACEHOLDER;
    this.required = request.required ?? false;
    this.hint = request.hint ?? null;
    this.value = request.initialValue ?? "";
    this.open = true;
    return new Promise((resolve) => {
      this.resolver = resolve;
    });
  }

  /** Confirm — resolves with the trimmed text. A required prompt ignores a
   *  blank confirm rather than resolving with nothing useful. */
  confirm(): void {
    const trimmed = this.value.trim();
    if (this.required && trimmed === "") {
      return;
    }
    this.finish(trimmed);
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

export const textPromptStore = new TextPromptStore();
