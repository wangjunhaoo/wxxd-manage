<script setup lang="ts">
import { computed } from "vue";
import { useWxXdApp } from "./composables/useWxXdApp";
import CollectionImportDialog from "./components/CollectionImportDialog.vue";
import OperationsDashboardSection from "./components/sections/OperationsDashboardSection.vue";
import OperationsExceptionsSection from "./components/sections/OperationsExceptionsSection.vue";
import OperationsAnalyticsSection from "./components/sections/OperationsAnalyticsSection.vue";
import SystemSettingsSection from "./components/sections/SystemSettingsSection.vue";
import PriceUpdateSection from "./components/sections/PriceUpdateSection.vue";
import ProcurementSection from "./components/sections/ProcurementSection.vue";
import PublishJobsSection from "./components/sections/PublishJobsSection.vue";
import AgentSkillsSection from "./components/sections/AgentSkillsSection.vue";
import ProductManagementSection from "./components/sections/ProductManagementSection.vue";
import OrderManagementSection from "./components/sections/OrderManagementSection.vue";
import "./styles/app.css";

const ctx = useWxXdApp();
const { dashboard, loading, Refresh, refreshAll, runtimeLabel, selectedSection } = ctx;

const navItems = [
  { key: "workbench", label: "今日工作台", desc: "先看待办" },
  { key: "products", label: "商品管理", desc: "商品与店铺" },
  { key: "orders", label: "订单管理", desc: "订单与履约" },
  { key: "publish", label: "铺货", desc: "采集到上架" },
  { key: "procurement", label: "采购下单", desc: "采购与物流" },
  { key: "price", label: "价格调整", desc: "订单改价" },
  { key: "exceptions", label: "异常处理", desc: "通知与售后" },
  { key: "analytics", label: "经营分析", desc: "动销库存利润" },
  { key: "skills", label: "Agent 技能", desc: "技能管理" },
  { key: "settings", label: "系统设置", desc: "低频配置" },
];

const sectionMeta = computed(() => {
  const metas: Record<string, { eyebrow: string; title: string }> = {
    workbench: { eyebrow: "Asia/Shanghai", title: "运营今日工作台" },
    products: { eyebrow: "商品管理", title: "货源、铺货和动销状态" },
    orders: { eyebrow: "订单管理", title: "订单、采购、发货和售后状态" },
    publish: { eyebrow: "铺货主流程", title: "从采集选品到微信上架" },
    procurement: { eyebrow: "采购下单", title: "待采购订单与供应商物流" },
    price: { eyebrow: "价格调整", title: "未付款订单批量改价" },
    exceptions: { eyebrow: "异常处理", title: "通知、售后与纠纷" },
    analytics: { eyebrow: "经营分析", title: "动销、库存和利润" },
    skills: { eyebrow: "Agent 技能", title: "AI 技能管理与试跑" },
    settings: { eyebrow: "系统设置", title: "配置、集成与任务日志" },
  };
  return metas[selectedSection.value] ?? metas.workbench;
});
</script>

<template>
  <div class="app-shell">
    <aside class="sidebar">
      <div class="brand">
        <div class="brand-mark">XD</div>
        <div>
          <strong>微信小店铺货中台</strong>
          <span>{{ runtimeLabel }}</span>
        </div>
      </div>

      <nav class="nav">
        <button
          v-for="item in navItems"
          :key="item.key"
          :class="{ active: selectedSection === item.key }"
          @click="selectedSection = item.key"
        >
          <span>{{ item.label }}</span>
          <small>{{ item.desc }}</small>
        </button>
      </nav>

      <div class="runtime-card">
        <span>本地数据库</span>
        <code>{{ dashboard?.database_path || "初始化中" }}</code>
      </div>
    </aside>

    <main class="workspace" v-loading="loading">
      <header class="topbar">
        <div>
          <p class="eyebrow">{{ sectionMeta.eyebrow }}</p>
          <h1>{{ sectionMeta.title }}</h1>
        </div>
        <el-button :icon="Refresh" @click="refreshAll">刷新</el-button>
      </header>

      <OperationsDashboardSection v-if="selectedSection === 'workbench'" :ctx="ctx" />
      <ProductManagementSection v-if="selectedSection === 'products'" :ctx="ctx" />
      <OrderManagementSection v-if="selectedSection === 'orders'" :ctx="ctx" />
      <PublishJobsSection v-if="selectedSection === 'publish'" :ctx="ctx" />
      <ProcurementSection v-if="selectedSection === 'procurement'" :ctx="ctx" />
      <PriceUpdateSection v-if="selectedSection === 'price'" :ctx="ctx" />
      <OperationsExceptionsSection v-if="selectedSection === 'exceptions'" :ctx="ctx" />
      <OperationsAnalyticsSection v-if="selectedSection === 'analytics'" :ctx="ctx" />
      <AgentSkillsSection v-if="selectedSection === 'skills'" :ctx="ctx" />
      <SystemSettingsSection v-if="selectedSection === 'settings'" :ctx="ctx" />
    </main>

    <CollectionImportDialog :ctx="ctx" />
  </div>
</template>
