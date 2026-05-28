<script setup lang="ts">
import { ref } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import SalesSection from "./SalesSection.vue";
import InventorySection from "./InventorySection.vue";
import ProfitSection from "./ProfitSection.vue";

const props = defineProps<{ ctx: WxXdAppContext }>();
const activeTab = ref("sales");
</script>

<template>
  <section class="content-stack">
    <div class="ops-page-head">
      <div>
        <p class="eyebrow">经营分析</p>
        <h2>用真实订单、成本、库存和售后判断下一步动作</h2>
        <p>这里只做合规经营判断：继续铺货、调价、补货、观察或下架。</p>
      </div>
      <el-button :icon="props.ctx.Refresh" @click="props.ctx.refreshAll">刷新</el-button>
    </div>

    <el-tabs v-model="activeTab" class="section-tabs">
      <el-tab-pane label="动销分析" name="sales">
        <SalesSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="库存风控" name="inventory">
        <InventorySection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="利润核算" name="profit">
        <ProfitSection :ctx="props.ctx" />
      </el-tab-pane>
    </el-tabs>
  </section>
</template>
