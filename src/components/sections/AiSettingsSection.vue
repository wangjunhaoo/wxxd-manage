<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  aiProviderApiKeyPlaceholder,
  aiProviderApiOptions,
  aiProviderBaseUrlPlaceholder,
  aiProviderCanTest,
  aiProviderForm,
  aiProviderFormIsCustom,
  aiProviderFormRequiresApiKey,
  aiProviderFormRequiresBaseUrl,
  aiProviderFormRuntimeLabel,
  aiProviderModelPlaceholder,
  aiProviderOptions,
  aiProviderRuntimeLabel,
  aiProviderSaving,
  aiProviderSettings,
  aiProviderTesting,
  formatDateTime,
  Refresh,
  saveAiProviderSettings,
  testAiProvider,
} = props.ctx;
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>AI Agent</h2>
          <p>
            用于采集结果审查和铺货自动补齐；铺货过程会自动调用，不需要单独人工执行。
          </p>
        </div>
        <el-tag :type="aiProviderSettings.enabled ? 'success' : 'info'">
          {{ aiProviderSettings.enabled ? aiProviderRuntimeLabel : "已关闭" }}
        </el-tag>
      </div>

      <dl class="status-list compact">
        <div>
          <dt>Provider</dt>
          <dd>
            {{
              aiProviderSettings.provider_type === "custom"
                ? aiProviderSettings.custom_provider_id
                : aiProviderSettings.provider_type
            }}
          </dd>
        </div>
        <div>
          <dt>模型</dt>
          <dd>{{ aiProviderSettings.model || "未配置" }}</dd>
        </div>
        <div>
          <dt>密钥</dt>
          <dd>
            {{
              aiProviderSettings.has_api_key
                ? aiProviderSettings.api_key_hint || "已保存"
                : "未保存"
            }}
          </dd>
        </div>
        <div>
          <dt>更新时间</dt>
          <dd>
            {{
              aiProviderSettings.updated_at
                ? formatDateTime(aiProviderSettings.updated_at)
                : "尚未保存"
            }}
          </dd>
        </div>
      </dl>

      <div class="ai-settings-form">
        <el-switch
          v-model="aiProviderForm.enabled"
          active-text="启用 AI Agent"
          inactive-text="关闭"
        />
        <el-select
          v-model="aiProviderForm.provider_type"
          class="ai-provider-select"
          filterable
        >
          <el-option
            v-for="option in aiProviderOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-input :model-value="aiProviderFormRuntimeLabel" disabled />
        <el-input
          v-if="aiProviderFormIsCustom"
          v-model="aiProviderForm.custom_provider_id"
          placeholder="Custom provider id，例如 wx-xd-custom"
        />
        <el-select
          v-if="aiProviderFormIsCustom"
          v-model="aiProviderForm.api"
          class="ai-provider-select"
        >
          <el-option
            v-for="option in aiProviderApiOptions"
            :key="option.value"
            :label="option.label"
            :value="option.value"
          />
        </el-select>
        <el-input
          v-model="aiProviderForm.base_url"
          :placeholder="aiProviderBaseUrlPlaceholder"
        />
        <el-input
          v-model="aiProviderForm.model"
          :placeholder="aiProviderModelPlaceholder"
        />
        <el-input
          v-model="aiProviderForm.temperature"
          placeholder="temperature，0 到 1"
        />
        <el-input
          v-if="aiProviderFormIsCustom"
          v-model="aiProviderForm.context_window"
          placeholder="context_window，例如 128000"
        />
        <el-input
          v-if="aiProviderFormIsCustom"
          v-model="aiProviderForm.max_tokens"
          placeholder="max_tokens，例如 4096"
        />
        <el-input
          v-model="aiProviderForm.api_key"
          type="password"
          show-password
          autocomplete="new-password"
          :placeholder="aiProviderApiKeyPlaceholder"
        />
      </div>
      <el-alert
        class="inline-alert"
        type="success"
        :closable="false"
        show-icon
        title="运行时固定为 pi-coding-agent，内置文件、命令和编辑工具已关闭；内置 provider 使用 Pi 模型定义，Custom 按所选 API 类型注册。"
      />

      <div class="action-row ai-actions">
        <el-checkbox
          v-if="aiProviderSettings.has_api_key"
          v-model="aiProviderForm.clear_api_key"
        >
          清除已保存 API Key
        </el-checkbox>
        <span v-else></span>
        <div class="button-group">
          <el-button
            :icon="Refresh"
            :loading="aiProviderTesting"
            :disabled="!aiProviderCanTest"
            @click="testAiProvider"
          >
            试连
          </el-button>
          <el-button
            type="primary"
            :loading="aiProviderSaving"
            @click="saveAiProviderSettings"
          >
            保存
          </el-button>
        </div>
      </div>
      <dl class="status-list compact provider-hints">
        <div>
          <dt>当前运行时</dt>
          <dd>{{ aiProviderFormRuntimeLabel }}</dd>
        </div>
        <div>
          <dt>Custom API</dt>
          <dd>
            {{
              aiProviderFormIsCustom
                ? aiProviderForm.api
                : "使用 Pi 内置模型定义"
            }}
          </dd>
        </div>
        <div>
          <dt>Base URL</dt>
          <dd>{{ aiProviderFormRequiresBaseUrl ? "必填" : "可选" }}</dd>
        </div>
        <div>
          <dt>API Key</dt>
          <dd>{{ aiProviderFormRequiresApiKey ? "必填" : "可选" }}</dd>
        </div>
      </dl>
    </div>
  </section>
</template>
