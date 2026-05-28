<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  aftersaleActionForm,
  aftersaleActionLabel,
  aftersaleActionStatusLabel,
  aftersaleEvidence,
  aftersaleEvidenceTotal,
  aftersaleRejectReasonLabel,
  aftersaleRejectReasonOptions,
  aftersaleResponsibilityForm,
  aftersaleResponsibilityLabel,
  aftersaleResponsibilityOptions,
  aftersales,
  aftersaleStatusFilter,
  aftersaleTotal,
  applyAftersaleRejectReason,
  CircleCheck,
  CircleClose,
  clearEvidenceTargetFilter,
  clearSupplierFollowupTargetFilter,
  evidenceExportFormat,
  evidenceExportPath,
  evidenceForm,
  evidenceStatusFilter,
  evidenceStatusOptions,
  evidenceTargetIdFilter,
  evidenceTargetTypeFilter,
  evidenceTargetTypeOptions,
  evidenceTypeOptions,
  exportAftersaleEvidence,
  formatCents,
  formatDateTime,
  formatUnixTime,
  guaranteeFollowupForm,
  guaranteeHandlingStatusLabel,
  guaranteeHandlingStatusOptions,
  guaranteeOrders,
  guaranteeOrderTotal,
  guaranteeStatusFilter,
  recordAftersaleEvidence,
  recordAftersaleResponsibility,
  recordGuaranteeFollowup,
  recordSupplierAftersaleFollowup,
  Refresh,
  refreshAftersaleEvidence,
  refreshAftersales,
  refreshGuaranteeOrders,
  refreshSupplierAftersaleFollowups,
  runAftersaleSyncOnce,
  runGuaranteeSyncOnce,
  selectAftersaleAction,
  selectAftersaleResponsibility,
  selectEvidenceTarget,
  selectGuaranteeFollowup,
  selectSupplierFollowupTarget,
  statusType,
  submitAftersaleAccept,
  submitAftersaleReject,
  supplierAftersaleFollowups,
  supplierFollowupForm,
  supplierFollowupStatusFilter,
  supplierFollowupStatusOptions,
  supplierFollowupTargetIdFilter,
  supplierFollowupTargetTypeFilter,
  supplierFollowupTypeOptions,
  syncAftersaleRejectReasons,
  updateAftersaleEvidenceStatus,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>售后异常处理台</h2>
              <p>同步微信售后列表和详情，保留状态、退款金额、关联订单和失败原因；支持人工责任归因、供应商赔付回款和人工触发的同意/拒绝动作。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="aftersaleStatusFilter"
                class="status-filter"
                @change="refreshAftersales"
              >
                <el-option value="active" label="处理中" />
                <el-option value="all" label="全部状态" />
                <el-option value="sync_failed" label="同步失败" />
                <el-option value="MERCHANT_PROCESSING" label="待商家处理" />
                <el-option value="MERCHANT_REFUND_SUCCESS" label="退款成功" />
                <el-option value="MERCHANT_RETURN_SUCCESS" label="退货退款成功" />
                <el-option value="USER_CANCELD" label="用户取消" />
                <el-option value="RETURN_CLOSED" label="退货关闭" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshAftersales">刷新</el-button>
              <el-button type="primary" :icon="Refresh" @click="runAftersaleSyncOnce">同步售后</el-button>
              <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>当前筛选</dt>
              <dd>{{ aftersaleStatusFilter === "all" ? "全部状态" : aftersaleStatusFilter }}</dd>
            </div>
            <div>
              <dt>显示售后</dt>
              <dd>{{ aftersales.length }}</dd>
            </div>
            <div>
              <dt>匹配总数</dt>
              <dd>{{ aftersaleTotal }}</dd>
            </div>
            <div>
              <dt>纠纷单</dt>
              <dd>{{ guaranteeOrders.length }} / {{ guaranteeOrderTotal }}</dd>
            </div>
            <div>
              <dt>本地凭证</dt>
              <dd>{{ aftersaleEvidence.length }} / {{ aftersaleEvidenceTotal }}</dd>
            </div>
          </dl>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>本地凭证资料包</h2>
                <p>只整理本地资料摘要、文件路径或来源链接；不会读取文件内容、上传微信或提交平台处理。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="evidenceTargetTypeFilter"
                  class="status-filter"
                  @change="refreshAftersaleEvidence"
                >
                  <el-option value="all" label="全部目标" />
                  <el-option
                    v-for="item in evidenceTargetTypeOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-input
                  v-model="evidenceTargetIdFilter"
                  class="status-filter evidence-target-filter"
                  placeholder="单据 ID/单号"
                  clearable
                  @change="refreshAftersaleEvidence"
                  @clear="refreshAftersaleEvidence"
                />
                <el-select
                  v-model="evidenceStatusFilter"
                  class="status-filter"
                  @change="refreshAftersaleEvidence"
                >
                  <el-option value="all" label="全部状态" />
                  <el-option
                    v-for="item in evidenceStatusOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-button
                  v-if="evidenceTargetIdFilter"
                  text
                  @click="clearEvidenceTargetFilter"
                >
                  清除单据
                </el-button>
                <el-button :icon="Refresh" @click="refreshAftersaleEvidence">刷新凭证</el-button>
                <el-select v-model="evidenceExportFormat" class="status-filter">
                  <el-option value="md" label="MD" />
                  <el-option value="jsonl" label="JSONL" />
                  <el-option value="json" label="JSON" />
                </el-select>
                <el-button :icon="UploadFilled" @click="exportAftersaleEvidence">导出资料包</el-button>
              </div>
            </div>
            <div class="form-grid evidence-grid">
              <el-select v-model="evidenceForm.target_type" placeholder="目标类型">
                <el-option
                  v-for="item in evidenceTargetTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="evidenceForm.target_id" placeholder="售后/纠纷本地 ID 或微信单号" />
              <el-select v-model="evidenceForm.evidence_type" placeholder="凭证类型">
                <el-option
                  v-for="item in evidenceTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="evidenceForm.status" placeholder="整理状态">
                <el-option
                  v-for="item in evidenceStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="evidenceForm.title" placeholder="凭证标题" />
              <el-input v-model="evidenceForm.local_file_path" placeholder="本地文件路径，可选" />
              <el-input v-model="evidenceForm.source_url" placeholder="来源链接，可选" />
              <el-input v-model="evidenceForm.content_text" class="span-2" placeholder="凭证说明，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">记录后仅进入本地资料包，后续平台举证仍需人工确认。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordAftersaleEvidence">记录凭证</el-button>
              </div>
            </div>
            <el-table :data="aftersaleEvidence" class="dense-table">
              <el-table-column prop="status_text" label="状态" width="110">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="target_type" label="目标" width="100">
                <template #default="{ row }">
                  {{ row.target_type === "guarantee" ? "纠纷单" : "售后单" }}
                </template>
              </el-table-column>
              <el-table-column prop="external_target_id" label="单号" min-width="180" />
              <el-table-column prop="evidence_type_text" label="类型" width="120" />
              <el-table-column prop="title" label="标题" min-width="180" show-overflow-tooltip />
              <el-table-column prop="content_text" label="说明" min-width="240" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.content_text || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="资料来源" min-width="240" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.local_file_path || row.source_url || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="更新时间" min-width="190">
                <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
              </el-table-column>
              <el-table-column label="操作" width="210" fixed="right">
                <template #default="{ row }">
                  <div class="table-actions">
                    <el-button
                      v-if="row.status !== 'ready'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'ready')"
                    >
                      已整理
                    </el-button>
                    <el-button
                      v-if="row.status !== 'used'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'used')"
                    >
                      已使用
                    </el-button>
                    <el-button
                      v-if="row.status !== 'archived'"
                      size="small"
                      text
                      @click="updateAftersaleEvidenceStatus(row, 'archived')"
                    >
                      归档
                    </el-button>
                  </div>
                </template>
              </el-table-column>
            </el-table>
            <div v-if="evidenceExportPath" class="action-row">
              <el-tag>最近导出：{{ evidenceExportPath }}</el-tag>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>供应商协同记录</h2>
                <p>只记录售后/纠纷下的供应商沟通摘要，不自动登录供应商平台，不上传微信，不计入利润。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="supplierFollowupTargetTypeFilter"
                  class="status-filter"
                  @change="refreshSupplierAftersaleFollowups"
                >
                  <el-option value="all" label="全部目标" />
                  <el-option
                    v-for="item in evidenceTargetTypeOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-input
                  v-model="supplierFollowupTargetIdFilter"
                  class="status-filter evidence-target-filter"
                  placeholder="单据 ID/单号"
                  clearable
                  @change="refreshSupplierAftersaleFollowups"
                  @clear="refreshSupplierAftersaleFollowups"
                />
                <el-select
                  v-model="supplierFollowupStatusFilter"
                  class="status-filter"
                  @change="refreshSupplierAftersaleFollowups"
                >
                  <el-option value="all" label="全部状态" />
                  <el-option
                    v-for="item in supplierFollowupStatusOptions"
                    :key="item.value"
                    :value="item.value"
                    :label="item.label"
                  />
                </el-select>
                <el-button
                  v-if="supplierFollowupTargetIdFilter"
                  text
                  @click="clearSupplierFollowupTargetFilter"
                >
                  清除单据
                </el-button>
                <el-button :icon="Refresh" @click="refreshSupplierAftersaleFollowups">刷新协同</el-button>
              </div>
            </div>
            <div class="form-grid supplier-followup-grid">
              <el-select v-model="supplierFollowupForm.target_type" placeholder="目标类型">
                <el-option
                  v-for="item in evidenceTargetTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="supplierFollowupForm.target_id" placeholder="售后/纠纷本地 ID 或微信单号" />
              <el-select v-model="supplierFollowupForm.followup_type" placeholder="协同类型">
                <el-option
                  v-for="item in supplierFollowupTypeOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="supplierFollowupForm.status" placeholder="协同状态">
                <el-option
                  v-for="item in supplierFollowupStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input v-model="supplierFollowupForm.supplier_name" placeholder="供应商名称，可选" />
              <el-input v-model="supplierFollowupForm.purchase_task_id" placeholder="采购任务 ID，可选" />
              <el-input v-model="supplierFollowupForm.note" class="span-2" placeholder="协同备注，脱敏填写" />
              <div class="aftersale-action-buttons">
                <span class="subtext">赔付入账仍走售后归因或纠纷跟进，这里只留协同流水。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordSupplierAftersaleFollowup">记录协同</el-button>
              </div>
            </div>
            <el-table :data="supplierAftersaleFollowups" class="dense-table">
              <el-table-column prop="status_text" label="状态" width="120">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                </template>
              </el-table-column>
              <el-table-column prop="target_type" label="目标" width="100">
                <template #default="{ row }">
                  {{ row.target_type === "guarantee" ? "纠纷单" : "售后单" }}
                </template>
              </el-table-column>
              <el-table-column prop="external_target_id" label="单号" min-width="180" />
              <el-table-column prop="followup_type_text" label="类型" width="120" />
              <el-table-column prop="supplier_name" label="供应商" min-width="130">
                <template #default="{ row }">
                  {{ row.supplier_name || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="purchase_task_id" label="采购任务" min-width="180" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.purchase_task_id || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="note" label="协同备注" min-width="260" show-overflow-tooltip />
              <el-table-column label="更新时间" min-width="190">
                <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
              </el-table-column>
            </el-table>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>责任归因</h2>
                <p>先人工判断责任方；供应商赔付金额会在已关联订单上计入利润回款。</p>
              </div>
            </div>
            <div class="form-grid">
              <el-input v-model="aftersaleResponsibilityForm.aftersale_id" placeholder="售后单 ID 或微信售后单号" />
              <el-select v-model="aftersaleResponsibilityForm.responsibility_party" placeholder="责任方">
                <el-option
                  v-for="item in aftersaleResponsibilityOptions"
                  :key="item.value"
                  :label="item.label"
                  :value="item.value"
                />
              </el-select>
              <el-input
                v-model="aftersaleResponsibilityForm.supplier_compensation_cents"
                placeholder="供应商赔付金额，单位分"
              />
              <el-input v-model="aftersaleResponsibilityForm.responsibility_note" placeholder="处理备注，可选" />
              <el-button type="primary" :icon="CircleCheck" @click="recordAftersaleResponsibility">记录归因</el-button>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>售后处理动作</h2>
                <p>同意或拒绝必须人工点击触发；系统记录本次微信接口结果，并等待下次同步确认平台状态。</p>
              </div>
            </div>
            <div class="form-grid aftersale-action-grid">
              <el-input v-model="aftersaleActionForm.aftersale_id" placeholder="售后单 ID 或微信售后单号" />
              <el-input v-model="aftersaleActionForm.address_id" placeholder="退货地址 ID，可选" />
              <el-select v-model="aftersaleActionForm.accept_type" placeholder="同意类型">
                <el-option value="" label="平台按状态判断" />
                <el-option value="1" label="同意退货" />
                <el-option value="2" label="同意退款" />
              </el-select>
              <el-select
                v-model="aftersaleActionForm.reject_reason_type"
                filterable
                allow-create
                placeholder="拒绝原因类型，必填"
                @change="applyAftersaleRejectReason"
              >
                <el-option
                  v-for="reason in aftersaleRejectReasonOptions"
                  :key="`${reason.shop_id}-${reason.reject_reason_type}`"
                  :label="aftersaleRejectReasonLabel(reason)"
                  :value="String(reason.reject_reason_type)"
                >
                  <span>{{ aftersaleRejectReasonLabel(reason) }}</span>
                  <small class="option-subtext">{{ reason.reject_scene_text }}</small>
                </el-option>
              </el-select>
              <el-input v-model="aftersaleActionForm.reject_reason" class="span-2" placeholder="拒绝原因，可选" />
              <el-input v-model="aftersaleActionForm.note" class="span-2" placeholder="处理备注，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">已缓存拒绝原因 {{ aftersaleRejectReasonOptions.length }} 条</span>
                <el-button :icon="Refresh" @click="syncAftersaleRejectReasons">同步拒绝原因</el-button>
                <el-button type="primary" :icon="CircleCheck" @click="submitAftersaleAccept">提交同意</el-button>
                <el-button type="danger" :icon="CircleClose" @click="submitAftersaleReject">提交拒绝</el-button>
              </div>
            </div>
          </div>

          <div class="sub-panel">
            <div class="panel-title tight">
              <div>
                <h2>纠纷 / 保障单</h2>
                <p>同步微信保障单状态后，只记录本地跟进、责任归因和供应商赔付，不调用微信纠纷处理接口。</p>
              </div>
              <div class="button-group">
                <el-select
                  v-model="guaranteeStatusFilter"
                  class="status-filter"
                  @change="refreshGuaranteeOrders"
                >
                  <el-option value="active" label="待处理" />
                  <el-option value="all" label="全部状态" />
                  <el-option value="sync_failed" label="同步失败" />
                  <el-option value="STATUS_WAIT_MERCHANT_HANDLE" label="等待商家处理" />
                  <el-option value="STATUS_WAIT_MERCHANT_PROOF" label="等待商家举证" />
                  <el-option value="STATUS_WAIT_BOTH_PROOF" label="等待双方举证" />
                  <el-option value="STATUS_PAY_SUCC" label="赔付成功" />
                  <el-option value="STATUS_USER_CANCEL" label="用户取消" />
                </el-select>
                <el-button :icon="Refresh" @click="refreshGuaranteeOrders">刷新纠纷单</el-button>
                <el-button :icon="Refresh" @click="runGuaranteeSyncOnce">同步纠纷单</el-button>
              </div>
            </div>
            <div class="form-grid aftersale-action-grid">
              <el-input v-model="guaranteeFollowupForm.guarantee_order_id" placeholder="纠纷单 ID 或微信纠纷单号" />
              <el-select v-model="guaranteeFollowupForm.handling_status" placeholder="跟进状态">
                <el-option
                  v-for="item in guaranteeHandlingStatusOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-select v-model="guaranteeFollowupForm.responsibility_party" placeholder="责任方">
                <el-option
                  v-for="item in aftersaleResponsibilityOptions"
                  :key="item.value"
                  :value="item.value"
                  :label="item.label"
                />
              </el-select>
              <el-input
                v-model="guaranteeFollowupForm.supplier_compensation_cents"
                placeholder="供应商赔付，单位分"
              />
              <el-input v-model="guaranteeFollowupForm.handling_note" class="span-2" placeholder="纠纷跟进备注，可选" />
              <div class="aftersale-action-buttons">
                <span class="subtext">只更新本地记录和利润回款，不上传凭证或处理平台纠纷。</span>
                <el-button type="primary" :icon="CircleCheck" @click="recordGuaranteeFollowup">记录纠纷跟进</el-button>
              </div>
            </div>
            <el-table :data="guaranteeOrders" class="dense-table">
              <el-table-column prop="status" label="状态" min-width="190">
                <template #default="{ row }">
                  <el-tag :type="statusType(row.status)">{{ row.status_text }}</el-tag>
                  <small class="subtext">{{ row.status }}</small>
                </template>
              </el-table-column>
              <el-table-column prop="shop_name" label="店铺" min-width="130" />
              <el-table-column prop="guarantee_order_id" label="纠纷单号" min-width="190" />
              <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170">
                <template #default="{ row }">
                  {{ row.wechat_order_id || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="guarantee_type_text" label="类型" width="130" />
              <el-table-column label="本地跟进" min-width="160">
                <template #default="{ row }">
                  <el-tag type="info">{{ guaranteeHandlingStatusLabel(row.handling_status) }}</el-tag>
                  <small class="subtext">{{ aftersaleResponsibilityLabel(row.responsibility_party) }}</small>
                </template>
              </el-table-column>
              <el-table-column label="凭证" width="90">
                <template #default="{ row }">
                  <el-tag :type="row.evidence_count > 0 ? 'success' : 'info'">
                    {{ row.evidence_count }} 条
                  </el-tag>
                </template>
              </el-table-column>
              <el-table-column label="赔付金额" width="110">
                <template #default="{ row }">
                  {{ formatCents(row.pay_amount_cents) }}
                </template>
              </el-table-column>
              <el-table-column label="供应商赔付" width="120">
                <template #default="{ row }">
                  {{ formatCents(row.supplier_compensation_cents) }}
                </template>
              </el-table-column>
              <el-table-column prop="apply_reason" label="申请原因 / 异常" min-width="260" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.apply_reason || row.merchant_refuse_reason || "-" }}
                </template>
              </el-table-column>
              <el-table-column prop="handling_note" label="跟进备注" min-width="220" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.handling_note || "-" }}
                </template>
              </el-table-column>
              <el-table-column label="过期时间" min-width="150">
                <template #default="{ row }">
                  {{ formatUnixTime(row.expire_time) }}
                </template>
              </el-table-column>
              <el-table-column label="跟进时间" min-width="190">
                <template #default="{ row }">
                  {{ formatDateTime(row.handled_at) }}
                </template>
              </el-table-column>
              <el-table-column label="同步时间" min-width="190">
                <template #default="{ row }">{{ formatDateTime(row.synced_at) }}</template>
              </el-table-column>
              <el-table-column prop="order_id" label="本地订单" min-width="180" show-overflow-tooltip>
                <template #default="{ row }">
                  {{ row.order_id || "未关联" }}
                </template>
              </el-table-column>
              <el-table-column label="操作" width="260" fixed="right">
                <template #default="{ row }">
                  <div class="row-actions">
                    <el-button size="small" @click="selectGuaranteeFollowup(row)">跟进</el-button>
                    <el-button size="small" @click="selectEvidenceTarget('guarantee', row)">凭证</el-button>
                    <el-button size="small" @click="selectSupplierFollowupTarget('guarantee', row)">协同</el-button>
                    <el-button
                      size="small"
                      text
                      @click="exportAftersaleEvidence('guarantee', row.id, `纠纷单 ${row.guarantee_order_id}`)"
                    >
                      导出凭证
                    </el-button>
                  </div>
                </template>
              </el-table-column>
            </el-table>
          </div>

          <el-table :data="aftersales" class="dense-table">
            <el-table-column prop="status" label="状态" width="170">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="130" />
            <el-table-column prop="wechat_order_id" label="微信订单号" min-width="170">
              <template #default="{ row }">
                {{ row.wechat_order_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="wechat_aftersale_id" label="售后单号" min-width="210" />
            <el-table-column prop="aftersale_type" label="类型" width="120">
              <template #default="{ row }">
                {{ row.aftersale_type || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="退款金额" width="110">
              <template #default="{ row }">
                {{ formatCents(row.refund_amount_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="责任归因" width="130">
              <template #default="{ row }">
                <el-tag :type="row.responsibility_party ? statusType(row.responsibility_party) : 'info'">
                  {{ aftersaleResponsibilityLabel(row.responsibility_party) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="供应商赔付" width="120">
              <template #default="{ row }">
                {{ formatCents(row.supplier_compensation_cents) }}
              </template>
            </el-table-column>
            <el-table-column label="最近动作" min-width="190">
              <template #default="{ row }">
                <template v-if="row.last_action">
                  <el-tag :type="statusType(row.last_action_status || '')">
                    {{ aftersaleActionLabel(row.last_action) }} · {{ aftersaleActionStatusLabel(row.last_action_status) }}
                  </el-tag>
                  <small v-if="row.last_action_error" class="subtext">{{ row.last_action_error }}</small>
                  <small v-else-if="row.last_action_note" class="subtext">{{ row.last_action_note }}</small>
                </template>
                <span v-else>-</span>
              </template>
            </el-table-column>
            <el-table-column label="凭证" width="90">
              <template #default="{ row }">
                <el-tag :type="row.evidence_count > 0 ? 'success' : 'info'">
                  {{ row.evidence_count }} 条
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="reason" label="原因 / 异常" min-width="260" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.reason || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="动作时间" min-width="190">
              <template #default="{ row }">
                {{ formatDateTime(row.last_action_at) }}
              </template>
            </el-table-column>
            <el-table-column label="处理时间" min-width="190">
              <template #default="{ row }">
                {{ formatDateTime(row.handled_at) }}
              </template>
            </el-table-column>
            <el-table-column label="同步时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.synced_at) }}</template>
            </el-table-column>
            <el-table-column prop="order_id" label="本地订单" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.order_id || "未关联" }}
              </template>
            </el-table-column>
            <el-table-column label="操作" width="330" fixed="right">
              <template #default="{ row }">
                <div class="row-actions">
                  <el-button size="small" :icon="CircleCheck" @click="selectAftersaleAction(row)">处理</el-button>
                  <el-button size="small" @click="selectAftersaleResponsibility(row)">归因</el-button>
                  <el-button size="small" @click="selectEvidenceTarget('aftersale', row)">凭证</el-button>
                  <el-button size="small" @click="selectSupplierFollowupTarget('aftersale', row)">协同</el-button>
                  <el-button
                    size="small"
                    text
                    @click="exportAftersaleEvidence('aftersale', row.id, `售后单 ${row.wechat_aftersale_id}`)"
                  >
                    导出凭证
                  </el-button>
                </div>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
