// A promise-based confirmation dialog for destructive actions that happen
// outside a view with room for an inline "Sure?" step — the right-click menu,
// mainly. `ask()` resolves with the button the user picked.
//
// A request may offer a second course of action beside the main one: a port
// clash can be settled either by stopping the other server or by changing
// this one's port, so the answer is a choice rather than a yes/no.

export type ConfirmChoice = "confirm" | "secondary" | "cancel";

type ButtonVariant = "primary" | "soft" | "danger";

export interface ConfirmRequest {
  title: string;
  body: string;
  confirmLabel: string;
  variant?: ButtonVariant;
  /** An optional second course of action, shown beside the main one. */
  secondaryLabel?: string;
  secondaryVariant?: ButtonVariant;
}

class ConfirmStore {
  open = $state(false);
  title = $state("");
  body = $state("");
  confirmLabel = $state("");
  variant = $state<ButtonVariant>("primary");
  secondaryLabel = $state<string | null>(null);
  secondaryVariant = $state<ButtonVariant>("soft");

  private resolver: ((choice: ConfirmChoice) => void) | null = null;

  ask(request: ConfirmRequest): Promise<ConfirmChoice> {
    this.title = request.title;
    this.body = request.body;
    this.confirmLabel = request.confirmLabel;
    this.variant = request.variant ?? "primary";
    this.secondaryLabel = request.secondaryLabel ?? null;
    this.secondaryVariant = request.secondaryVariant ?? "soft";
    this.open = true;
    return new Promise((resolve) => {
      this.resolver = resolve;
    });
  }

  confirm(): void {
    this.finish("confirm");
  }

  chooseSecondary(): void {
    this.finish("secondary");
  }

  /** Cancel — also used when the dialog is dismissed any other way. */
  cancel(): void {
    this.finish("cancel");
  }

  private finish(choice: ConfirmChoice): void {
    this.open = false;
    const resolve = this.resolver;
    this.resolver = null;
    resolve?.(choice);
  }
}

export const confirmStore = new ConfirmStore();
