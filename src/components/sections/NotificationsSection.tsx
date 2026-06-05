/* ============================================================================
   通知中心 —— 运营侧操作类通知列表，支持状态/级别筛选、标记已读、定位跳转。
   忠实移植自 NotificationsSection.vue，视觉换用 Soft 设计系统。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select, Empty } from "../primitives";

export default function NotificationsSection() {
  const ctx = useApp();

  const statusOptions = [
    { label: "未读", value: "unread" },
    { label: "全部状态", value: "all" },
    { label: "已读", value: "read" },
  ];

  const severityOptions = [
    { label: "全部级别", value: "all" },
    { label: "严重", value: "critical" },
    { label: "提醒", value: "warning" },
    { label: "信息", value: "info" },
  ];

  const rows = ctx.notifications.value;

  return (
    <section className="content-stack">
      <div className="panel">
        {/* 面板头：标题 + 操作区 */}
        <div className="panel-title">
          <div>
            <h2>通知中心</h2>
            <p>集中展示铺货失败、采购映射缺失、履约发货失败、售后待处理和店铺同步异常。</p>
          </div>
          <div className="button-group">
            <Select
              value={ctx.notificationStatusFilter.value}
              onChange={(v) => {
                ctx.notificationStatusFilter.value = v;
                ctx.refreshNotifications();
              }}
              options={statusOptions}
              width={110}
            />
            <Select
              value={ctx.notificationSeverityFilter.value}
              onChange={(v) => {
                ctx.notificationSeverityFilter.value = v;
                ctx.refreshNotifications();
              }}
              options={severityOptions}
              width={110}
            />
            <Button icon="refresh" onClick={() => ctx.refreshNotifications()}>
              刷新
            </Button>
            <Button icon="check" onClick={() => ctx.markAllNotificationsRead()}>
              全部已读
            </Button>
          </div>
        </div>

        {/* KPI strip */}
        <dl className="status-list compact">
          <div>
            <dt>未读通知</dt>
            <dd>{ctx.unreadNotificationCount.value}</dd>
          </div>
          <div>
            <dt>严重未读</dt>
            <dd>{ctx.criticalNotificationCount.value}</dd>
          </div>
          <div>
            <dt>当前筛选</dt>
            <dd>{ctx.notificationTotal.value}</dd>
          </div>
        </dl>

        {/* 通知表格 */}
        <div className="tbl-wrap">
          <table className="tbl dense-table">
            <thead>
              <tr>
                <th style={{ width: 90 }}>级别</th>
                <th style={{ width: 90 }}>状态</th>
                <th style={{ minWidth: 120 }}>来源</th>
                <th style={{ minWidth: 130 }}>店铺</th>
                <th style={{ minWidth: 380 }}>通知</th>
                <th style={{ minWidth: 190 }}>更新时间</th>
                <th style={{ width: 170 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {rows.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <Empty>暂无数据</Empty>
                  </td>
                </tr>
              ) : (
                rows.map((row) => (
                  <tr key={row.id}>
                    {/* 级别 */}
                    <td>
                      <Pill tone={ctx.notificationSeverityType(row.severity)}>
                        {ctx.notificationSeverityLabel(row.severity)}
                      </Pill>
                    </td>
                    {/* 状态 */}
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>
                        {row.status === "unread" ? "未读" : "已读"}
                      </Pill>
                    </td>
                    {/* 来源 */}
                    <td>{ctx.notificationSourceLabel(row.source_type)}</td>
                    {/* 店铺 */}
                    <td>{row.shop_name || row.shop_id || "-"}</td>
                    {/* 通知 */}
                    <td>
                      <div className="cell-main">
                        <strong>{row.title}</strong>
                        <span className="subtext">{row.body}</span>
                      </div>
                    </td>
                    {/* 更新时间 */}
                    <td>{ctx.formatDateTime(row.updated_at)}</td>
                    {/* 操作 */}
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          icon="bell"
                          variant="ghost"
                          onClick={() => ctx.openNotification(row)}
                        >
                          定位
                        </Button>
                        {row.status !== "read" && (
                          <Button
                            size="sm"
                            icon="check"
                            variant="ghost"
                            onClick={() => ctx.markNotificationRead(row)}
                          >
                            已读
                          </Button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
