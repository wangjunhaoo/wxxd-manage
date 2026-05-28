<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  allowedValuesText,
  applyProductAttributeSuggestions,
  applySingleAttributeSuggestion,
  automationRunning,
  attributeKindLabel,
  attributeSuggestionApplying,
  attributeSuggestionsForItem,
  attributeSuggestionsForProduct,
  attributeSuggestionValue,
  checkTaobaoLoginState,
  clearCollectionHistory,
  clearTaobaoAccessLimitState,
  canSelectCollectionTask,
  collectionAccessLimitChecking,
  collectionAccessLimitState,
  collectionCheckingLogin,
  collectionDetailVisible,
  collectionImportVisible,
  collectionLoggingIn,
  collectionPublishedShopNames,
  collectionProductSummary,
  collectionPublishJobText,
  collectionPublishing,
  collectionPublishTargetShopIds,
  collectionReviewConfirming,
  collectionReviewing,
  collectionReviewStatusLabel,
  collectionReviewStatusType,
  collectionTaskPage,
  collectionTaskPageSize,
  collectionTaskPageSizeOptions,
  collectionTaskStatusLabel,
  collectionTaskTotal,
  collectionTesting,
  createPublishJobFromSelectedCollections,
  currentJob,
  formatCents,
  formatDateTime,
  handleCollectionTaskPageChange,
  handleCollectionTaskPageSizeChange,
  isCollectionTaskSelected,
  isCurrentCollectionPageAllSelected,
  isCurrentCollectionPageIndeterminate,
  openCollectionDetail,
  openPublishPricingDialog,
  openTestCollectDialog,
  paginatedCollectionTasks,
  pendingAttributeSuggestionCountForItem,
  pendingAttributeSuggestionsForProduct,
  publishPricingDialogVisible,
  publishPricingForm,
  publishPricingPreviewRows,
  publishPricingSaving,
  publishPricingSummary,
  queriedTaskId,
  queryJob,
  Refresh,
  refreshCollectionTasks,
  refreshJobAttributeSuggestions,
  refreshTaobaoAccessLimitState,
  resumeCollectionTasks,
  retryCollection,
  resetSelectedCollectionReview,
  reviewSingleCollectionTask,
  reviewSelectedCollectionTasks,
  runOperationalAutomationOnce,
  runPublishAiAttributeSuggestionsOnce,
  runPublishAssetUploadsOnce,
  runPublishAttributeFillOnce,
  runPublishCategoryPrechecksOnce,
  runPublishListingOnce,
  runPublishStatusSyncOnce,
  runPublishSubmitsOnce,
  runPublishTasksOnce,
  runTestCollect,
  savePublishPricingStrategyFromForm,
  Search,
  confirmSelectedCollectionReview,
  selectedCollectionDetailImages,
  selectedCollectionDetailJson,
  selectedCollectionDetailProduct,
  selectedCollectionDetailTask,
  selectedCollectionPublishable,
  selectedCollectionRemovedImages,
  selectedCollectionReviewIssues,
  selectedCollectionCategoryCandidates,
  selectedReviewCategoryKey,
  selectedCollectionMainImages,
  selectedCollectionSkuPreview,
  selectedCollectionTasks,
  setCollectionTaskSelected,
  statusType,
  shops,
  testCollectHeaded,
  testCollectResult,
  testCollectUrl,
  testCollectVisible,
  toggleCurrentCollectionPageSelection,
  triggerTaobaoLogin,
  UploadFilled,
} = props.ctx;

const fixedMarkupYuan = computed({
  get: () => publishPricingForm.value.sale_price_fixed_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_fixed_cents = Math.round(Number(value || 0) * 100);
  },
});

const floorPriceYuan = computed({
  get: () => publishPricingForm.value.sale_price_floor_cents / 100,
  set: (value: number) => {
    publishPricingForm.value.sale_price_floor_cents = Math.round(Number(value || 0) * 100);
  },
});

function onCurrentCollectionPageSelectionChange(value: boolean | string | number) {
  toggleCurrentCollectionPageSelection(Boolean(value));
}

function onCollectionTaskSelectionChange(
  row: Parameters<typeof setCollectionTaskSelected>[0],
  value: boolean | string | number,
) {
  setCollectionTaskSelected(row, Boolean(value));
}

function formatCollectionSkuSpecs(sku: any) {
  const specs = sku?.specs;
  if (!specs) {
    return sku?.external_sku_id || "默认规格";
  }
  if (typeof specs === "string") {
    return specs;
  }
  if (Array.isArray(specs)) {
    return specs.join(" / ");
  }
  return Object.entries(specs)
    .map(([key, value]) => `${key}：${value}`)
    .join(" / ") || sku?.external_sku_id || "默认规格";
}
</script>

<template>
      <section class="content-stack">
        <!-- 批量导入与淘宝登录控制面板 -->
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>铺货选品与淘宝采集</h2>
              <p>运营只需要导入货源、确认采集结果、选择目标小店；后续预检、素材和上架在本页继续推进。</p>
            </div>
            <div class="button-group">
              <el-button type="danger" plain @click="openPublishPricingDialog">
                价格策略
              </el-button>
              <el-button :loading="collectionCheckingLogin" @click="checkTaobaoLoginState">
                检测登录态
              </el-button>
              <el-button :loading="collectionAccessLimitChecking" @click="refreshTaobaoAccessLimitState">
                限制状态
              </el-button>
              <el-button
                v-if="collectionAccessLimitState?.access_limited || collectionAccessLimitState?.captcha_cooling"
                type="danger"
                plain
                :loading="collectionAccessLimitChecking"
                @click="clearTaobaoAccessLimitState"
              >
                清除限制标记
              </el-button>
              <el-button :loading="collectionTesting" @click="openTestCollectDialog">
                测试抓取
              </el-button>
              <el-button type="warning" :loading="collectionLoggingIn" @click="triggerTaobaoLogin">
                淘宝登录(保持状态)
              </el-button>
              <el-button type="primary" @click="collectionImportVisible = true">
                导入铺货表
              </el-button>
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
                  <span>按当前选中采集商品展示，最多 20 条</span>
                </div>
                <el-table
                  v-if="publishPricingPreviewRows.length > 0"
                  :data="publishPricingPreviewRows"
                  class="dense-table compact-table"
                  max-height="260"
                >
                  <el-table-column prop="title" label="商品" min-width="190" show-overflow-tooltip />
                  <el-table-column prop="sku_label" label="SKU" min-width="170" show-overflow-tooltip />
                  <el-table-column label="采集成本" width="110">
                    <template #default="{ row }">{{ row.cost_price_yuan.toFixed(2) }}</template>
                  </el-table-column>
                  <el-table-column label="上架售价" width="120">
                    <template #default="{ row }">
                      <strong>{{ formatCents(row.sale_price_cents) }}</strong>
                    </template>
                  </el-table-column>
                </el-table>
                <div v-else class="empty-hint">
                  选择采集成功的商品后，可在这里预览每个 SKU 的上架售价。
                </div>
              </div>
            </div>
            <template #footer>
              <el-button @click="publishPricingDialogVisible = false">取消</el-button>
              <el-button
                type="primary"
                :loading="publishPricingSaving"
                @click="savePublishPricingStrategyFromForm"
              >
                保存策略
              </el-button>
            </template>
          </el-dialog>

          <el-alert
            v-if="collectionAccessLimitState?.access_limited || collectionAccessLimitState?.captcha_cooling"
            type="warning"
            :closable="false"
            show-icon
            style="margin-top: 12px;"
            :title="collectionAccessLimitState.message || collectionAccessLimitState.captcha_cooling_message || '淘宝账号处于本地保护冷却期，暂不建议采集。'"
          />

          <el-dialog v-model="testCollectVisible" title="测试抓取淘宝商品" width="720px">
            <div class="inline-form">
              <el-input
                v-model="testCollectUrl"
                placeholder="https://item.taobao.com/item.htm?id=..."
                clearable
              />
              <el-button type="primary" :loading="collectionTesting" @click="runTestCollect">
                开始抓取
              </el-button>
            </div>
            <div style="margin-top: 8px;">
              <el-checkbox v-model="testCollectHeaded">
                使用可见浏览器（headed，规避淘宝滑块/反爬，推荐勾选）
              </el-checkbox>
            </div>
            <div v-if="testCollectResult" class="sub-panel" style="margin-top: 12px;">
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
                  <dt>主图数</dt>
                  <dd>{{ (testCollectResult.raw.images || []).length }}</dd>
                </div>
                <div v-if="testCollectResult.raw">
                  <dt>详情图数</dt>
                  <dd>{{ (testCollectResult.raw.detail_images || []).length }}</dd>
                </div>
                <div v-if="testCollectResult.raw">
                  <dt>SKU 数</dt>
                  <dd>{{ (testCollectResult.raw.skus || []).length }}</dd>
                </div>
              </dl>
              <el-input
                type="textarea"
                :rows="14"
                :model-value="JSON.stringify(testCollectResult.raw ?? { stderr: testCollectResult.stderr }, null, 2)"
                readonly
                style="margin-top: 8px; font-family: monospace;"
              />
            </div>
          </el-dialog>
        </div>

        <div class="panel publish-pipeline-panel">
          <div class="panel-title">
            <div>
              <h2>铺货推进</h2>
              <p>商品选好并创建铺货任务后，在这里按流程继续处理；“检查能否铺货”是进入微信素材上传前的本地准备校验。</p>
            </div>
            <el-button
              type="primary"
              :icon="Refresh"
              :loading="automationRunning"
              @click="runOperationalAutomationOnce"
            >
              自动推进一轮
            </el-button>
          </div>
          <div class="publish-pipeline">
            <div class="pipeline-step">
              <strong>1. 准备发品</strong>
              <span>检查店铺、图片、类目、库存和发品参数</span>
              <el-button type="primary" :icon="Refresh" @click="runPublishTasksOnce">
                检查能否铺货
              </el-button>
            </div>
            <div class="pipeline-step">
              <strong>2. 补类目/属性</strong>
              <span>先用本地类目库匹配微信类目，再处理必填属性和类目预检</span>
              <div class="pipeline-actions">
                <el-button :icon="Refresh" @click="runPublishAttributeFillOnce">补齐必填属性</el-button>
                <el-button :icon="Refresh" @click="runPublishAiAttributeSuggestionsOnce">补类目/属性建议</el-button>
                <el-button :icon="Refresh" @click="runPublishCategoryPrechecksOnce">微信类目预检</el-button>
              </div>
            </div>
            <div class="pipeline-step">
              <strong>3. 提交上架</strong>
              <span>上传微信素材、提交发品、同步审核状态并上架</span>
              <div class="pipeline-actions">
                <el-button :icon="UploadFilled" @click="runPublishAssetUploadsOnce">上传素材</el-button>
                <el-button :icon="UploadFilled" @click="runPublishSubmitsOnce">提交微信发品</el-button>
                <el-button :icon="Refresh" @click="runPublishStatusSyncOnce">同步审核状态</el-button>
                <el-button :icon="UploadFilled" @click="runPublishListingOnce">上架通过商品</el-button>
              </div>
            </div>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>查看铺货进度</h2>
            <p>按任务号查看每个商品在各小店的状态、失败原因和待采纳属性。</p>
          </div>
          <div class="inline-form">
            <el-input v-model="queriedTaskId" placeholder="pub_xxx" />
            <el-button type="primary" :icon="Search" @click="queryJob">查询</el-button>
          </div>
        </div>

        <div v-if="currentJob" class="panel">
          <div class="panel-title">
            <h2>{{ currentJob.id }}</h2>
            <el-tag :type="statusType(currentJob.status)">{{ currentJob.status }}</el-tag>
          </div>
          <dl class="status-list compact">
            <div>
              <dt>request_id</dt>
              <dd>{{ currentJob.request_id }}</dd>
            </div>
            <div>
              <dt>商品数</dt>
              <dd>{{ currentJob.accepted_product_count }}</dd>
            </div>
            <div>
              <dt>目标店铺</dt>
              <dd>{{ currentJob.target_shop_count }}</dd>
            </div>
          </dl>

          <el-collapse>
            <el-collapse-item
              v-for="product in currentJob.products"
              :key="product.external_product_id"
              :title="`${product.title} / 失败 ${product.failed_count} / 待处理 ${product.pending_count}`"
            >
              <el-table :data="product.items" class="dense-table">
                <el-table-column prop="shop_name" label="店铺" min-width="160" />
                <el-table-column prop="status" label="状态" width="120">
                  <template #default="{ row }">
                    <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
                  </template>
                </el-table-column>
                <el-table-column prop="error_code" label="错误码" min-width="180" />
                <el-table-column prop="error_summary" label="失败原因" min-width="260" />
                <el-table-column label="属性建议" width="120">
                  <template #default="{ row }">
                    <el-tag
                      v-if="attributeSuggestionsForItem(row).length > 0"
                      :type="pendingAttributeSuggestionCountForItem(row) > 0 ? 'warning' : 'success'"
                    >
                      {{ pendingAttributeSuggestionCountForItem(row) > 0 ? `待采纳 ${pendingAttributeSuggestionCountForItem(row)}` : "已处理" }}
                    </el-tag>
                    <span v-else>-</span>
                  </template>
                </el-table-column>
              </el-table>
              <div v-if="attributeSuggestionsForProduct(product).length > 0" class="sub-panel job-suggestion-panel">
                <div class="automation-head">
                  <div>
                    <h3>属性建议</h3>
                    <p>当前商品的低置信属性建议可在这里直接采纳；采纳后仍需重新执行微信类目预检。</p>
                  </div>
                  <div class="button-group">
                    <el-button :icon="Refresh" @click="refreshJobAttributeSuggestions">刷新建议</el-button>
                    <el-button
                      type="primary"
                      :disabled="pendingAttributeSuggestionsForProduct(product).length === 0 || attributeSuggestionApplying"
                      :loading="attributeSuggestionApplying"
                      @click="applyProductAttributeSuggestions(product)"
                    >
                      采纳当前商品建议
                    </el-button>
                  </div>
                </div>
                <el-table :data="attributeSuggestionsForProduct(product)" class="dense-table compact-table">
                  <el-table-column label="状态" width="95">
                    <template #default="{ row }">
                      <el-tag :type="row.applied ? 'success' : 'warning'">
                        {{ row.applied ? "已采纳" : "待确认" }}
                      </el-tag>
                    </template>
                  </el-table-column>
                  <el-table-column label="店铺/属性" min-width="180">
                    <template #default="{ row }">
                      <span>{{ row.shop_name }} · {{ row.attr_key }}</span>
                      <small class="subtext">{{ attributeKindLabel(row.attr_kind) }}</small>
                    </template>
                  </el-table-column>
                  <el-table-column label="建议值" min-width="210" show-overflow-tooltip>
                    <template #default="{ row }">
                      {{ attributeSuggestionValue(row) }}
                    </template>
                  </el-table-column>
                  <el-table-column label="允许值" min-width="170" show-overflow-tooltip>
                    <template #default="{ row }">
                      {{ allowedValuesText(row.allowed_values) }}
                    </template>
                  </el-table-column>
                  <el-table-column label="来源/置信" min-width="160">
                    <template #default="{ row }">
                      <span>{{ row.source }}</span>
                      <small class="subtext">confidence {{ row.confidence }}</small>
                    </template>
                  </el-table-column>
                  <el-table-column label="操作" width="95">
                    <template #default="{ row }">
                      <el-button
                        size="small"
                        :disabled="row.applied || attributeSuggestionApplying"
                        @click="applySingleAttributeSuggestion(row)"
                      >
                        采纳
                      </el-button>
                    </template>
                  </el-table-column>
                </el-table>
              </div>
            </el-collapse-item>
          </el-collapse>
        </div>

        <!-- 采集结果池 -->
        <div class="panel collection-panel" style="margin-top: 20px;">
          <div class="panel-title">
            <div>
              <h2>选品结果池</h2>
              <p>采集成功后先进入结果池，选中商品和目标小店后创建铺货任务。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshCollectionTasks">刷新队列</el-button>
              <el-button
                type="primary"
                plain
                :icon="Refresh"
                :loading="collectionReviewing"
                @click="reviewSelectedCollectionTasks"
              >
                审查选中商品
              </el-button>
              <el-button type="primary" :icon="Refresh" @click="resumeCollectionTasks">继续采集</el-button>
              <el-button type="danger" text @click="clearCollectionHistory">清空列表</el-button>
            </div>
          </div>

          <div class="sub-panel collection-publish-panel">
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
                :loading="collectionPublishing"
                :disabled="!selectedCollectionPublishable || collectionPublishTargetShopIds.length === 0"
                @click="createPublishJobFromSelectedCollections"
              >
                创建铺货任务
              </el-button>
            </div>
            <span class="subtext">
              已选 {{ selectedCollectionTasks.length }} 个商品，{{ collectionPublishTargetShopIds.length }} 个目标小店；只有审查通过的商品才能创建铺货任务。
            </span>
            <span class="subtext">
              价格策略：{{ publishPricingSummary }}
            </span>
          </div>
          
          <el-table
            :data="paginatedCollectionTasks"
            class="dense-table"
            max-height="420"
          >
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
                  :disabled="!canSelectCollectionTask(row)"
                  @change="onCollectionTaskSelectionChange(row, $event)"
                />
              </template>
            </el-table-column>
            <el-table-column prop="title" label="商品名称" min-width="180" show-overflow-tooltip />
            <el-table-column prop="source_url" label="淘宝链接" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                <a :href="row.source_url" target="_blank" class="link" style="color: #c38a21; text-decoration: underline;">{{ row.source_url }}</a>
              </template>
            </el-table-column>
            <el-table-column prop="category_path" label="微信类目" min-width="150" show-overflow-tooltip />
            <el-table-column label="采集结果" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                <el-button
                  v-if="row.collected_data"
                  type="primary"
                  text
                  class="collection-result-link"
                  @click="openCollectionDetail(row)"
                >
                  {{ collectionProductSummary(row) }}
                </el-button>
                <span v-else>{{ collectionProductSummary(row) }}</span>
              </template>
            </el-table-column>
            <el-table-column prop="status" label="采集状态" width="120">
              <template #default="{ row }">
                <el-tag :type="row.status === 'success' ? 'success' : row.status === 'failed' ? 'danger' : row.status === 'running' ? 'warning' : 'info'">
                  {{ collectionTaskStatusLabel(row.status) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="review_status" label="审查状态" width="132">
              <template #default="{ row }">
                <el-tag :type="collectionReviewStatusType(row.review_status)">
                  {{ collectionReviewStatusLabel(row.review_status) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="已创建小店" min-width="160" show-overflow-tooltip>
              <template #default="{ row }">
                {{ collectionPublishedShopNames(row) }}
              </template>
            </el-table-column>
            <el-table-column label="铺货任务" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">
                {{ collectionPublishJobText(row) }}
              </template>
            </el-table-column>
            <el-table-column prop="error_summary" label="进度/失败原因" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                <span :style="{ color: row.status === 'failed' ? '#bd4c2f' : '#6c685e' }">
                  {{ row.review_summary || row.error_summary || '-' }}
                </span>
              </template>
            </el-table-column>
            <el-table-column label="创建时间" width="180">
              <template #default="{ row }">{{ formatDateTime(row.created_at) }}</template>
            </el-table-column>
            <el-table-column label="操作" width="132" fixed="right">
              <template #default="{ row }">
                <div class="row-actions">
                  <el-button
                    v-if="row.collected_data"
                    size="small"
                    type="primary"
                    text
                    @click="openCollectionDetail(row)"
                  >
                    查看
                  </el-button>
                  <el-button
                    v-if="row.status === 'success' && row.review_status !== 'passed'"
                    size="small"
                    type="primary"
                    text
                    :loading="collectionReviewing"
                    @click="reviewSingleCollectionTask(row)"
                  >
                    审查
                  </el-button>
                  <el-button
                    v-if="row.review_status === 'passed'"
                    size="small"
                    type="warning"
                    text
                    @click="resetSelectedCollectionReview(row)"
                  >
                    重置
                  </el-button>
                  <el-button 
                    v-if="row.status === 'failed'" 
                    size="small" 
                    type="primary" 
                    text 
                    @click="retryCollection(row.id)"
                  >
                    重试
                  </el-button>
                  <span v-if="!row.collected_data && row.status !== 'failed'">-</span>
                </div>
              </template>
            </el-table-column>
          </el-table>

          <div class="collection-pagination">
            <span>
              共 {{ collectionTaskTotal }} 条，已选 {{ selectedCollectionTasks.length }} 条
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
          v-model="collectionDetailVisible"
          class="collection-detail-dialog"
          width="860px"
          :title="selectedCollectionDetailTask ? `采集数据：${selectedCollectionDetailTask.title}` : '采集数据'"
        >
          <div v-if="selectedCollectionDetailTask" class="collection-detail-layout">
            <dl class="status-list compact collection-detail-summary">
              <div>
                <dt>商品标题</dt>
                <dd>{{ selectedCollectionDetailProduct?.title || selectedCollectionDetailTask.title }}</dd>
              </div>
              <div>
                <dt>淘宝链接</dt>
                <dd>
                  <a
                    :href="selectedCollectionDetailProduct?.source_url || selectedCollectionDetailTask.source_url"
                    target="_blank"
                    class="inline-link"
                  >
                    {{ selectedCollectionDetailProduct?.source_url || selectedCollectionDetailTask.source_url }}
                  </a>
                </dd>
              </div>
              <div>
                <dt>供应商</dt>
                <dd>{{ selectedCollectionDetailProduct?.supplier_name || '-' }}</dd>
              </div>
              <div>
                <dt>微信类目提示</dt>
                <dd>{{ selectedCollectionDetailProduct?.category_hint || selectedCollectionDetailTask.category_path || '-' }}</dd>
              </div>
              <div>
                <dt>采集结果</dt>
                <dd>{{ collectionProductSummary(selectedCollectionDetailTask) }}</dd>
              </div>
              <div>
                <dt>采集时间</dt>
                <dd>{{ formatDateTime(selectedCollectionDetailTask.updated_at) }}</dd>
              </div>
            </dl>

            <div class="collection-detail-block review-panel">
              <div class="review-panel-header">
                <div>
                  <h3>采集审查</h3>
                  <p>{{ selectedCollectionDetailTask.review_summary || '尚未审查，先运行审查后再创建铺货任务。' }}</p>
                </div>
                <el-tag :type="collectionReviewStatusType(selectedCollectionDetailTask.review_status)">
                  {{ collectionReviewStatusLabel(selectedCollectionDetailTask.review_status) }}
                </el-tag>
              </div>
              <div class="review-actions">
                <el-button
                  type="primary"
                  :icon="Refresh"
                  :loading="collectionReviewing"
                  @click="reviewSingleCollectionTask(selectedCollectionDetailTask)"
                >
                  重新审查
                </el-button>
                <el-button
                  v-if="selectedCollectionDetailTask.review_status === 'needs_review'"
                  type="success"
                  :loading="collectionReviewConfirming"
                  @click="confirmSelectedCollectionReview"
                >
                  确认审查通过
                </el-button>
                <el-button
                  v-if="selectedCollectionDetailTask.review_status !== 'pending'"
                  text
                  @click="resetSelectedCollectionReview(selectedCollectionDetailTask)"
                >
                  重置审查
                </el-button>
              </div>
              <div v-if="selectedCollectionReviewIssues.length > 0" class="review-issue-list">
                <div
                  v-for="(issue, index) in selectedCollectionReviewIssues"
                  :key="`${issue.kind || 'issue'}-${index}`"
                  class="review-issue"
                  :class="`is-${issue.severity || 'confirm'}`"
                >
                  <strong>{{ issue.severity === 'block' ? '拦截' : '确认' }}</strong>
                  <span>{{ issue.message }}</span>
                </div>
              </div>
              <div v-if="selectedCollectionCategoryCandidates.length > 0" class="review-category-list">
                <h4>候选微信类目</h4>
                <el-radio-group v-model="selectedReviewCategoryKey">
                  <el-radio
                    v-for="candidate in selectedCollectionCategoryCandidates"
                    :key="candidate.category_ids.join('/')"
                    :value="candidate.category_ids.join('/')"
                    border
                  >
                    {{ candidate.category_path }} · {{ candidate.score }}
                  </el-radio>
                </el-radio-group>
              </div>
              <div v-if="selectedCollectionRemovedImages.length > 0" class="review-removed-list">
                <h4>已剔除图片</h4>
                <div class="review-removed-grid">
                  <div
                    v-for="item in selectedCollectionRemovedImages"
                    :key="item.url"
                    class="review-removed-item"
                  >
                    <img :src="item.url" alt="已剔除图片" loading="lazy" />
                    <span>{{ item.reason }}</span>
                  </div>
                </div>
              </div>
            </div>

            <div v-if="selectedCollectionMainImages.length > 0" class="collection-detail-block">
              <h3>主图预览</h3>
              <div class="collection-image-strip">
                <img
                  v-for="image in selectedCollectionMainImages"
                  :key="image"
                  :src="image"
                  alt="采集主图"
                  class="collection-image-thumb"
                  loading="lazy"
                />
              </div>
            </div>

            <div v-if="selectedCollectionDetailImages.length > 0" class="collection-detail-block">
              <h3>详情图预览</h3>
              <div class="collection-image-strip">
                <img
                  v-for="image in selectedCollectionDetailImages"
                  :key="image"
                  :src="image"
                  alt="采集详情图"
                  class="collection-image-thumb"
                  loading="lazy"
                />
              </div>
            </div>

            <div v-if="selectedCollectionSkuPreview.length > 0" class="collection-detail-block">
              <h3>SKU 预览</h3>
              <div class="sku-preview-list">
                <div
                  v-for="sku in selectedCollectionSkuPreview"
                  :key="sku.external_sku_id"
                  class="sku-preview-item"
                >
                  <span>{{ formatCollectionSkuSpecs(sku) }}</span>
                  <small>成本 {{ sku.cost_price ?? '-' }}</small>
                  <small>库存 {{ sku.stock ?? '-' }}</small>
                </div>
              </div>
            </div>

            <div class="collection-detail-block">
              <h3>原始采集 JSON</h3>
              <el-input
                type="textarea"
                :rows="14"
                :model-value="selectedCollectionDetailJson"
                readonly
                class="json-editor collection-detail-json"
              />
            </div>
          </div>
        </el-dialog>
      </section>
</template>
