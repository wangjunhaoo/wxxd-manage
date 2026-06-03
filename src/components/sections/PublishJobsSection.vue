<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type { CollectionTaskView } from "../../types/app";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  checkTaobaoLoginState,
  collectionAccessLimitState,
  collectionImportVisible,
  collectionProductSummary,
  collectionPublishedShopNames,
  collectionPublishJobText,
  collectionPublishing,
  collectionPublishTargetShopIds,
  collectionTaskPage,
  collectionTaskPageSize,
  collectionTaskPageSizeOptions,
  collectionTasks,
  collectionTaskTotal,
  collectionTesting,
  createPublishJobFromSelectedCollections,
  formatCents,
  formatDateTime,
  handleCollectionTaskPageChange,
  handleCollectionTaskPageSizeChange,
  isCollectionTaskSelected,
  isCurrentCollectionPageAllSelected,
  isCurrentCollectionPageIndeterminate,
  openPublishPricingDialog,
  openTestCollectDialog,
  paginatedCollectionTasks,
  publishPricingDialogVisible,
  publishPricingForm,
  publishPricingPreviewRows,
  publishPricingSaving,
  publishPricingSummary,
  Refresh,
  refreshCollectionTasks,
  runTestCollect,
  savePublishPricingStrategyFromForm,
  selectedCollectionTasks,
  selectedSection,
  setCollectionTaskSelected,
  shops,
  testCollectHeaded,
  testCollectResult,
  testCollectUrl,
  testCollectVisible,
  toggleCurrentCollectionPageSelection,
  triggerTaobaoLogin,
} = props.ctx;

type RowStatus = "collecting" | "ready" | "publishing" | "exception";

const fixedMarkupYuan = computed({
  get: () => publishPricingForm.value.sale_price_fixed_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_fixed_cents = Math.round(
      Number(value || 0) * 100,
    );
  },
});

const floorPriceYuan = computed({
  get: () => publishPricingForm.value.sale_price_floor_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_floor_cents = Math.round(
      Number(value || 0) * 100,
    );
  },
});

const publishStats = computed(() => {
  const stats = {
    total: collectionTasks.value.length,
    collecting: 0,
    ready: 0,
    publishing: 0,
    exception: 0,
  };
  for (const task of collectionTasks.value) {
    stats[toRowStatus(task)] += 1;
  }
  return stats;
});

const selectedCountText = computed(() => {
  const count = selectedCollectionTasks.value.length;
  return count > 0 ? `已选 ${count} 个商品` : "未选择商品";
});

const targetShopText = computed(() => {
  const count = collectionPublishTargetShopIds.value.length;
  return count > 0 ? `${count} 个目标小店` : "未选择目标小店";
});

const canCreatePublishJob = computed(() => {
  return (
    selectedCollectionTasks.value.length > 0 &&
    collectionPublishTargetShopIds.value.length > 0 &&
    !collectionPublishing.value
  );
});

function toRowStatus(task: CollectionTaskView): RowStatus {
  if ((task.publish_job_ids || []).length > 0) {
    return "publishing";
  }
  if (task.status === "failed" || task.review_status === "blocked") {
    return "exception";
  }
  if (task.status === "success" && Boolean(task.collected_data?.trim())) {
    return "ready";
  }
  return "collecting";
}

function rowStatusLabel(task: CollectionTaskView) {
  const labels: Record<RowStatus, string> = {
    collecting: "采集中",
    ready: "可铺货",
    publishing: "已进入铺货",
    exception: "异常",
  };
  return labels[toRowStatus(task)];
}

function rowStatusTone(task: CollectionTaskView) {
  const tones: Record<RowStatus, "info" | "success" | "primary" | "danger"> = {
    collecting: "info",
    ready: "success",
    publishing: "primary",
    exception: "danger",
  };
  return tones[toRowStatus(task)];
}

function rowIssueText(task: CollectionTaskView) {
  if (task.status === "failed") {
    return task.error_summary || "采集失败";
  }
  if (task.review_status === "blocked") {
    return task.review_summary || "商品不适合铺货";
  }
  return task.review_summary || task.error_summary || "-";
}

function onCurrentCollectionPageSelectionChange(
  value: boolean | string | number,
) {
  toggleCurrentCollectionPageSelection(Boolean(value));
}

function onCollectionTaskSelectionChange(
  row: CollectionTaskView,
  value: boolean | string | number,
) {
  setCollectionTaskSelected(row, Boolean(value));
}
</script>

<template>
  <section class="content-stack publish-simple-page">
    <div class="panel publish-workflow-panel">
      <div class="panel-title publish-workflow-title">
        <div>
          <h2>采集商品铺货</h2>
          <p>选商品和目标小店，创建任务后系统自动完成微信发布和上架确认。</p>
        </div>
        <div class="workflow-primary-actions">
          <el-button @click="openPublishPricingDialog">价格策略</el-button>
          <el-button type="primary" @click="collectionImportVisible = true">
            导入铺货表
          </el-button>
          <el-dropdown trigger="click">
            <el-button>采集工具</el-button>
            <template #dropdown>
              <el-dropdown-menu>
                <el-dropdown-item @click="triggerTaobaoLogin">
                  淘宝登录
                </el-dropdown-item>
                <el-dropdown-item @click="checkTaobaoLoginState">
                  检测登录态
                </el-dropdown-item>
                <el-dropdown-item @click="openTestCollectDialog">
                  测试抓取
                </el-dropdown-item>
              </el-dropdown-menu>
            </template>
          </el-dropdown>
        </div>
      </div>

      <div class="publish-workflow-stages">
        <div class="workflow-stage is-active">
          <span class="workflow-index">1</span>
          <div>
            <strong>选择商品</strong>
            <p>
              共 {{ publishStats.total }} 个，{{ publishStats.ready }} 个可铺货
            </p>
          </div>
        </div>
        <div
          class="workflow-stage"
          :class="{ 'is-active': collectionPublishTargetShopIds.length > 0 }"
        >
          <span class="workflow-index">2</span>
          <div>
            <strong>选择小店</strong>
            <p>{{ targetShopText }}</p>
          </div>
        </div>
        <div
          class="workflow-stage"
          :class="{ 'is-active': selectedCollectionTasks.length > 0 }"
        >
          <span class="workflow-index">3</span>
          <div>
            <strong>开始铺货</strong>
            <p>自动审查、发品、审核同步和上架确认</p>
          </div>
        </div>
      </div>

      <el-alert
        v-if="
          collectionAccessLimitState?.access_limited ||
          collectionAccessLimitState?.captcha_cooling
        "
        type="warning"
        :closable="false"
        show-icon
        style="margin-top: 12px"
        :title="
          collectionAccessLimitState.message ||
          collectionAccessLimitState.captcha_cooling_message ||
          '淘宝账号处于本地保护冷却期，暂不建议采集。'
        "
      />
    </div>

    <div class="panel collection-publish-panel publish-simple-command">
      <div class="collection-publish-head">
        <div>
          <strong>铺货目标</strong>
          <span
            >只需要选择目标小店；微信类目、素材、发品和上架由系统推进。</span
          >
        </div>
        <el-button text @click="selectedSection = 'publish-tasks'">
          查看铺货结果
        </el-button>
      </div>
      <div class="collection-publish-main">
        <el-select
          v-model="collectionPublishTargetShopIds"
          multiple
          collapse-tags
          placeholder="选择要铺货到的微信小店"
        >
          <el-option
            v-for="shop in shops"
            :key="shop.id"
            :label="`${shop.name} (${shop.group_name})`"
            :value="shop.id"
          />
        </el-select>
        <el-button
          type="primary"
          size="large"
          :loading="collectionPublishing"
          :disabled="!canCreatePublishJob"
          @click="createPublishJobFromSelectedCollections"
        >
          创建并开始铺货
        </el-button>
      </div>
      <div class="collection-publish-meta">
        <span>{{ selectedCountText }}</span>
        <span>{{ targetShopText }}</span>
        <span>价格策略：{{ publishPricingSummary }}</span>
      </div>
    </div>

    <div class="panel collection-panel publish-selection-panel">
      <div class="panel-title">
        <div>
          <h2>已采集商品</h2>
          <p>勾选要铺货的商品；异常商品会在结果页按原因汇总。</p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="refreshCollectionTasks">
            刷新
          </el-button>
        </div>
      </div>

      <dl class="status-list compact publish-simple-stats">
        <div>
          <dt>可铺货</dt>
          <dd>{{ publishStats.ready }}</dd>
        </div>
        <div>
          <dt>铺货中</dt>
          <dd>{{ publishStats.publishing }}</dd>
        </div>
        <div>
          <dt>采集中</dt>
          <dd>{{ publishStats.collecting }}</dd>
        </div>
        <div>
          <dt>异常</dt>
          <dd>{{ publishStats.exception }}</dd>
        </div>
      </dl>

      <el-table :data="paginatedCollectionTasks" class="dense-table">
        <el-table-column width="48" fixed>
          <template #header>
            <el-checkbox
              :model-value="isCurrentCollectionPageAllSelected"
              :indeterminate="isCurrentCollectionPageIndeterminate"
              @change="onCurrentCollectionPageSelectionChange"
            />
          </template>
          <template #default="{ row }">
            <el-checkbox
              :model-value="isCollectionTaskSelected(row)"
              :disabled="toRowStatus(row) !== 'ready'"
              @change="onCollectionTaskSelectionChange(row, $event)"
            />
          </template>
        </el-table-column>
        <el-table-column label="商品" min-width="340" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="collection-product-cell">
              <strong>{{ row.title }}</strong>
              <a :href="row.source_url" target="_blank" class="link">
                {{ row.source_url }}
              </a>
              <small v-if="row.category_path" class="subtext">
                {{ row.category_path }}
              </small>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="采集数据" width="150" show-overflow-tooltip>
          <template #default="{ row }">
            {{ collectionProductSummary(row) }}
          </template>
        </el-table-column>
        <el-table-column label="状态" width="120">
          <template #default="{ row }">
            <el-tag :type="rowStatusTone(row)">
              {{ rowStatusLabel(row) }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="铺货记录" min-width="180" show-overflow-tooltip>
          <template #default="{ row }">
            <div class="collection-publish-record-cell">
              <span>{{ collectionPublishedShopNames(row) }}</span>
              <small class="subtext">{{ collectionPublishJobText(row) }}</small>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="异常原因" min-width="220" show-overflow-tooltip>
          <template #default="{ row }">
            <span
              :style="{
                color: toRowStatus(row) === 'exception' ? '#bd4c2f' : '#6c685e',
              }"
            >
              {{ rowIssueText(row) }}
            </span>
          </template>
        </el-table-column>
        <el-table-column label="创建时间" width="150">
          <template #default="{ row }">
            {{ formatDateTime(row.created_at) }}
          </template>
        </el-table-column>
      </el-table>

      <div class="collection-pagination">
        <span>
          共 {{ collectionTaskTotal }} 条，已选
          {{ selectedCollectionTasks.length }} 条
        </span>
        <el-pagination
          v-model:current-page="collectionTaskPage"
          v-model:page-size="collectionTaskPageSize"
          :page-sizes="collectionTaskPageSizeOptions"
          :total="collectionTaskTotal"
          background
          layout="sizes, prev, pager, next, jumper"
          @current-change="handleCollectionTaskPageChange"
          @size-change="handleCollectionTaskPageSizeChange"
        />
      </div>
    </div>

    <el-dialog
      v-model="publishPricingDialogVisible"
      title="铺货价格策略"
      width="780px"
      class="pricing-strategy-dialog"
    >
      <div class="pricing-strategy-body">
        <el-form label-width="110px" class="pricing-strategy-form">
          <el-form-item label="加价倍率">
            <el-input-number
              v-model="publishPricingForm.sale_price_markup_rate"
              :min="0.1"
              :max="100"
              :precision="2"
              :step="0.1"
            />
          </el-form-item>
          <el-form-item label="固定加价">
            <el-input-number
              v-model="fixedMarkupYuan"
              :min="0"
              :max="100000"
              :precision="2"
              :step="1"
            />
            <span class="form-suffix">元</span>
          </el-form-item>
          <el-form-item label="最低售价">
            <el-input-number
              v-model="floorPriceYuan"
              :min="0.01"
              :max="100000"
              :precision="2"
              :step="1"
            />
            <span class="form-suffix">元</span>
          </el-form-item>
        </el-form>

        <div class="pricing-preview-panel">
          <div class="pricing-preview-head">
            <strong>SKU 试算</strong>
            <span>按当前选中商品展示，最多 20 条</span>
          </div>
          <el-table
            v-if="publishPricingPreviewRows.length > 0"
            :data="publishPricingPreviewRows"
            class="dense-table compact-table"
            max-height="260"
          >
            <el-table-column
              prop="title"
              label="商品"
              min-width="190"
              show-overflow-tooltip
            />
            <el-table-column
              prop="sku_label"
              label="SKU"
              min-width="170"
              show-overflow-tooltip
            />
            <el-table-column label="采集成本" width="110">
              <template #default="{ row }">
                {{ row.cost_price_yuan.toFixed(2) }}
              </template>
            </el-table-column>
            <el-table-column label="上架售价" width="120">
              <template #default="{ row }">
                <strong>{{ formatCents(row.sale_price_cents) }}</strong>
              </template>
            </el-table-column>
          </el-table>
          <div v-else class="empty-hint">
            选择采集成功的商品后，可预览每个 SKU 的上架售价。
          </div>
        </div>
      </div>
      <template #footer>
        <el-button @click="publishPricingDialogVisible = false">
          取消
        </el-button>
        <el-button
          type="primary"
          :loading="publishPricingSaving"
          @click="savePublishPricingStrategyFromForm"
        >
          保存策略
        </el-button>
      </template>
    </el-dialog>

    <el-dialog
      v-model="testCollectVisible"
      title="测试抓取淘宝商品"
      width="720px"
    >
      <div class="inline-form">
        <el-input
          v-model="testCollectUrl"
          placeholder="https://item.taobao.com/item.htm?id=..."
          clearable
        />
        <el-button
          type="primary"
          :loading="collectionTesting"
          @click="runTestCollect"
        >
          开始抓取
        </el-button>
      </div>
      <div style="margin-top: 8px">
        <el-checkbox v-model="testCollectHeaded"> 使用可见浏览器 </el-checkbox>
      </div>
      <div v-if="testCollectResult" class="sub-panel" style="margin-top: 12px">
        <dl class="status-list compact">
          <div>
            <dt>结果</dt>
            <dd>
              <el-tag :type="testCollectResult.success ? 'success' : 'danger'">
                {{ testCollectResult.success ? "成功" : "失败" }}
              </el-tag>
            </dd>
          </div>
          <div v-if="!testCollectResult.success">
            <dt>错误</dt>
            <dd>{{ testCollectResult.error || "未知错误" }}</dd>
          </div>
          <div v-if="testCollectResult.raw">
            <dt>标题</dt>
            <dd>{{ testCollectResult.raw.title || "-" }}</dd>
          </div>
          <div v-if="testCollectResult.raw">
            <dt>主图</dt>
            <dd>{{ (testCollectResult.raw.images || []).length }}</dd>
          </div>
          <div v-if="testCollectResult.raw">
            <dt>SKU</dt>
            <dd>{{ (testCollectResult.raw.skus || []).length }}</dd>
          </div>
        </dl>
      </div>
    </el-dialog>
  </section>
</template>
