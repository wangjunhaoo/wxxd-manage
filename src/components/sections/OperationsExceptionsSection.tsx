/* 异常处理（exceptions）—— 二级 Tab 壳：通知待办 / 售后与纠纷 */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import NotificationsSection from "./NotificationsSection";
import AftersalesSection from "./AftersalesSection";

const TABS = [
  { key: "notifications", label: "通知待办" },
  { key: "aftersales", label: "售后与纠纷" },
];

export default function OperationsExceptionsSection() {
  const ctx = useApp();
  const [tab, setTab] = useState("notifications");
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
      {tab === "notifications" && <NotificationsSection />}
      {tab === "aftersales" && <AftersalesSection />}
    </>
  );
}
