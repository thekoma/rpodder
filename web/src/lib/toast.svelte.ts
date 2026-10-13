// Short confirmations and errors, announced politely to screen readers.

export type ToastKind = 'ok' | 'error';

export interface Toast {
  id: number;
  kind: ToastKind;
  text: string;
}

let nextId = 1;
let _items = $state<Toast[]>([]);

function push(kind: ToastKind, text: string, ms: number) {
  const id = nextId++;
  _items = [..._items, { id, kind, text }];
  setTimeout(() => dismiss(id), ms);
}

export function dismiss(id: number) {
  _items = _items.filter(t => t.id !== id);
}

export const toasts = {
  get items() {
    return _items;
  },
};

export function notify(text: string) {
  push('ok', text, 3500);
}

export function notifyError(text: string) {
  push('error', text, 6000);
}
