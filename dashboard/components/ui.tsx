"use client";

import { type ReactNode } from "react";

// ── StatCard ─────────────────────────────────────────────────────
export function StatCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: number | string;
  sub?: string;
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-950/60 p-4">
      <div className="text-[11px] font-semibold uppercase tracking-widest text-zinc-500">
        {label}
      </div>
      <div className="mt-1 text-2xl font-bold tabular-nums text-zinc-100">{value}</div>
      {sub && <div className="mt-0.5 text-xs text-zinc-500">{sub}</div>}
    </div>
  );
}

// ── Card ─────────────────────────────────────────────────────────
export function Card({
  title,
  action,
  children,
}: {
  title?: string;
  action?: ReactNode;
  children: ReactNode;
}) {
  return (
    <div className="rounded-xl border border-zinc-800 bg-zinc-950/60">
      {(title || action) && (
        <div className="flex items-center justify-between border-b border-zinc-800 px-4 py-3">
          {title && (
            <div className="text-xs font-semibold text-zinc-300">{title}</div>
          )}
          {action}
        </div>
      )}
      <div className="p-4">{children}</div>
    </div>
  );
}

// ── Tag ──────────────────────────────────────────────────────────
const TAG_COLORS = {
  green: "border-green-500/20 bg-green-500/10 text-green-400",
  amber: "border-amber-500/20 bg-amber-500/10 text-amber-400",
  red: "border-red-500/20 bg-red-500/10 text-red-400",
  blue: "border-blue-500/20 bg-blue-500/10 text-blue-400",
  zinc: "border-zinc-700/50 bg-zinc-800/50 text-zinc-400",
  ee: "border-stone-600/30 bg-stone-800/40 text-stone-400",
};

export function Tag({
  color = "zinc",
  children,
}: {
  color?: keyof typeof TAG_COLORS;
  children: ReactNode;
}) {
  return (
    <span
      className={`inline-flex items-center rounded-md border px-1.5 py-0.5 text-[10px] font-semibold leading-none ${TAG_COLORS[color]}`}
    >
      {children}
    </span>
  );
}

// ── EmptyState ───────────────────────────────────────────────────
export function EmptyState({ message }: { message: string }) {
  return (
    <div className="flex flex-col items-center justify-center py-12 text-center">
      <svg
        className="mb-3 h-10 w-10 text-zinc-700"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.2"
      >
        <rect x="3" y="3" width="18" height="18" rx="2" />
        <path d="M3 9h18M9 21V9" />
      </svg>
      <div className="text-sm text-zinc-500">{message}</div>
    </div>
  );
}

// ── Button ───────────────────────────────────────────────────────
const BTN = {
  primary:
    "bg-white text-zinc-900 hover:bg-zinc-200 active:bg-zinc-300",
  secondary:
    "border border-zinc-700 bg-zinc-900 text-zinc-300 hover:bg-zinc-800",
  danger:
    "border border-red-500/30 bg-red-500/10 text-red-400 hover:bg-red-500/20",
  ghost:
    "text-zinc-400 hover:text-zinc-200 hover:bg-white/[0.06]",
};

export function Button({
  variant = "primary",
  size = "sm",
  children,
  ...rest
}: {
  variant?: keyof typeof BTN;
  size?: "sm" | "md";
  children: ReactNode;
} & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return (
    <button
      className={`inline-flex items-center justify-center rounded-lg font-semibold transition-colors ${BTN[variant]} ${
        size === "sm" ? "px-2.5 py-1 text-[11px]" : "px-4 py-2 text-xs"
      }`}
      {...rest}
    >
      {children}
    </button>
  );
}

// ── Modal ────────────────────────────────────────────────────────
export function Modal({
  open,
  onClose,
  title,
  children,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
}) {
  if (!open) return null;
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/70 backdrop-blur-sm"
      onClick={(e) => e.target === e.currentTarget && onClose()}
      onKeyDown={(e) => e.key === "Escape" && onClose()}
    >
      <div className="w-full max-w-lg rounded-xl border border-zinc-800 bg-zinc-950 p-6 shadow-2xl">
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-sm font-semibold text-zinc-200">{title}</h3>
          <button
            onClick={onClose}
            className="rounded-md p-1 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-300"
          >
            <svg className="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

// ── Form helpers ─────────────────────────────────────────────────
export function FormField({
  label,
  children,
}: {
  label: string;
  children: ReactNode;
}) {
  return (
    <div>
      <label className="mb-1 block text-[11px] font-semibold uppercase tracking-widest text-zinc-500">
        {label}
      </label>
      {children}
    </div>
  );
}

const INPUT_CLS =
  "w-full rounded-lg border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm text-zinc-200 placeholder-zinc-600 outline-none transition-colors focus:border-zinc-500";

export function Input(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input className={INPUT_CLS} {...props} />;
}

export function Select(
  props: React.SelectHTMLAttributes<HTMLSelectElement> & { children: ReactNode },
) {
  return <select className={INPUT_CLS} {...props} />;
}

export function SearchInput({
  value,
  onChange,
  placeholder,
}: {
  value: string;
  onChange: (v: string) => void;
  placeholder?: string;
}) {
  return (
    <div className="relative">
      <svg
        className="pointer-events-none absolute left-2.5 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-zinc-500"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
      >
        <circle cx="11" cy="11" r="8" />
        <path d="M21 21l-4.35-4.35" />
      </svg>
      <input
        type="text"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder ?? "Search…"}
        className="w-full rounded-lg border border-zinc-700 bg-zinc-900 py-1.5 pl-8 pr-3 text-xs text-zinc-200 placeholder-zinc-600 outline-none focus:border-zinc-500"
      />
    </div>
  );
}

// ── Confirm dialog hook ──────────────────────────────────────────
import { useState } from "react";

export function useConfirm() {
  const [state, setState] = useState<{
    open: boolean;
    title: string;
    message: string;
    resolve: ((v: boolean) => void) | null;
  }>({ open: false, title: "", message: "", resolve: null });

  function confirm(title: string, message: string): Promise<boolean> {
    return new Promise((resolve) => {
      setState({ open: true, title, message, resolve });
    });
  }

  function ConfirmDialog() {
    if (!state.open) return null;
    const close = (v: boolean) => {
      state.resolve?.(v);
      setState({ open: false, title: "", message: "", resolve: null });
    };
    return (
      <Modal open title={state.title} onClose={() => close(false)}>
        <p className="mb-4 text-sm text-zinc-400">{state.message}</p>
        <div className="flex justify-end gap-2">
          <Button variant="secondary" onClick={() => close(false)}>
            Cancel
          </Button>
          <Button variant="danger" onClick={() => close(true)}>
            Delete
          </Button>
        </div>
      </Modal>
    );
  }

  return { confirm, ConfirmDialog };
}
