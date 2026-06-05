/* 经营分析（analytics）—— 二级 Tab 壳：动销分析 / 库存风控 / 利润核算 */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import SalesSection from "./SalesSection";
import InventorySection from "./InventorySection";
import ProfitSection from "./ProfitSection";

const TABS = [
  { key: "sales", label: "动销分析" },
  { key: "inventory", label: "库存风控" },
  { key: "profit", label: "利润核算" },
];

export default function OperationsAnalyticsSection() {
  const ctx = useApp();
  const [tab, setTab] = useState("sales");
  return (
    <>
      <div className="pad" style={{ paddingBottom: 0 }}>
        <div className="wrap-wide">
          <div className="tabs" style={{ marginBottom: 0 }}>
            {TABS.map((t) => (
              <button
                key={t.key}
                className={`tab ${tab === t.key ? "on" : ""}`}
                onClick={() => setTab(t.key)}
              >
                {t.label}
              </button>
            ))}
            <button
              className="btn-link"
              style={{ marginLeft: "auto", alignSelf: "center" }}
              onClick={() => ctx.refreshAll()}
            >
              刷新全部
            </button>
          </div>
        </div>
      </div>
      {tab === "sales" && <SalesSection />}
      {tab === "inventory" && <InventorySection />}
      {tab === "profit" && <ProfitSection />}
    </>
  );
}
