/* ============================================================================
   反馈层 —— 以 React 复刻 Element Plus 的 ElMessage / ElMessageBox 命令式 API
   ----------------------------------------------------------------------------
   原 composable 中 318 处 ElMessage / ElMessageBox 调用只需把 import 源换成本文件，
   行为保持一致：
   - ElMessage.{success,error,warning,info}(text)  →  右下角 Soft 风格 toast
   - ElMessageBox.confirm(message, title?, options?) → Promise（取消时 reject('cancel')）
   - ElMessageBox.prompt(message, title?, options?)  → Promise<{ value, action }>
   FeedbackHost 在 App 根挂载一次即可。
   ============================================================================ */
import {
  useSyncExternalStore,
  useState,
  useRef,
  useEffect,
  type ReactNode,
} from "react";

type ToastType = "success" | "error" | "warning" | "info";

interface ToastItem {
  id: number;
  type: ToastType;
  message: string;
}

interface DialogState {
  id: number;
  kind: "confirm" | "prompt";
  message: ReactNode;
  title: string;
  confirmText: string;
  cancelText: string;
  type: ToastType;
  inputValue: string;
  inputPlaceholder: string;
  inputValidator?: (value: string) => boolean | string;
  inputPattern?: RegExp;
  resolve: (v: any) => void;
  reject: (e: any) => void;
}

// ---- 极简发布订阅 store ---------------------------------------------------
let seq = 1;
let toasts: ToastItem[] = [];
let dialogQueue: DialogState[] = [];
const subs = new Set<() => void>();

function emit() {
  subs.forEach((s) => s());
}
function subscribe(cb: () => void) {
  subs.add(cb);
  return () => subs.delete(cb);
}
const toastSnapshot = () => toasts;
const dialogSnapshot = () => dialogQueue;

function pushToast(type: ToastType, message: unknown) {
  const text =
    typeof message === "string"
      ? message
      : ((message as any)?.message ?? String(message));
  const id = seq++;
  toasts = [...toasts, { id, type, message: text }];
  emit();
  window.setTimeout(() => {
    toasts = toasts.filter((t) => t.id !== id);
    emit();
  }, 2600);
}

interface MsgBoxOptions {
  confirmButtonText?: string;
  cancelButtonText?: string;
  type?: ToastType;
  inputValue?: string;
  inputPlaceholder?: string;
  inputValidator?: (value: string) => boolean | string;
  inputPattern?: RegExp;
  dangerouslyUseHTMLString?: boolean;
}

function openDialog(
  kind: "confirm" | "prompt",
  message: ReactNode,
  title: string,
  options: MsgBoxOptions,
): Promise<any> {
  return new Promise((resolve, reject) => {
    dialogQueue = [
      ...dialogQueue,
      {
        id: seq++,
        kind,
        message,
        title: title || (kind === "prompt" ? "请输入" : "提示"),
        confirmText: options.confirmButtonText ?? "确定",
        cancelText: options.cancelButtonText ?? "取消",
        type: options.type ?? "info",
        inputValue: options.inputValue ?? "",
        inputPlaceholder: options.inputPlaceholder ?? "",
        inputValidator: options.inputValidator,
        inputPattern: options.inputPattern,
        resolve,
        reject,
      },
    ];
    emit();
  });
}

function closeActiveDialog() {
  dialogQueue = dialogQueue.slice(1);
  emit();
}

// ---- 对外命令式 API（命名与 Element Plus 对齐）-----------------------------
export const ElMessage = {
  success: (m: unknown) => pushToast("success", m),
  error: (m: unknown) => pushToast("error", m),
  warning: (m: unknown) => pushToast("warning", m),
  info: (m: unknown) => pushToast("info", m),
};

export const ElMessageBox = {
  confirm: (message: ReactNode, title?: string, options: MsgBoxOptions = {}) =>
    openDialog("confirm", message, title ?? "提示", options),
  prompt: (message: ReactNode, title?: string, options: MsgBoxOptions = {}) =>
    openDialog("prompt", message, title ?? "请输入", options),
};

// ---- 渲染宿主 -------------------------------------------------------------
const TONE_CLASS: Record<ToastType, string> = {
  success: "ok",
  error: "crit",
  warning: "warn",
  info: "info",
};

export function FeedbackHost() {
  const toastList = useSyncExternalStore(subscribe, toastSnapshot);
  const dialogs = useSyncExternalStore(subscribe, dialogSnapshot);
  const active = dialogs[0];

  return (
    <>
      <div className="toast-stack">
        {toastList.map((t) => (
          <div key={t.id} className={`toast show toast-${t.type}`}>
            <span className={`d pill ${TONE_CLASS[t.type]}`} aria-hidden />
            {t.message}
          </div>
        ))}
      </div>
      {active && <DialogCard key={active.id} dialog={active} />}
    </>
  );
}

function DialogCard({ dialog }: { dialog: DialogState }) {
  const [input, setInput] = useState(dialog.inputValue);
  const [err, setErr] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (dialog.kind === "prompt") inputRef.current?.focus();
  }, [dialog.kind]);

  function onConfirm() {
    if (dialog.kind === "prompt") {
      if (dialog.inputPattern && !dialog.inputPattern.test(input)) {
        setErr("输入格式不正确");
        return;
      }
      if (dialog.inputValidator) {
        const res = dialog.inputValidator(input);
        if (res !== true) {
          setErr(typeof res === "string" ? res : "输入不合法");
          return;
        }
      }
      dialog.resolve({ value: input, action: "confirm" });
    } else {
      dialog.resolve("confirm");
    }
    closeActiveDialog();
  }
  function onCancel() {
    dialog.reject("cancel");
    closeActiveDialog();
  }

  return (
    <div className="modal-overlay" onMouseDown={onCancel}>
      <div
        className="modal-card mb"
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
      >
        <h3 className="mb-title">{dialog.title}</h3>
        <div className="mb-body">{dialog.message}</div>
        {dialog.kind === "prompt" && (
          <>
            <input
              ref={inputRef}
              className="inp"
              style={{ marginTop: 14 }}
              value={input}
              placeholder={dialog.inputPlaceholder}
              onChange={(e) => {
                setInput(e.target.value);
                if (err) setErr("");
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") onConfirm();
              }}
            />
            {err && (
              <div style={{ marginTop: 8, fontSize: 12.5, color: "var(--rose-ink)" }}>
                {err}
              </div>
            )}
          </>
        )}
        <div className="mb-actions">
          <button className="btn sm" onClick={onCancel}>
            {dialog.cancelText}
          </button>
          <button className="btn graphite sm" onClick={onConfirm}>
            {dialog.confirmText}
          </button>
        </div>
      </div>
    </div>
  );
}
