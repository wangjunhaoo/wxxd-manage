/* 系统设置（settings）—— 二级 Tab 壳：店铺 / 类目 / AI / 任务 / 备份 */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import ShopsSection from "./ShopsSection";
import CatalogSection from "./CatalogSection";
import AiSettingsSection from "./AiSettingsSection";
import TasksSection from "./TasksSection";
import BackupSection from "./BackupSection";

const TABS = [
  { key: "shops", label: "店铺与密钥" },
  { key: "catalog", label: "类目规则" },
  { key: "ai", label: "AI 设置" },
  { key: "tasks", label: "任务日志" },
  { key: "backup", label: "数据备份" },
];

export default function SystemSettingsSection() {
  const ctx = useApp();
  const [tab, setTab] = useState("shops");
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
      {tab === "shops" && <ShopsSection />}
      {tab === "catalog" && <CatalogSection />}
      {tab === "ai" && <AiSettingsSection />}
      {tab === "tasks" && <TasksSection />}
      {tab === "backup" && <BackupSection />}
    </>
  );
}
