<script setup lang="ts">
import { computed } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  CircleClose,
  closeCurrentPublishJob,
  currentJob,
  formatDateTime,
  openTask,
  publishRetryRunning,
  queriedTaskId,
  queryJob,
  Refresh,
  resumePublishFailuresOnce,
  Search,
  selectedSection,
  taskRuns,
} = props.ctx;

type BusinessStatus = "pending" | "running" | "listed" | "exception";

const statusLabelMap: Record<BusinessStatus, string> = {
  pending: "待铺货",
  running: "铺货中",
  listed: "已上架",
  exception: "异常",
};

const statusToneMap: Record<
  BusinessStatus,
  "info" | "primary" | "success" | "danger"
> = {
  pending: "info",
  running: "primary",
  listed: "success",
  exception: "danger",
};

const publishTaskRuns = computed(() =>
  taskRuns.value.filter(
    (task) => task.task_type === "publish.create_external_job",
  ),
);

const currentJobItems = computed(
  () =>
    currentJob.value?.products.flatMap((product) =>
      product.items.map((item) => ({
        ...item,
        product_title: product.title,
        external_product_id: product.external_product_id,
      })),
    ) ?? [],
);

const currentJobSummary = computed(() => {
  const items = currentJobItems.value;
  const summary = {
    total: items.length,
    pending: 0,
    running: 0,
    listed: 0,
    exception: 0,
    percentage: 0,
  };
  for (const item of items) {
    summary[toBusinessStatus(item.status)] += 1;
  }
  summary.percentage =
    summary.total > 0
      ? Math.round(((summary.listed + summary.exception) / summary.total) * 100)
      : 0;
  return summary;
});

const exceptionGroups = computed(() => {
  const groups = new Map<string, { text: string; count: number }>();
  for (const item of currentJobItems.value) {
    if (toBusinessStatus(item.status) !== "exception") {
      continue;
    }
    const text = publishExceptionText(item.error_code, item.error_summary);
    const group = groups.get(text);
    if (group) {
      group.count += 1;
    } else {
      groups.set(text, { text, count: 1 });
    }
  }
  return Array.from(groups.values()).sort((a, b) => b.count - a.count);
});

function toBusinessStatus(status: string): BusinessStatus {
  if (status === "success" || status === "listed") {
    return "listed";
  }
  if (status === "failed" || status === "exception") {
    return "exception";
  }
  if (status === "pending" || status === "queued") {
    return "pending";
  }
  return "running";
}

function businessStatusLabel(status: string) {
  return statusLabelMap[toBusinessStatus(status)];
}

function businessStatusTone(status: string) {
  return statusToneMap[toBusinessStatus(status)];
}

function publishExceptionText(code: string | null, summary: string | null) {
  const value = code || "";
  if (
    value.includes("CATEGORY") ||
    value.includes("ATTR") ||
    value.includes("PAYLOAD")
  ) {
    return "微信类目或必填属性无法自动确定";
  }
  if (value.includes("FREIGHT_TEMPLATE")) {
    return "店铺缺默认运费模板";
  }
  if (value.includes("AFTER_SALE") || value.includes("ADDRESS")) {
    return "店铺缺默认售后地址";
  }
  if (value.includes("IMAGE") || value.includes("ASSET")) {
    return "商品图片不符合微信要求";
  }
  if (value.includes("SHOP_SECRET")) {
    return "店铺密钥未配置";
  }
  if (value.includes("SHOP_NOT_ACTIVE")) {
    return "店铺未启用";
  }
  if (value.includes("WECHAT")) {
    return summary || "微信接口返回异常";
  }
  return summary || "系统无法自动完成铺货";
}
</script>

<template>
  <section class="content-stack publish-task-page">
    <div class="panel publish-task-command-panel">
      <div class="panel-title">
        <div>
          <h2>铺货结果</h2>
          <p>系统自动完成微信发布链路；这里只看业务结果和异常原因。</p>
        </div>
        <div class="button-group">
          <el-button :icon="Refresh" @click="queryJob">刷新</el-button>
          <el-button
            type="primary"
            :loading="publishRetryRunning"
            @click="resumePublishFailuresOnce"
          >
            推进铺货
          </el-button>
          <el-button text @click="selectedSection = 'publish'"
            >回到采集铺货</el-button
          >
        </div>
      </div>
    </div>

    <div class="publish-task-workbench">
      <aside class="panel publish-task-board publish-task-board-page">
        <div class="panel-title">
          <div>
            <h2>任务</h2>
            <p>点击任务查看商品和店铺铺货结果。</p>
          </div>
        </div>

        <div
          v-if="publishTaskRuns.length > 0"
          class="publish-task-list publish-task-list-page"
        >
          <button
            v-for="task in publishTaskRuns"
            :key="task.id"
            class="publish-task-row"
            type="button"
            :class="{ active: currentJob?.id === task.id }"
            @click="openTask(task)"
          >
            <span class="publish-task-row-main">
              <strong>{{ task.id }}</strong>
              <small>{{ formatDateTime(task.created_at) }}</small>
            </span>
            <el-tag :type="businessStatusTone(task.status)">{{
              businessStatusLabel(task.status)
            }}</el-tag>
            <el-progress :percentage="task.progress" :stroke-width="7" />
            <span class="publish-task-row-counts">
              待 {{ task.pending_count }} · 异常 {{ task.failed_count }}
            </span>
          </button>
        </div>
        <div v-else class="empty-hint publish-task-empty">暂无铺货任务</div>

        <div class="inline-form publish-task-search">
          <el-input
            v-model="queriedTaskId"
            placeholder="输入任务 ID 精确查询"
          />
          <el-button :icon="Search" @click="queryJob">查询</el-button>
        </div>
      </aside>

      <div class="publish-task-detail">
        <div
          v-if="currentJob"
          class="panel publish-current-job publish-current-job-page"
        >
          <div class="panel-title">
            <div>
              <h2>{{ currentJob.id }}</h2>
              <p>{{ currentJob.request_id }}</p>
            </div>
            <div class="button-group">
              <el-tag :type="businessStatusTone(currentJob.status)">
                {{ businessStatusLabel(currentJob.status) }}
              </el-tag>
              <el-button :icon="Refresh" @click="queryJob">刷新</el-button>
              <el-button :icon="CircleClose" @click="closeCurrentPublishJob"
                >收起</el-button
              >
              <el-button
                type="primary"
                :loading="publishRetryRunning"
                @click="resumePublishFailuresOnce"
              >
                推进铺货
              </el-button>
            </div>
          </div>

          <div class="publish-progress-summary">
            <div class="publish-progress-main">
              <strong>{{ currentJobSummary.percentage }}%</strong>
              <el-progress
                :percentage="currentJobSummary.percentage"
                :stroke-width="10"
              />
            </div>
            <div class="publish-progress-metrics">
              <span>总项 {{ currentJobSummary.total }}</span>
              <span>待铺货 {{ currentJobSummary.pending }}</span>
              <span>铺货中 {{ currentJobSummary.running }}</span>
              <span>已上架 {{ currentJobSummary.listed }}</span>
              <span>异常 {{ currentJobSummary.exception }}</span>
            </div>
          </div>

          <div
            v-if="exceptionGroups.length > 0"
            class="sub-panel failure-groups"
          >
            <div class="automation-head">
              <div>
                <h3>异常</h3>
                <p>这些是系统自动处理不了的问题；处理完后点击推进铺货。</p>
              </div>
              <el-button
                type="primary"
                :loading="publishRetryRunning"
                @click="resumePublishFailuresOnce"
              >
                重新铺货
              </el-button>
            </div>
            <div class="failure-group-list">
              <div
                v-for="group in exceptionGroups"
                :key="group.text"
                class="failure-group-item"
              >
                <el-tag type="danger">{{ group.count }}</el-tag>
                <span>{{ group.text }}</span>
              </div>
            </div>
          </div>

          <el-table :data="currentJobItems" class="dense-table">
            <el-table-column
              prop="product_title"
              label="商品"
              min-width="220"
              show-overflow-tooltip
            />
            <el-table-column prop="shop_name" label="店铺" min-width="150" />
            <el-table-column label="状态" width="110">
              <template #default="{ row }">
                <el-tag :type="businessStatusTone(row.status)">
                  {{ businessStatusLabel(row.status) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column
              label="异常原因"
              min-width="280"
              show-overflow-tooltip
            >
              <template #default="{ row }">
                <span v-if="toBusinessStatus(row.status) === 'exception'">
                  {{ publishExceptionText(row.error_code, row.error_summary) }}
                </span>
                <span v-else>-</span>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="120">
              <template #default="{ row }">
                <el-button
                  v-if="toBusinessStatus(row.status) === 'exception'"
                  size="small"
                  :loading="publishRetryRunning"
                  @click="resumePublishFailuresOnce"
                >
                  重新铺货
                </el-button>
                <span v-else>-</span>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <div v-else class="panel publish-task-empty-detail">
          <h2>选择一个铺货任务</h2>
          <p>任务打开后，只显示待铺货、铺货中、已上架和异常。</p>
          <el-button type="primary" plain @click="selectedSection = 'publish'">
            去创建铺货
          </el-button>
        </div>
      </div>
    </div>
  </section>
</template>
