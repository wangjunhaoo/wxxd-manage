/* ============================================================================
   今日工作台（workbench）—— Soft 居中英雄 + 4 KPI 磁贴 + 3 待办泳道 + 最近提醒
   忠实保留真实 IA（先铺货→采购→改价），套用 Soft 视觉。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Tile, Pill, Button } from "../primitives";

export default function OperationsDashboardSection() {
  const ctx = useApp();
  const d = ctx.dashboard.value;

  // ---- 本地派生计数 ----
  const collections = ctx.collectionTasks.value;
  const activeCollections = collections.filter(
    (t) => t.status === "pending" || t.status === "running",
  ).length;
  const readyCollections = collections.filter(
    (t) => t.status === "success" && t.publish_job_ids.length === 0,
  ).length;
  const failedCollections = collections.filter((t) => t.status === "failed").length;

  const purchase = ctx.purchaseTasks.value;
  const pendingPurchaseCount = purchase.filter((t) => t.status === "pending_purchase").length;
  const mappingCount = purchase.filter((t) => t.status === "needs_mapping").length;
  const supplierIssueCount = purchase.filter((t) =>
    [
      "supplier_out_of_stock",
      "supplier_price_changed",
      "supplier_quality_risk",
      "supplier_cancelled",
    ].includes(t.status),
  ).length;
  const shipmentTodoCount = ctx.deliveryShipments.value.filter((s) =>
    ["waiting_confirmation", "ready_to_send", "send_failed"].includes(s.status),
  ).length;

  const priceTodoCount =
    ctx.currentOrderPriceAdjustmentJob.value?.items.filter((it) =>
      ["pending", "submitting", "failed"].includes(it.status),
    ).length ?? 0;

  const salesTotals = ctx.productSalesAnalysisTotals.value;
  const latestNotifications = ctx.notifications.value.slice(0, 5);

  // ---- 导航助手 ----
  const showSection = (s: string) => {
    ctx.selectedSection.value = s;
  };
  const showPurchaseStatus = async (status: string) => {
    ctx.selectedSection.value = "procurement";
    ctx.purchaseStatusFilter.value = status;
    await ctx.refreshPurchaseTasks();
  };
  const showDeliveryStatus = async (status: string) => {
    ctx.selectedSection.value = "procurement";
    ctx.deliveryStatusFilter.value = status;
    await ctx.refreshDeliveryShipments();
  };
  const showSalesStatus = async (status: string) => {
    ctx.selectedSection.value = "analytics";
    ctx.productSalesAnalysisStatusFilter.value = status;
    await ctx.refreshProductSalesAnalysis();
  };

  return (
    <div className="pad">
      <div className="wrap">
        {/* Hero */}
        <div className="hero">
          <div className="hello">运营今日优先级 · Asia/Shanghai</div>
          <h1 className="greet">先铺货，再处理采购下单和物流回填</h1>
          <p className="subgreet">
            首页只放今天需要运营处理的动作；接口、任务日志和系统配置都收进设置区。
          </p>
          <div
            style={{
              display: "flex",
              gap: 12,
              justifyContent: "center",
              marginTop: 26,
              marginBottom: 44,
              flexWrap: "wrap",
            }}
          >
            <Button
              variant="graphite"
              icon="refresh"
              disabled={ctx.automationRunning.value}
              onClick={() => ctx.runOperationalAutomationOnce()}
            >
              {ctx.automationRunning.value ? "推进中…" : "自动推进一轮"}
            </Button>
            <Button icon="refresh" onClick={() => ctx.refreshAll()}>
              刷新数据
            </Button>
          </div>
        </div>

        {/* KPI 磁贴 */}
        <div className="sec-h">
          <h2>今日概览</h2>
        </div>
        <div className="metrics">
          <Tile
            icon="file"
            label="待处理订单"
            value={d?.pending_order_count ?? 0}
            tick="rose"
            hint="采购 / 发货 / 售后 / 超时"
            onClick={() => showSection("exceptions")}
          />
          <Tile
            icon="box"
            label="铺货异常商品"
            value={d?.failed_publish_product_count ?? 0}
            tick="amber"
            hint="只看需要人工处理的异常原因"
            onClick={() => showSection("publish")}
          />
          <Tile
            icon="bell"
            label="未读通知"
            value={d?.unread_notification_count ?? ctx.unreadNotificationCount.value}
            tick="blue"
            hint="铺货、采购、发货、售后异常"
            onClick={() => showSection("exceptions")}
          />
          <Tile
            icon="clock"
            label="运行中任务"
            value={d?.running_task_count ?? 0}
            hint={d?.controller_status || "主控机状态未知"}
            onClick={() => showSection("settings")}
          />
        </div>

        {/* 待办泳道 */}
        <div className="lanes" style={{ marginBottom: 16 }}>
          <div className="lane">
            <div className="lane-head">
              <h3>铺货</h3>
              <button className="btn-link" onClick={() => showSection("publish")}>
                进入铺货 ›
              </button>
            </div>
            <LaneRow label="采集中 / 待采集" value={activeCollections} onClick={() => showSection("publish")} />
            <LaneRow label="已采集待创建铺货" value={readyCollections} onClick={() => showSection("publish")} />
            <LaneRow
              label="采集失败"
              value={failedCollections}
              tone="r"
              onClick={() => showSection("publish")}
            />
          </div>

          <div className="lane">
            <div className="lane-head">
              <h3>采购下单</h3>
              <button className="btn-link" onClick={() => showSection("procurement")}>
                进入采购 ›
              </button>
            </div>
            <LaneRow
              label="待采购"
              value={pendingPurchaseCount}
              tone="b"
              onClick={() => showPurchaseStatus("pending_purchase")}
            />
            <LaneRow
              label="缺货源信息"
              value={mappingCount}
              tone="a"
              onClick={() => showPurchaseStatus("needs_mapping")}
            />
            <LaneRow
              label="供应商异常"
              value={supplierIssueCount}
              tone="r"
              onClick={() => showPurchaseStatus("all")}
            />
            <LaneRow
              label="待发货 / 发货失败"
              value={shipmentTodoCount}
              onClick={() => showDeliveryStatus("all")}
            />
          </div>

          <div className="lane">
            <div className="lane-head">
              <h3>价格与经营</h3>
              <button className="btn-link" onClick={() => showSection("price")}>
                进入改价 ›
              </button>
            </div>
            <LaneRow label="当前订单改价" value={priceTodoCount} onClick={() => showSection("price")} />
            <LaneRow
              label="可继续放量商品"
              value={salesTotals.scale_candidate_count}
              tone="g"
              onClick={() => showSalesStatus("scale_candidate")}
            />
            <LaneRow
              label="库存 / 毛利风险"
              value={salesTotals.risk_product_count}
              tone="a"
              onClick={() => showSection("analytics")}
            />
            <LaneRow
              label="缺采购成本"
              value={salesTotals.missing_cost_product_count}
              onClick={() => showSection("analytics")}
            />
          </div>
        </div>

        {/* 最近提醒 */}
        <div className="panel tight">
          <div className="ph">
            <div>
              <h3>最近提醒</h3>
              <p>只展示脱敏摘要，点提醒进入对应运营页。</p>
            </div>
            <button className="more" onClick={() => showSection("exceptions")}>
              查看全部 ›
            </button>
          </div>
          {latestNotifications.length === 0 ? (
            <div className="empty" style={{ margin: "6px 2px 12px" }}>
              当前没有新的运营提醒。
            </div>
          ) : (
            latestNotifications.map((n) => (
              <button key={n.id} className="nrow" onClick={() => ctx.openNotification(n)}>
                <Pill tone={ctx.notificationSeverityType(n.severity)}>
                  {ctx.notificationSeverityLabel(n.severity)}
                </Pill>
                <span className="ttl">
                  <span style={{ color: "var(--ink-3)", marginRight: 8 }}>
                    {ctx.notificationSourceLabel(n.source_type)}
                  </span>
                  {n.title}
                </span>
                <span className="tm">{ctx.formatDateTime(n.updated_at)}</span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

function LaneRow({
  label,
  value,
  tone,
  onClick,
}: {
  label: string;
  value: number;
  tone?: "b" | "a" | "r" | "g" | "n";
  onClick: () => void;
}) {
  return (
    <button className="lane-item" onClick={onClick}>
      <span className="li-l">
        <span className={`dot ${tone ?? "n"}`} />
        {label}
      </span>
      <span className="li-v">{value}</span>
    </button>
  );
}
