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
  allowedValuesText,
  appliedAttributeSuggestionCount,
  applySelectedAttributeSuggestions,
  applySingleAttributeSuggestion,
  attributeKindLabel,
  attributeSuggestionApplying,
  attributeSuggestions,
  attributeSuggestionStatusFilter,
  attributeSuggestionTotal,
  attributeSuggestionValue,
  formatDateTime,
  handleAttributeSuggestionSelection,
  pendingAttributeSuggestionCount,
  Refresh,
  refreshAttributeSuggestions,
  runPublishAiAttributeSuggestionsOnce,
  saveAiProviderSettings,
  selectedAttributeSuggestions,
  testAiProvider,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>AI Agent</h2>
              <p>用于采集结果审查和微信类目属性建议；统一通过 pi-coding-agent 调用已选择的 Pi provider。</p>
            </div>
            <el-tag :type="aiProviderSettings.enabled ? 'success' : 'info'">
              {{ aiProviderSettings.enabled ? aiProviderRuntimeLabel : "已关闭" }}
            </el-tag>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>Provider</dt>
              <dd>{{ aiProviderSettings.provider_type === "custom" ? aiProviderSettings.custom_provider_id : aiProviderSettings.provider_type }}</dd>
            </div>
            <div>
              <dt>模型</dt>
              <dd>{{ aiProviderSettings.model || "未配置" }}</dd>
            </div>
            <div>
              <dt>密钥</dt>
              <dd>{{ aiProviderSettings.has_api_key ? (aiProviderSettings.api_key_hint || "已保存") : "未保存" }}</dd>
            </div>
            <div>
              <dt>更新时间</dt>
              <dd>{{ aiProviderSettings.updated_at ? formatDateTime(aiProviderSettings.updated_at) : "尚未保存" }}</dd>
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
            <el-input v-model="aiProviderForm.model" :placeholder="aiProviderModelPlaceholder" />
            <el-input v-model="aiProviderForm.temperature" placeholder="temperature，0 到 1" />
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
              <dd>{{ aiProviderFormIsCustom ? aiProviderForm.api : "使用 Pi 内置模型定义" }}</dd>
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

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>属性建议任务</h2>
              <p>从类目预检失败项读取缺失属性和商品资料，请 AI 生成候选值，再交给本地规则校验是否能自动补齐。</p>
            </div>
            <el-button :icon="Refresh" @click="runPublishAiAttributeSuggestionsOnce">生成建议</el-button>
          </div>
          <dl class="status-list">
            <div>
              <dt>输入来源</dt>
              <dd>CATEGORY_ATTRS_NEED_AI_FILL 失败项</dd>
            </div>
            <div>
              <dt>输出位置</dt>
              <dd>publish_attribute_suggestions.prompt_json / suggestion_json</dd>
            </div>
          </dl>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>属性建议确认</h2>
              <p>低置信建议不会自动写回，人工采纳后只回到待类目预检状态，后续仍按正常铺货状态机推进。</p>
            </div>
            <div class="button-group">
              <el-select
                v-model="attributeSuggestionStatusFilter"
                class="status-filter"
                @change="refreshAttributeSuggestions"
              >
                <el-option value="pending" label="待确认" />
                <el-option value="all" label="全部建议" />
                <el-option value="applied" label="已采纳" />
              </el-select>
              <el-button :icon="Refresh" @click="refreshAttributeSuggestions">刷新</el-button>
              <el-button
                type="primary"
                :loading="attributeSuggestionApplying"
                :disabled="selectedAttributeSuggestions.filter((row) => !row.applied).length === 0"
                @click="applySelectedAttributeSuggestions"
              >
                批量采纳
              </el-button>
            </div>
          </div>

          <dl class="status-list compact">
            <div>
              <dt>待确认</dt>
              <dd>{{ pendingAttributeSuggestionCount }}</dd>
            </div>
            <div>
              <dt>已采纳</dt>
              <dd>{{ appliedAttributeSuggestionCount }}</dd>
            </div>
            <div>
              <dt>当前筛选</dt>
              <dd>{{ attributeSuggestionTotal }}</dd>
            </div>
          </dl>

          <el-table
            :data="attributeSuggestions"
            class="dense-table"
            @selection-change="handleAttributeSuggestionSelection"
          >
            <el-table-column type="selection" width="48" />
            <el-table-column label="状态" width="100">
              <template #default="{ row }">
                <el-tag :type="row.applied ? 'success' : 'warning'">
                  {{ row.applied ? "已采纳" : "待确认" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="shop_name" label="店铺" min-width="120" />
            <el-table-column label="商品" min-width="240" show-overflow-tooltip>
              <template #default="{ row }">
                <span>{{ row.title }}</span>
                <small class="subtext">{{ row.external_product_id }}</small>
              </template>
            </el-table-column>
            <el-table-column label="属性" min-width="150">
              <template #default="{ row }">
                <span>{{ row.attr_key }}</span>
                <small class="subtext">{{ attributeKindLabel(row.attr_kind) }}</small>
              </template>
            </el-table-column>
            <el-table-column label="建议值" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                {{ attributeSuggestionValue(row) }}
              </template>
            </el-table-column>
            <el-table-column label="允许值" min-width="180" show-overflow-tooltip>
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
            <el-table-column prop="reason" label="原因" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                {{ row.reason || "-" }}
              </template>
            </el-table-column>
            <el-table-column label="更新时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
            </el-table-column>
            <el-table-column label="操作" width="100">
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
      </section>
</template>
