// Short confirmations at the bottom of the panel ("Saved as Playing",
// "Posted"). One at a time: a new toast replaces the current one.

import { createContext, type ComponentChildren } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";

import { Icon } from "./icons";

export interface Toast {
  text: string;
  tone?: "success" | "error";
  action?: { label: string; run: () => void };
}

const VISIBLE_MS = 3200;
/** Matches the toast-out animation in app.css. */
const LEAVE_MS = 160;

const ToastContext = createContext<(toast: Toast) => void>(() => {});

export const useToast = () => useContext(ToastContext);

export function ToastProvider({ children, dismissLabel }: { children: ComponentChildren; dismissLabel: string }) {
  const [current, setCurrent] = useState<(Toast & { id: number; leaving: boolean }) | null>(null);
  const nextId = useRef(0);

  const show = useCallback((toast: Toast) => {
    nextId.current += 1;
    setCurrent({ ...toast, id: nextId.current, leaving: false });
  }, []);

  const dismiss = useCallback((id: number) => {
    setCurrent((toast) => (toast?.id === id ? { ...toast, leaving: true } : toast));
    setTimeout(() => setCurrent((toast) => (toast?.id === id ? null : toast)), LEAVE_MS);
  }, []);

  useEffect(() => {
    if (!current || current.leaving) return;
    const timer = setTimeout(() => dismiss(current.id), VISIBLE_MS);
    return () => clearTimeout(timer);
  }, [current?.id, current?.leaving]);

  return (
    <ToastContext.Provider value={show}>
      {children}
      <div class="toast-region" role="status" aria-live="polite">
        {current && (
          <div key={current.id} class={`toast toast-${current.tone ?? "success"}`} data-leaving={current.leaving}>
            <span class="toast-icon">
              <Icon name={current.tone === "error" ? "close" : "check"} size={16} />
            </span>
            <span class="toast-text">{current.text}</span>
            {current.action && (
              <button
                class="toast-action"
                onClick={() => {
                  current.action!.run();
                  dismiss(current.id);
                }}
              >
                {current.action.label}
              </button>
            )}
            <button class="toast-close" aria-label={dismissLabel} onClick={() => dismiss(current.id)}>
              <Icon name="close" size={14} />
            </button>
          </div>
        )}
      </div>
    </ToastContext.Provider>
  );
}
