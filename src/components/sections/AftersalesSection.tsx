/* ============================================================================
   售后异常处理台（SUB 售后/纠纷）—— Soft 视觉移植
   ----------------------------------------------------------------------------
   忠实保留原 Vue 模板的 IA / 交互 / 中文文案：1 主面板 + 5 子面板 + 1 主售后表。
   仅本地记录/同步，不读取文件内容、不上传微信、不调用平台纠纷接口；同意/拒绝人工触发。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, PageHead, Select, Field } from "../primitives";
import type {
  AftersaleView,
  AftersaleEvidenceView,
  SupplierAftersaleFollowupView,
  GuaranteeOrderView,
} from "../../types/app";

export default function AftersalesSection() {
  const ctx = useApp();

  const aftersales = ctx.aftersales.value;
  const evidence = ctx.aftersaleEvidence.value;
  const followups = ctx.supplierAftersaleFollowups.value;
  const guaranteeOrders = ctx.guaranteeOrders.value;
  const rejectReasons = ctx.aftersaleRejectReasonOptions.value;

  // 凭证 / 协同筛选的「目标」下拉：首项「全部目标」+ 选项
  const targetTypeFilterOptions = [
    { value: "all", label: "全部目标" },
    ...ctx.evidenceTargetTypeOptions,
  ];
  const evidenceStatusFilterOptions = [
    { value: "all", label: "全部状态" },
    ...ctx.evidenceStatusOptions,
  ];
  const supplierStatusFilterOptions = [
    { value: "all", label: "全部状态" },
    ...ctx.supplierFollowupStatusOptions,
  ];

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* ---- 主面板头 ---- */}
        <PageHead
          eyebrow="售后 / 纠纷 · Asia/Shanghai"
          title="售后异常处理台"
          desc="同步微信售后列表和详情，保留状态、退款金额、关联订单和失败原因；支持人工责任归因、供应商赔付回款和人工触发的同意/拒绝动作。"
          actions={
            <>
              <Select
                value={ctx.aftersaleStatusFilter.value}
                onChange={(v) => {
                  ctx.aftersaleStatusFilter.value = v;
                  void ctx.refreshAftersales();
                }}
                width={150}
                options={[
                  { value: "active", label: "处理中" },
                  { value: "all", label: "全部状态" },
                  { value: "sync_failed", label: "同步失败" },
                  { value: "MERCHANT_PROCESSING", label: "待商家处理" },
                  { value: "MERCHANT_REFUND_SUCCESS", label: "退款成功" },
                  { value: "MERCHANT_RETURN_SUCCESS", label: "退货退款成功" },
                  { value: "USER_CANCELD", label: "用户取消" },
                  { value: "RETURN_CLOSED", label: "退货关闭" },
                ]}
              />
              <Button icon="refresh" size="sm" onClick={() => ctx.refreshAftersales()}>
                刷新
              </Button>
              <Button
                variant="accent"
                icon="refresh"
                size="sm"
                onClick={() => ctx.runAftersaleSyncOnce()}
              >
                同步售后
              </Button>
              <Button icon="refresh" size="sm" onClick={() => ctx.runGuaranteeSyncOnce()}>
                同步纠纷单
              </Button>
            </>
          }
        />

        {/* ---- KPI 条 ---- */}
        <div className="kpis cols-5" style={{ marginBottom: 16 }}>
          <div className="kpi">
            <span>当前筛选</span>
            <strong>
              {ctx.aftersaleStatusFilter.value === "all"
                ? "全部状态"
                : ctx.aftersaleStatusFilter.value}
            </strong>
          </div>
          <div className="kpi">
            <span>显示售后</span>
            <strong>{aftersales.length}</strong>
          </div>
          <div className="kpi">
            <span>匹配总数</span>
            <strong>{ctx.aftersaleTotal.value}</strong>
          </div>
          <div className="kpi">
            <span>纠纷单</span>
            <strong>
              {guaranteeOrders.length} / {ctx.guaranteeOrderTotal.value}
            </strong>
          </div>
          <div className="kpi">
            <span>本地凭证</span>
            <strong>
              {evidence.length} / {ctx.aftersaleEvidenceTotal.value}
            </strong>
          </div>
        </div>

        {/* ==================================================================
            子面板 A：本地凭证资料包
            ================================================================== */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="ph">
            <div>
              <h3>本地凭证资料包</h3>
              <p>
                只整理本地资料摘要、文件路径或来源链接；不会读取文件内容、上传微信或提交平台处理。
              </p>
            </div>
            <div className="toolbar">
              <Select
                value={ctx.evidenceTargetTypeFilter.value}
                onChange={(v) => {
                  ctx.evidenceTargetTypeFilter.value = v;
                  void ctx.refreshAftersaleEvidence();
                }}
                width={130}
                options={targetTypeFilterOptions}
              />
              <input
                className="inp"
                style={{ width: 150 }}
                placeholder="单据 ID/单号"
                value={ctx.evidenceTargetIdFilter.value}
                onChange={(e) => {
                  ctx.evidenceTargetIdFilter.value = e.target.value;
                }}
                onBlur={() => ctx.refreshAftersaleEvidence()}
              />
              <Select
                value={ctx.evidenceStatusFilter.value}
                onChange={(v) => {
                  ctx.evidenceStatusFilter.value = v;
                  void ctx.refreshAftersaleEvidence();
                }}
                width={130}
                options={evidenceStatusFilterOptions}
              />
              {ctx.evidenceTargetIdFilter.value && (
                <Button variant="ghost" size="sm" onClick={() => ctx.clearEvidenceTargetFilter()}>
                  清除单据
                </Button>
              )}
              <Button icon="refresh" size="sm" onClick={() => ctx.refreshAftersaleEvidence()}>
                刷新凭证
              </Button>
              <Select
                value={ctx.evidenceExportFormat.value}
                onChange={(v) => {
                  ctx.evidenceExportFormat.value = v;
                }}
                width={100}
                options={[
                  { value: "md", label: "MD" },
                  { value: "jsonl", label: "JSONL" },
                  { value: "json", label: "JSON" },
                ]}
              />
              <Button icon="upload" size="sm" onClick={() => ctx.exportAftersaleEvidence()}>
                导出资料包
              </Button>
            </div>
          </div>

          {/* 录入表单 */}
          <div className="form-grid cols-3">
            <Field label="目标类型">
              <Select
                value={ctx.evidenceForm.target_type}
                onChange={(v) => {
                  ctx.evidenceForm.target_type = v;
                }}
                placeholder="目标类型"
                options={ctx.evidenceTargetTypeOptions}
              />
            </Field>
            <Field label="单据 ID">
              <input
                className="inp"
                placeholder="售后/纠纷本地 ID 或微信单号"
                value={ctx.evidenceForm.target_id}
                onChange={(e) => {
                  ctx.evidenceForm.target_id = e.target.value;
                }}
              />
            </Field>
            <Field label="凭证类型">
              <Select
                value={ctx.evidenceForm.evidence_type}
                onChange={(v) => {
                  ctx.evidenceForm.evidence_type = v;
                }}
                placeholder="凭证类型"
                options={ctx.evidenceTypeOptions}
              />
            </Field>
            <Field label="整理状态">
              <Select
                value={ctx.evidenceForm.status}
                onChange={(v) => {
                  ctx.evidenceForm.status = v;
                }}
                placeholder="整理状态"
                options={ctx.evidenceStatusOptions}
              />
            </Field>
            <Field label="凭证标题">
              <input
                className="inp"
                placeholder="凭证标题"
                value={ctx.evidenceForm.title}
                onChange={(e) => {
                  ctx.evidenceForm.title = e.target.value;
                }}
              />
            </Field>
            <Field label="本地文件路径">
              <input
                className="inp"
                placeholder="本地文件路径，可选"
                value={ctx.evidenceForm.local_file_path}
                onChange={(e) => {
                  ctx.evidenceForm.local_file_path = e.target.value;
                }}
              />
            </Field>
            <Field label="来源链接">
              <input
                className="inp"
                placeholder="来源链接，可选"
                value={ctx.evidenceForm.source_url}
                onChange={(e) => {
                  ctx.evidenceForm.source_url = e.target.value;
                }}
              />
            </Field>
            <Field label="凭证说明" span={2}>
              <input
                className="inp"
                placeholder="凭证说明，可选"
                value={ctx.evidenceForm.content_text}
                onChange={(e) => {
                  ctx.evidenceForm.content_text = e.target.value;
                }}
              />
            </Field>
            <div className="form-actions col-span-all">
              <span className="subtext">
                记录后仅进入本地资料包，后续平台举证仍需人工确认。
              </span>
              <Button variant="accent" icon="check" onClick={() => ctx.recordAftersaleEvidence()}>
                记录凭证
              </Button>
            </div>
          </div>

          {/* 凭证表 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>目标</th>
                  <th>单号</th>
                  <th>类型</th>
                  <th>标题</th>
                  <th>说明</th>
                  <th>资料来源</th>
                  <th>更新时间</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {evidence.map((row: AftersaleEvidenceView) => (
                  <tr key={row.id}>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>{row.status_text}</Pill>
                    </td>
                    <td>{row.target_type === "guarantee" ? "纠纷单" : "售后单"}</td>
                    <td>{row.external_target_id}</td>
                    <td>{row.evidence_type_text}</td>
                    <td>{row.title}</td>
                    <td>{row.content_text || "-"}</td>
                    <td>{row.local_file_path || row.source_url || "-"}</td>
                    <td>{ctx.formatDateTime(row.updated_at)}</td>
                    <td>
                      <div className="row-actions">
                        {row.status !== "ready" && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => ctx.updateAftersaleEvidenceStatus(row, "ready")}
                          >
                            已整理
                          </Button>
                        )}
                        {row.status !== "used" && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => ctx.updateAftersaleEvidenceStatus(row, "used")}
                          >
                            已使用
                          </Button>
                        )}
                        {row.status !== "archived" && (
                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => ctx.updateAftersaleEvidenceStatus(row, "archived")}
                          >
                            归档
                          </Button>
                        )}
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
          {ctx.evidenceExportPath.value && (
            <div className="action-row" style={{ marginTop: 10 }}>
              <Pill tone="info">最近导出：{ctx.evidenceExportPath.value}</Pill>
            </div>
          )}
        </div>

        {/* ==================================================================
            子面板 B：供应商协同记录
            ================================================================== */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="ph">
            <div>
              <h3>供应商协同记录</h3>
              <p>
                只记录售后/纠纷下的供应商沟通摘要，不自动登录供应商平台，不上传微信，不计入利润。
              </p>
            </div>
            <div className="toolbar">
              <Select
                value={ctx.supplierFollowupTargetTypeFilter.value}
                onChange={(v) => {
                  ctx.supplierFollowupTargetTypeFilter.value = v;
                  void ctx.refreshSupplierAftersaleFollowups();
                }}
                width={130}
                options={targetTypeFilterOptions}
              />
              <input
                className="inp"
                style={{ width: 150 }}
                placeholder="单据 ID/单号"
                value={ctx.supplierFollowupTargetIdFilter.value}
                onChange={(e) => {
                  ctx.supplierFollowupTargetIdFilter.value = e.target.value;
                }}
                onBlur={() => ctx.refreshSupplierAftersaleFollowups()}
              />
              <Select
                value={ctx.supplierFollowupStatusFilter.value}
                onChange={(v) => {
                  ctx.supplierFollowupStatusFilter.value = v;
                  void ctx.refreshSupplierAftersaleFollowups();
                }}
                width={130}
                options={supplierStatusFilterOptions}
              />
              {ctx.supplierFollowupTargetIdFilter.value && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => ctx.clearSupplierFollowupTargetFilter()}
                >
                  清除单据
                </Button>
              )}
              <Button
                icon="refresh"
                size="sm"
                onClick={() => ctx.refreshSupplierAftersaleFollowups()}
              >
                刷新协同
              </Button>
            </div>
          </div>

          {/* 录入表单 */}
          <div className="form-grid cols-3">
            <Field label="目标类型">
              <Select
                value={ctx.supplierFollowupForm.target_type}
                onChange={(v) => {
                  ctx.supplierFollowupForm.target_type = v;
                }}
                placeholder="目标类型"
                options={ctx.evidenceTargetTypeOptions}
              />
            </Field>
            <Field label="单据 ID">
              <input
                className="inp"
                placeholder="售后/纠纷本地 ID 或微信单号"
                value={ctx.supplierFollowupForm.target_id}
                onChange={(e) => {
                  ctx.supplierFollowupForm.target_id = e.target.value;
                }}
              />
            </Field>
            <Field label="协同类型">
              <Select
                value={ctx.supplierFollowupForm.followup_type}
                onChange={(v) => {
                  ctx.supplierFollowupForm.followup_type = v;
                }}
                placeholder="协同类型"
                options={ctx.supplierFollowupTypeOptions}
              />
            </Field>
            <Field label="协同状态">
              <Select
                value={ctx.supplierFollowupForm.status}
                onChange={(v) => {
                  ctx.supplierFollowupForm.status = v;
                }}
                placeholder="协同状态"
                options={ctx.supplierFollowupStatusOptions}
              />
            </Field>
            <Field label="供应商名称">
              <input
                className="inp"
                placeholder="供应商名称，可选"
                value={ctx.supplierFollowupForm.supplier_name}
                onChange={(e) => {
                  ctx.supplierFollowupForm.supplier_name = e.target.value;
                }}
              />
            </Field>
            <Field label="采购任务 ID">
              <input
                className="inp"
                placeholder="采购任务 ID，可选"
                value={ctx.supplierFollowupForm.purchase_task_id}
                onChange={(e) => {
                  ctx.supplierFollowupForm.purchase_task_id = e.target.value;
                }}
              />
            </Field>
            <Field label="协同备注" span={2}>
              <input
                className="inp"
                placeholder="协同备注，脱敏填写"
                value={ctx.supplierFollowupForm.note}
                onChange={(e) => {
                  ctx.supplierFollowupForm.note = e.target.value;
                }}
              />
            </Field>
            <div className="form-actions col-span-all">
              <span className="subtext">
                赔付入账仍走售后归因或纠纷跟进，这里只留协同流水。
              </span>
              <Button
                variant="accent"
                icon="check"
                onClick={() => ctx.recordSupplierAftersaleFollowup()}
              >
                记录协同
              </Button>
            </div>
          </div>

          {/* 协同表 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>目标</th>
                  <th>单号</th>
                  <th>类型</th>
                  <th>供应商</th>
                  <th>采购任务</th>
                  <th>协同备注</th>
                  <th>更新时间</th>
                </tr>
              </thead>
              <tbody>
                {followups.map((row: SupplierAftersaleFollowupView) => (
                  <tr key={row.id}>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>{row.status_text}</Pill>
                    </td>
                    <td>{row.target_type === "guarantee" ? "纠纷单" : "售后单"}</td>
                    <td>{row.external_target_id}</td>
                    <td>{row.followup_type_text}</td>
                    <td>{row.supplier_name || "-"}</td>
                    <td>{row.purchase_task_id || "-"}</td>
                    <td>{row.note}</td>
                    <td>{ctx.formatDateTime(row.updated_at)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        {/* ==================================================================
            子面板 C：责任归因（无表）
            ================================================================== */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="ph">
            <div>
              <h3>责任归因</h3>
              <p>先人工判断责任方；供应商赔付金额会在已关联订单上计入利润回款。</p>
            </div>
          </div>
          <div className="form-grid cols-3">
            <Field label="售后单 ID">
              <input
                className="inp"
                placeholder="售后单 ID 或微信售后单号"
                value={ctx.aftersaleResponsibilityForm.aftersale_id}
                onChange={(e) => {
                  ctx.aftersaleResponsibilityForm.aftersale_id = e.target.value;
                }}
              />
            </Field>
            <Field label="责任方">
              <Select
                value={ctx.aftersaleResponsibilityForm.responsibility_party}
                onChange={(v) => {
                  ctx.aftersaleResponsibilityForm.responsibility_party = v;
                }}
                placeholder="责任方"
                options={ctx.aftersaleResponsibilityOptions}
              />
            </Field>
            <Field label="供应商赔付金额">
              <input
                className="inp"
                placeholder="供应商赔付金额，单位分"
                value={ctx.aftersaleResponsibilityForm.supplier_compensation_cents}
                onChange={(e) => {
                  ctx.aftersaleResponsibilityForm.supplier_compensation_cents = e.target.value;
                }}
              />
            </Field>
            <Field label="处理备注">
              <input
                className="inp"
                placeholder="处理备注，可选"
                value={ctx.aftersaleResponsibilityForm.responsibility_note}
                onChange={(e) => {
                  ctx.aftersaleResponsibilityForm.responsibility_note = e.target.value;
                }}
              />
            </Field>
            <div className="form-actions col-span-all">
              <Button
                variant="accent"
                icon="check"
                onClick={() => ctx.recordAftersaleResponsibility()}
              >
                记录归因
              </Button>
            </div>
          </div>
        </div>

        {/* ==================================================================
            子面板 D：售后处理动作（无表）
            ================================================================== */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="ph">
            <div>
              <h3>售后处理动作</h3>
              <p>
                同意或拒绝必须人工点击触发；系统记录本次微信接口结果，并等待下次同步确认平台状态。
              </p>
            </div>
          </div>
          <div className="form-grid cols-3">
            <Field label="售后单 ID">
              <input
                className="inp"
                placeholder="售后单 ID 或微信售后单号"
                value={ctx.aftersaleActionForm.aftersale_id}
                onChange={(e) => {
                  ctx.aftersaleActionForm.aftersale_id = e.target.value;
                }}
              />
            </Field>
            <Field label="退货地址 ID">
              <input
                className="inp"
                placeholder="退货地址 ID，可选"
                value={ctx.aftersaleActionForm.address_id}
                onChange={(e) => {
                  ctx.aftersaleActionForm.address_id = e.target.value;
                }}
              />
            </Field>
            <Field label="同意类型">
              <Select
                value={ctx.aftersaleActionForm.accept_type}
                onChange={(v) => {
                  ctx.aftersaleActionForm.accept_type = v;
                }}
                options={[
                  { value: "", label: "平台按状态判断" },
                  { value: "1", label: "同意退货" },
                  { value: "2", label: "同意退款" },
                ]}
              />
            </Field>
            <Field label="拒绝原因类型">
              {/* 原 el-select filterable+allow-create：用 input+datalist 恢复「可选已有或输入自定义」，option 带场景说明 */}
              <input
                className="inp"
                list="reject-reason-types"
                placeholder="拒绝原因类型，必填（可选已有或输入自定义）"
                value={ctx.aftersaleActionForm.reject_reason_type}
                onChange={(e) => {
                  ctx.aftersaleActionForm.reject_reason_type = e.target.value;
                  ctx.applyAftersaleRejectReason(e.target.value);
                }}
              />
              <datalist id="reject-reason-types">
                {rejectReasons.map((reason) => (
                  <option
                    key={`${reason.shop_id}-${reason.reject_reason_type}`}
                    value={String(reason.reject_reason_type)}
                  >
                    {ctx.aftersaleRejectReasonLabel(reason)}
                    {reason.reject_scene_text ? ` · ${reason.reject_scene_text}` : ""}
                  </option>
                ))}
              </datalist>
            </Field>
            <Field label="拒绝原因" span={2}>
              <input
                className="inp"
                placeholder="拒绝原因，可选"
                value={ctx.aftersaleActionForm.reject_reason}
                onChange={(e) => {
                  ctx.aftersaleActionForm.reject_reason = e.target.value;
                }}
              />
            </Field>
            <Field label="处理备注" span={2}>
              <input
                className="inp"
                placeholder="处理备注，可选"
                value={ctx.aftersaleActionForm.note}
                onChange={(e) => {
                  ctx.aftersaleActionForm.note = e.target.value;
                }}
              />
            </Field>
            <div className="form-actions col-span-all">
              <span className="subtext">
                已缓存拒绝原因 {rejectReasons.length} 条
              </span>
              <Button icon="refresh" onClick={() => ctx.syncAftersaleRejectReasons()}>
                同步拒绝原因
              </Button>
              <Button variant="accent" icon="check" onClick={() => ctx.submitAftersaleAccept()}>
                提交同意
              </Button>
              <Button variant="danger" icon="x" onClick={() => ctx.submitAftersaleReject()}>
                提交拒绝
              </Button>
            </div>
          </div>
        </div>

        {/* ==================================================================
            子面板 E：纠纷 / 保障单
            ================================================================== */}
        <div className="panel tight" style={{ marginBottom: 16 }}>
          <div className="ph">
            <div>
              <h3>纠纷 / 保障单</h3>
              <p>
                同步微信保障单状态后，只记录本地跟进、责任归因和供应商赔付，不调用微信纠纷处理接口。
              </p>
            </div>
            <div className="toolbar">
              <Select
                value={ctx.guaranteeStatusFilter.value}
                onChange={(v) => {
                  ctx.guaranteeStatusFilter.value = v;
                  void ctx.refreshGuaranteeOrders();
                }}
                width={150}
                options={[
                  { value: "active", label: "待处理" },
                  { value: "all", label: "全部状态" },
                  { value: "sync_failed", label: "同步失败" },
                  { value: "STATUS_WAIT_MERCHANT_HANDLE", label: "等待商家处理" },
                  { value: "STATUS_WAIT_MERCHANT_PROOF", label: "等待商家举证" },
                  { value: "STATUS_WAIT_BOTH_PROOF", label: "等待双方举证" },
                  { value: "STATUS_PAY_SUCC", label: "赔付成功" },
                  { value: "STATUS_USER_CANCEL", label: "用户取消" },
                ]}
              />
              <Button icon="refresh" size="sm" onClick={() => ctx.refreshGuaranteeOrders()}>
                刷新纠纷单
              </Button>
              <Button icon="refresh" size="sm" onClick={() => ctx.runGuaranteeSyncOnce()}>
                同步纠纷单
              </Button>
            </div>
          </div>

          {/* 跟进表单 */}
          <div className="form-grid cols-3">
            <Field label="纠纷单 ID">
              <input
                className="inp"
                placeholder="纠纷单 ID 或微信纠纷单号"
                value={ctx.guaranteeFollowupForm.guarantee_order_id}
                onChange={(e) => {
                  ctx.guaranteeFollowupForm.guarantee_order_id = e.target.value;
                }}
              />
            </Field>
            <Field label="跟进状态">
              <Select
                value={ctx.guaranteeFollowupForm.handling_status}
                onChange={(v) => {
                  ctx.guaranteeFollowupForm.handling_status = v;
                }}
                placeholder="跟进状态"
                options={ctx.guaranteeHandlingStatusOptions}
              />
            </Field>
            <Field label="责任方">
              <Select
                value={ctx.guaranteeFollowupForm.responsibility_party}
                onChange={(v) => {
                  ctx.guaranteeFollowupForm.responsibility_party = v;
                }}
                placeholder="责任方"
                options={ctx.aftersaleResponsibilityOptions}
              />
            </Field>
            <Field label="供应商赔付">
              <input
                className="inp"
                placeholder="供应商赔付，单位分"
                value={ctx.guaranteeFollowupForm.supplier_compensation_cents}
                onChange={(e) => {
                  ctx.guaranteeFollowupForm.supplier_compensation_cents = e.target.value;
                }}
              />
            </Field>
            <Field label="纠纷跟进备注" span={2}>
              <input
                className="inp"
                placeholder="纠纷跟进备注，可选"
                value={ctx.guaranteeFollowupForm.handling_note}
                onChange={(e) => {
                  ctx.guaranteeFollowupForm.handling_note = e.target.value;
                }}
              />
            </Field>
            <div className="form-actions col-span-all">
              <span className="subtext">
                只更新本地记录和利润回款，不上传凭证或处理平台纠纷。
              </span>
              <Button variant="accent" icon="check" onClick={() => ctx.recordGuaranteeFollowup()}>
                记录纠纷跟进
              </Button>
            </div>
          </div>

          {/* 纠纷表 */}
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>店铺</th>
                  <th>纠纷单号</th>
                  <th>微信订单号</th>
                  <th>类型</th>
                  <th>本地跟进</th>
                  <th>凭证</th>
                  <th>赔付金额</th>
                  <th>供应商赔付</th>
                  <th>申请原因 / 异常</th>
                  <th>跟进备注</th>
                  <th>过期时间</th>
                  <th>跟进时间</th>
                  <th>同步时间</th>
                  <th>本地订单</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {guaranteeOrders.map((row: GuaranteeOrderView) => (
                  <tr key={row.id}>
                    <td>
                      <div className="cell-main">
                        <Pill tone={ctx.statusType(row.status)}>{row.status_text}</Pill>
                        <span className="subtext">{row.status}</span>
                      </div>
                    </td>
                    <td>{row.shop_name}</td>
                    <td>{row.guarantee_order_id}</td>
                    <td>{row.wechat_order_id || "-"}</td>
                    <td>{row.guarantee_type_text}</td>
                    <td>
                      <div className="cell-main">
                        <Pill tone="info">
                          {ctx.guaranteeHandlingStatusLabel(row.handling_status)}
                        </Pill>
                        <span className="subtext">
                          {ctx.aftersaleResponsibilityLabel(row.responsibility_party)}
                        </span>
                      </div>
                    </td>
                    <td>
                      <Pill tone={row.evidence_count > 0 ? "success" : "info"}>
                        {row.evidence_count} 条
                      </Pill>
                    </td>
                    <td>{ctx.formatCents(row.pay_amount_cents)}</td>
                    <td>{ctx.formatCents(row.supplier_compensation_cents)}</td>
                    <td>{row.apply_reason || row.merchant_refuse_reason || "-"}</td>
                    <td>{row.handling_note || "-"}</td>
                    <td>{ctx.formatUnixTime(row.expire_time)}</td>
                    <td>{ctx.formatDateTime(row.handled_at)}</td>
                    <td>{ctx.formatDateTime(row.synced_at)}</td>
                    <td>{row.order_id || "未关联"}</td>
                    <td>
                      <div className="row-actions">
                        <Button size="sm" onClick={() => ctx.selectGuaranteeFollowup(row)}>
                          跟进
                        </Button>
                        <Button
                          size="sm"
                          onClick={() => ctx.selectEvidenceTarget("guarantee", row)}
                        >
                          凭证
                        </Button>
                        <Button
                          size="sm"
                          onClick={() => ctx.selectSupplierFollowupTarget("guarantee", row)}
                        >
                          协同
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            ctx.exportAftersaleEvidence(
                              "guarantee",
                              row.id,
                              `纠纷单 ${row.guarantee_order_id}`,
                            )
                          }
                        >
                          导出凭证
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>

        {/* ==================================================================
            主售后表
            ================================================================== */}
        <div className="panel tight">
          <div className="tbl-wrap">
            <table className="tbl">
              <thead>
                <tr>
                  <th>状态</th>
                  <th>店铺</th>
                  <th>微信订单号</th>
                  <th>售后单号</th>
                  <th>类型</th>
                  <th>退款金额</th>
                  <th>责任归因</th>
                  <th>供应商赔付</th>
                  <th>最近动作</th>
                  <th>凭证</th>
                  <th>原因 / 异常</th>
                  <th>动作时间</th>
                  <th>处理时间</th>
                  <th>同步时间</th>
                  <th>本地订单</th>
                  <th>操作</th>
                </tr>
              </thead>
              <tbody>
                {aftersales.map((row: AftersaleView) => (
                  <tr key={row.id}>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>{row.status}</Pill>
                    </td>
                    <td>{row.shop_name}</td>
                    <td>{row.wechat_order_id || "-"}</td>
                    <td>{row.wechat_aftersale_id}</td>
                    <td>{row.aftersale_type || "-"}</td>
                    <td>{ctx.formatCents(row.refund_amount_cents)}</td>
                    <td>
                      <Pill
                        tone={
                          row.responsibility_party
                            ? ctx.statusType(row.responsibility_party)
                            : "info"
                        }
                      >
                        {ctx.aftersaleResponsibilityLabel(row.responsibility_party)}
                      </Pill>
                    </td>
                    <td>{ctx.formatCents(row.supplier_compensation_cents)}</td>
                    <td>
                      {row.last_action ? (
                        <div className="cell-main">
                          <Pill tone={ctx.statusType(row.last_action_status || "")}>
                            {ctx.aftersaleActionLabel(row.last_action)} ·{" "}
                            {ctx.aftersaleActionStatusLabel(row.last_action_status)}
                          </Pill>
                          {row.last_action_error ? (
                            <span className="subtext">{row.last_action_error}</span>
                          ) : row.last_action_note ? (
                            <span className="subtext">{row.last_action_note}</span>
                          ) : null}
                        </div>
                      ) : (
                        <span>-</span>
                      )}
                    </td>
                    <td>
                      <Pill tone={row.evidence_count > 0 ? "success" : "info"}>
                        {row.evidence_count} 条
                      </Pill>
                    </td>
                    <td>{row.reason || "-"}</td>
                    <td>{ctx.formatDateTime(row.last_action_at)}</td>
                    <td>{ctx.formatDateTime(row.handled_at)}</td>
                    <td>{ctx.formatDateTime(row.synced_at)}</td>
                    <td>{row.order_id || "未关联"}</td>
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          icon="check"
                          onClick={() => ctx.selectAftersaleAction(row)}
                        >
                          处理
                        </Button>
                        <Button size="sm" onClick={() => ctx.selectAftersaleResponsibility(row)}>
                          归因
                        </Button>
                        <Button
                          size="sm"
                          onClick={() => ctx.selectEvidenceTarget("aftersale", row)}
                        >
                          凭证
                        </Button>
                        <Button
                          size="sm"
                          onClick={() => ctx.selectSupplierFollowupTarget("aftersale", row)}
                        >
                          协同
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            ctx.exportAftersaleEvidence(
                              "aftersale",
                              row.id,
                              `售后单 ${row.wechat_aftersale_id}`,
                            )
                          }
                        >
                          导出凭证
                        </Button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      </div>
    </div>
  );
}
