/* ============================================================================
   AppShell —— 纤细 72px 图标栏 + bloom 工作区
   真实应用的 10 个一级导航（图标 + hover 提示），active = 白色胶囊 + 蓝色图标。
   ============================================================================ */
import { useState, type ReactNode } from "react";
import { Icon } from "./primitives";
import { useApp } from "../runtime/AppContext";

const RAIL_STORAGE_KEY = "wxxd.railExpanded";

export interface NavItem {
  key: string;
  label: string;
  desc: string;
  icon: string;
}

export const NAV_ITEMS: NavItem[] = [
  { key: "workbench", label: "今日工作台", desc: "先看待办", icon: "grid" },
  { key: "products", label: "商品管理", desc: "商品与店铺", icon: "package" },
  { key: "orders", label: "订单管理", desc: "订单与履约", icon: "receipt" },
  { key: "publish", label: "铺货工作台", desc: "采集到上架", icon: "box" },
  { key: "procurement", label: "采购下单", desc: "采购与物流", icon: "cart" },
  { key: "price", label: "价格调整", desc: "订单改价", icon: "tag" },
  { key: "exceptions", label: "异常处理", desc: "通知与售后", icon: "alert" },
  { key: "analytics", label: "经营分析", desc: "动销库存利润", icon: "chart" },
  { key: "skills", label: "Agent 技能", desc: "技能管理", icon: "sparkles" },
  { key: "settings", label: "系统设置", desc: "低频配置", icon: "settings" },
];

export const SECTION_META: Record<string, { eyebrow: string; title: string }> = {
  workbench: { eyebrow: "Asia/Shanghai", title: "运营今日工作台" },
  products: { eyebrow: "商品管理", title: "货源、铺货和动销状态" },
  orders: { eyebrow: "订单管理", title: "订单、采购、发货和售后状态" },
  publish: { eyebrow: "铺货工作台", title: "导入即自动采集、审查、铺货上架" },
  procurement: { eyebrow: "采购下单", title: "待采购订单与供应商物流" },
  price: { eyebrow: "价格调整", title: "未付款订单批量改价" },
  exceptions: { eyebrow: "异常处理", title: "通知、售后与纠纷" },
  analytics: { eyebrow: "经营分析", title: "动销、库存和利润" },
  skills: { eyebrow: "Agent 技能", title: "AI 技能管理与试跑" },
  settings: { eyebrow: "系统设置", title: "配置、集成与任务日志" },
};

function Rail() {
  const ctx = useApp();
  const section = ctx.selectedSection.value;

  // 侧栏展开态（图标 ↔ 图标+文字），持久化到 localStorage
  const [expanded, setExpanded] = useState<boolean>(() => {
    try {
      return localStorage.getItem(RAIL_STORAGE_KEY) === "1";
    } catch {
      return false;
    }
  });
  function toggleRail() {
    setExpanded((prev) => {
      const next = !prev;
      try {
        localStorage.setItem(RAIL_STORAGE_KEY, next ? "1" : "0");
      } catch {
        /* localStorage 不可用时忽略 */
      }
      return next;
    });
  }

  return (
    <aside className={`rail${expanded ? " open" : ""}`}>
      <div className="rail-top">
        <div className="logo" title="微信小店铺货中台">
          铺
        </div>
        <span className="brand">铺货中台</span>
        <button
          className="rail-toggle"
          type="button"
          onClick={toggleRail}
          title={expanded ? "收起菜单" : "展开菜单"}
          aria-label={expanded ? "收起菜单" : "展开菜单"}
          aria-expanded={expanded}
        >
          <Icon name="chevronRight" size={18} />
        </button>
      </div>
      {NAV_ITEMS.map((it) => (
        <button
          key={it.key}
          className={`nav-ic ${section === it.key ? "active" : ""}`}
          onClick={() => {
            ctx.selectedSection.value = it.key;
          }}
        >
          <Icon name={it.icon} size={21} />
          <span className="nav-label">{it.label}</span>
          <span className="tip">
            {it.label} · {it.desc}
          </span>
        </button>
      ))}
      <div className="sp" />
      <div className="rail-user" title="运营">
        <div className="avatar">W</div>
        <span className="user-label">运营</span>
      </div>
    </aside>
  );
}

export function AppShell({ children }: { children: ReactNode }) {
  return (
    <div className="app">
      <Rail />
      <main className="main">{children}</main>
    </div>
  );
}
