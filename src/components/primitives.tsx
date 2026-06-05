/* ============================================================================
   Soft 设计系统 · React 原子组件
   覆盖真实业务页所需的全部控件：Icon / Button / Pill / Chip / Tile / Field /
   Select / Switch / Tabs / Segmented / Modal / Drawer / Pagination / Callout /
   Empty / PageHead。图标为 Lucide 风格描边路径（替代 Element Plus Icons）。
   ============================================================================ */
import {
  useEffect,
  useState,
  useRef,
  type ReactNode,
  type CSSProperties,
} from "react";

// ---- 图标集 ---------------------------------------------------------------
const ICON_PATHS: Record<string, string[]> = {
  grid: ["M3 3h7v9H3z", "M14 3h7v5h-7z", "M14 12h7v9h-7z", "M3 16h7v5H3z"],
  box: ["M21 8 12 3 3 8l9 5 9-5Z", "m3 8 0 8 9 5 9-5 0-8", "M12 13v8"],
  package: [
    "M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z",
    "m3.3 7 8.7 5 8.7-5",
    "M12 22V12",
  ],
  receipt: [
    "M4 2v20l2-1 2 1 2-1 2 1 2-1 2 1 2-1 2 1V2l-2 1-2-1-2 1-2-1-2 1-2-1-2 1Z",
    "M8 7h8",
    "M8 11h8",
    "M8 15h5",
  ],
  cart: [
    "M2 3h2l2.5 13h11l2-9H6",
    "M9 20a1.4 1.4 0 1 0 0-2.8A1.4 1.4 0 0 0 9 20z",
    "M18 20a1.4 1.4 0 1 0 0-2.8A1.4 1.4 0 0 0 18 20z",
  ],
  tag: [
    "M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z",
    "M7.5 7.5h.01",
  ],
  alert: [
    "M10.3 3.7 1.8 18a2 2 0 0 0 1.7 3h16.9a2 2 0 0 0 1.7-3L13.7 3.7a2 2 0 0 0-3.4 0Z",
    "M12 9v4",
    "M12 17h.01",
  ],
  chart: ["M3 3v18h18", "M18 17V9", "M13 17V5", "M8 17v-3"],
  sparkles: [
    "M9.937 15.5A2 2 0 0 0 8.5 14.063l-6.135-1.582a.5.5 0 0 1 0-.962L8.5 9.936A2 2 0 0 0 9.937 8.5l1.582-6.135a.5.5 0 0 1 .963 0L14.063 8.5A2 2 0 0 0 15.5 9.937l6.135 1.581a.5.5 0 0 1 0 .964L15.5 14.063a2 2 0 0 0-1.437 1.437l-1.582 6.135a.5.5 0 0 1-.963 0z",
    "M20 3v4",
    "M22 5h-4",
    "M4 17v2",
    "M5 18H3",
  ],
  settings: [
    "M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6z",
    "M19.4 15a1.6 1.6 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.6 1.6 0 0 0-2.7 1.1V21a2 2 0 1 1-4 0v-.1A1.6 1.6 0 0 0 7 19.4l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.6 1.6 0 0 0-1.1-2.7H3a2 2 0 1 1 0-4h.1A1.6 1.6 0 0 0 4.6 7l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.6 1.6 0 0 0 1.8.3H9a1.6 1.6 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.6 1.6 0 0 0 1 1.5 1.6 1.6 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.6 1.6 0 0 0-.3 1.8V9a1.6 1.6 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.6 1.6 0 0 0-1.5 1Z",
  ],
  refresh: ["M21 12a9 9 0 1 1-2.6-6.4", "M21 3v6h-6"],
  plus: ["M12 5v14", "M5 12h14"],
  arrowRight: ["M5 12h14", "m12 5 7 7-7 7"],
  check: ["m5 12 5 5 9-12"],
  x: ["M18 6 6 18", "m6 6 12 12"],
  clock: ["M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18z", "M12 7v5l3 2"],
  bell: ["M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9", "M10.3 21a1.94 1.94 0 0 0 3.4 0"],
  file: ["M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z", "M14 2v6h6", "M9 13h6M9 17h4"],
  upload: ["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M17 8l-5-5-5 5", "M12 3v12"],
  download: ["M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4", "M7 10l5 5 5-5", "M12 15V3"],
  search: ["M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16z", "m21 21-4.3-4.3"],
  edit: ["M12 20h9", "M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4Z"],
  trash: ["M3 6h18", "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2", "M10 11v6", "M14 11v6"],
  external: ["M15 3h6v6", "M10 14 21 3", "M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"],
  chevronDown: ["m6 9 6 6 6-6"],
  chevronRight: ["m9 18 6-6-6-6"],
  play: ["M5 3 19 12 5 21Z"],
  dollar: ["M12 2v20", "M17 5H9.5a3.5 3.5 0 0 0 0 7h5a3.5 3.5 0 0 1 0 7H6"],
  image: [
    "M19 3H5a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2V5a2 2 0 0 0-2-2z",
    "M8.5 10a1.5 1.5 0 1 0 0-3 1.5 1.5 0 0 0 0 3z",
    "m21 15-5-5L5 21",
  ],
};

export type IconName = keyof typeof ICON_PATHS;

export function Icon({
  name,
  size = 18,
  style,
}: {
  name: string;
  size?: number;
  style?: CSSProperties;
}) {
  const paths = ICON_PATHS[name] ?? [];
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
      style={style}
      aria-hidden="true"
    >
      {paths.map((d, i) => (
        <path key={i} d={d} />
      ))}
    </svg>
  );
}

// ---- Button ---------------------------------------------------------------
export type ButtonVariant = "outline" | "graphite" | "accent" | "ghost" | "danger";
export function Button({
  variant = "outline",
  size,
  icon,
  iconRight,
  iconOnly,
  children,
  onClick,
  disabled,
  type = "button",
  title,
}: {
  variant?: ButtonVariant;
  size?: "sm";
  icon?: string;
  iconRight?: string;
  iconOnly?: boolean;
  children?: ReactNode;
  onClick?: () => void;
  disabled?: boolean;
  type?: "button" | "submit";
  title?: string;
}) {
  const cls = [
    "btn",
    variant !== "outline" ? variant : "",
    size === "sm" ? "sm" : "",
    iconOnly ? "icon" : "",
  ]
    .filter(Boolean)
    .join(" ");
  const isz = size === "sm" ? 15 : 16;
  return (
    <button className={cls} onClick={onClick} disabled={disabled} type={type} title={title}>
      {icon ? <Icon name={icon} size={isz} /> : null}
      {children}
      {iconRight ? <Icon name={iconRight} size={isz} /> : null}
    </button>
  );
}

// ---- Pill (status) --------------------------------------------------------
// Element 色调 + kit 色调 → kit 类名
const TONE: Record<string, string> = {
  ok: "ok", success: "ok", info: "info", primary: "info", "": "neu",
  warn: "warn", warning: "warn", crit: "crit", danger: "crit", neu: "neu", info_neu: "neu",
};
export function Pill({
  tone = "neu",
  dot = true,
  children,
}: {
  tone?: string;
  dot?: boolean;
  children: ReactNode;
}) {
  const t = TONE[tone] ?? "neu";
  return (
    <span className={`pill ${t}`}>
      {dot && <span className="d" />}
      {children}
    </span>
  );
}

// ---- Chip (filter) --------------------------------------------------------
export function Chip({
  label,
  count,
  active,
  onClick,
}: {
  label: ReactNode;
  count?: number | null;
  active?: boolean;
  onClick?: () => void;
}) {
  return (
    <button className={`chip-f ${active ? "on" : ""}`} onClick={onClick}>
      {label}
      {count != null && <b>{count}</b>}
    </button>
  );
}

// ---- Tile (metric) --------------------------------------------------------
export function Tile({
  icon,
  label,
  value,
  hint,
  tick,
  onClick,
}: {
  icon: string;
  label: ReactNode;
  value: ReactNode;
  hint?: ReactNode;
  tick?: "rose" | "amber" | "teal" | "blue";
  onClick?: () => void;
}) {
  return (
    <button className="tile" onClick={onClick}>
      <div className="ic">
        <Icon name={icon} />
      </div>
      <div className="lab">{label}</div>
      <div className="num">{value}</div>
      <div className="hint">
        {tick && <span className={`tickr ${tick === "rose" ? "" : tick}`} />}
        {hint}
      </div>
    </button>
  );
}

// ---- Field ----------------------------------------------------------------
export function Field({
  label,
  children,
  span,
}: {
  label?: ReactNode;
  children: ReactNode;
  span?: 2;
}) {
  return (
    <div className={`field ${span === 2 ? "col-span-2" : ""}`}>
      {label && <label>{label}</label>}
      {children}
    </div>
  );
}

// ---- Select (native, styled) ----------------------------------------------
export interface SelectOption {
  label: ReactNode;
  value: string | number;
}
export function Select({
  value,
  onChange,
  options,
  placeholder,
  width,
  disabled,
  style,
}: {
  value: string | number;
  onChange: (v: string) => void;
  options: SelectOption[];
  placeholder?: string;
  width?: number | string;
  disabled?: boolean;
  style?: CSSProperties;
}) {
  return (
    <select
      className="sel"
      value={String(value)}
      disabled={disabled}
      onChange={(e) => onChange(e.target.value)}
      style={{ width: width ?? "100%", ...style }}
    >
      {placeholder && (
        <option value="" disabled>
          {placeholder}
        </option>
      )}
      {options.map((o) => (
        <option key={String(o.value)} value={String(o.value)}>
          {o.label as string}
        </option>
      ))}
    </select>
  );
}

// ---- Switch ---------------------------------------------------------------
export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label?: ReactNode;
  disabled?: boolean;
}) {
  return (
    <label className="switch">
      <input
        type="checkbox"
        checked={checked}
        disabled={disabled}
        onChange={(e) => onChange(e.target.checked)}
      />
      <span className="track" />
      {label}
    </label>
  );
}

// ---- Tabs -----------------------------------------------------------------
export function Tabs({
  tabs,
  active,
  onChange,
}: {
  tabs: { key: string; label: ReactNode }[];
  active: string;
  onChange: (k: string) => void;
}) {
  return (
    <div className="tabs">
      {tabs.map((t) => (
        <button
          key={t.key}
          className={`tab ${active === t.key ? "on" : ""}`}
          onClick={() => onChange(t.key)}
        >
          {t.label}
        </button>
      ))}
    </div>
  );
}

// ---- Segmented ------------------------------------------------------------
export function Segmented({
  options,
  value,
  onChange,
}: {
  options: { label: ReactNode; value: string }[];
  value: string;
  onChange: (v: string) => void;
}) {
  return (
    <div className="seg">
      {options.map((o) => (
        <button
          key={o.value}
          className={value === o.value ? "on" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

// ---- Callout --------------------------------------------------------------
export function Callout({
  tone = "info",
  children,
}: {
  tone?: "info" | "warn" | "crit" | "ok" | "neutral";
  children: ReactNode;
}) {
  return <div className={`callout ${tone === "neutral" ? "" : tone}`}>{children}</div>;
}

// ---- Empty ----------------------------------------------------------------
export function Empty({ children }: { children: ReactNode }) {
  return <div className="empty">{children}</div>;
}

// ---- PageHead -------------------------------------------------------------
export function PageHead({
  eyebrow,
  title,
  desc,
  actions,
}: {
  eyebrow?: ReactNode;
  title: ReactNode;
  desc?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="pagehead">
      <div>
        {eyebrow && <div className="eyebrow">{eyebrow}</div>}
        <h1>{title}</h1>
        {desc && <p>{desc}</p>}
      </div>
      {actions && <div className="actions">{actions}</div>}
    </div>
  );
}

// ---- Modal ----------------------------------------------------------------
export function Modal({
  open,
  title,
  onClose,
  children,
  footer,
  size,
}: {
  open: boolean;
  title?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  size?: "lg";
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="modal-overlay" onMouseDown={onClose}>
      <div
        className={`modal-card ${size === "lg" ? "lg" : ""}`}
        role="dialog"
        aria-modal="true"
        onMouseDown={(e) => e.stopPropagation()}
      >
        {title && (
          <div className="modal-head">
            <h3>{title}</h3>
            <button className="x" onClick={onClose} aria-label="关闭">
              <Icon name="x" size={16} />
            </button>
          </div>
        )}
        {children}
        {footer && <div className="mb-actions">{footer}</div>}
      </div>
    </div>
  );
}

// ---- Drawer ---------------------------------------------------------------
export function Drawer({
  open,
  title,
  onClose,
  children,
  wide,
}: {
  open: boolean;
  title?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  wide?: boolean;
}) {
  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  return (
    <div className="drawer-overlay" onMouseDown={onClose}>
      <div className={`drawer ${wide ? "wide" : ""}`} onMouseDown={(e) => e.stopPropagation()}>
        {title && (
          <div className="modal-head">
            <h3>{title}</h3>
            <button className="x" onClick={onClose} aria-label="关闭">
              <Icon name="x" size={16} />
            </button>
          </div>
        )}
        {children}
      </div>
    </div>
  );
}

// ---- Dropdown -------------------------------------------------------------
export interface DropdownItem {
  label: ReactNode;
  onClick: () => void;
  disabled?: boolean;
}
export function Dropdown({
  label,
  icon,
  items,
  size,
}: {
  label: ReactNode;
  icon?: string;
  items: DropdownItem[];
  size?: "sm";
}) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDoc = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", onDoc);
    return () => window.removeEventListener("mousedown", onDoc);
  }, [open]);
  return (
    <div className="dropdown" ref={ref}>
      <Button icon={icon} iconRight="chevronDown" size={size} onClick={() => setOpen((o) => !o)}>
        {label}
      </Button>
      {open && (
        <div className="dropdown-menu">
          {items.map((it, i) => (
            <button
              key={i}
              className="dropdown-item"
              disabled={it.disabled}
              onClick={() => {
                it.onClick();
                setOpen(false);
              }}
            >
              {it.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

// ---- Pagination -----------------------------------------------------------
export function Pagination({
  page,
  pageSize,
  total,
  onChange,
}: {
  page: number;
  pageSize: number;
  total: number;
  onChange: (page: number) => void;
}) {
  const pageCount = Math.max(1, Math.ceil(total / pageSize));
  if (pageCount <= 1) return null;
  const windowed: number[] = [];
  const from = Math.max(1, page - 2);
  const to = Math.min(pageCount, from + 4);
  for (let i = from; i <= to; i++) windowed.push(i);
  return (
    <div className="pager">
      <span className="text-muted">共 {total} 条</span>
      <div className="pages">
        <button disabled={page <= 1} onClick={() => onChange(page - 1)}>
          上一页
        </button>
        {windowed.map((p) => (
          <button key={p} className={p === page ? "on" : ""} onClick={() => onChange(p)}>
            {p}
          </button>
        ))}
        <button disabled={page >= pageCount} onClick={() => onChange(page + 1)}>
          下一页
        </button>
      </div>
    </div>
  );
}
