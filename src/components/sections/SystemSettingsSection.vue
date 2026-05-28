<script setup lang="ts">
import { ref } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import ShopsSection from "./ShopsSection.vue";
import CatalogSection from "./CatalogSection.vue";
import AiSettingsSection from "./AiSettingsSection.vue";
import PublishApiSection from "./PublishApiSection.vue";
import BackupSection from "./BackupSection.vue";
import TasksSection from "./TasksSection.vue";

const props = defineProps<{ ctx: WxXdAppContext }>();
const activeTab = ref("shops");
</script>

<template>
  <section class="content-stack">
    <div class="ops-page-head">
      <div>
        <p class="eyebrow">系统设置</p>
        <h2>低频配置、技术集成和任务日志集中放在这里</h2>
        <p>日常运营不用理解 API、runner 和数据库路径；排障时再进这些页。</p>
      </div>
      <el-button :icon="props.ctx.Refresh" @click="props.ctx.refreshAll">刷新</el-button>
    </div>

    <el-tabs v-model="activeTab" class="section-tabs">
      <el-tab-pane label="店铺与密钥" name="shops">
        <ShopsSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="类目规则" name="catalog">
        <CatalogSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="AI 设置" name="ai">
        <AiSettingsSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="本地 API" name="api">
        <PublishApiSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="任务日志" name="tasks">
        <TasksSection :ctx="props.ctx" />
      </el-tab-pane>
      <el-tab-pane label="数据备份" name="backup">
        <BackupSection :ctx="props.ctx" />
      </el-tab-pane>
    </el-tabs>
  </section>
</template>
