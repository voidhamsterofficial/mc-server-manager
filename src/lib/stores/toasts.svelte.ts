// Tiny toast notification store.

export type ToastKind = "success" | "error" | "info";

export interface Toast {
  id: number;
  message: string;
  kind: ToastKind;
}

const TOAST_LIFETIME_MS = 4200;

class ToastsStore {
  toasts = $state<Toast[]>([]);
  #nextId = 1;

  show(message: string, kind: ToastKind = "info"): void {
    const toast: Toast = { id: this.#nextId++, message, kind };
    this.toasts = [...this.toasts, toast];
    setTimeout(() => this.dismiss(toast.id), TOAST_LIFETIME_MS);
  }

  error(message: string): void {
    this.show(message, "error");
  }

  success(message: string): void {
    this.show(message, "success");
  }

  dismiss(id: number): void {
    this.toasts = this.toasts.filter((toast) => toast.id !== id);
  }
}

export const toastsStore = new ToastsStore();
