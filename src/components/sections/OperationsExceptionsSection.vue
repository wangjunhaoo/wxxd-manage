<script setup lang="ts">
import { ref } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import NotificationsSection from "./NotificationsSection.vue";
import AftersalesSection from "./AftersalesSection.vue";

const props = defineProps<{ ctx: WxXdAppContext }>();
const activeTab = ref("notifications");
</script>

<template>
  <section class="content-stack">
    <div class="ops-page-head">
      <div>
        <p class="eyebrow">异常处理</p>
        <h2>把铺货、采购、履约和售后异常集中处理</h2>
        <p>运营只需要从提醒进入处理页；系统内部任务和接口日志放到系统设置里。</p>
      </div>
      <el-button :icon="props.ctx.Refresh" @click="props.ctx.refreshAll">刷新</el-button>
    </div>

    <el-tabs v-model="activeTab" class="section-tabs">
      <el-tab-pane label="通知待办" name="notifications">
        <NotificationsSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="售后与纠纷" name="aftersales">
        <AftersalesSection :ctx="props.ctx" />
      </el-tab-pane>
    </el-tabs>
  </section>
</template>
