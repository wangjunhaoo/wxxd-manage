<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  checkShopQuota,
  createGroup,
  createShop,
  formatDateTime,
  groupForm,
  groups,
  Plus,
  selectedGroupOptions,
  shopForm,
  shops,
  statusType,
  syncShopBasicInfo,
  verifyShop,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <h2>店铺组</h2>
            <el-tag>{{ groups.length }} 组</el-tag>
          </div>
          <div class="inline-form">
            <el-input v-model="groupForm.name" placeholder="例如：默认铺货组" />
            <el-button type="primary" :icon="Plus" @click="createGroup">新建店铺组</el-button>
          </div>
          <el-table :data="groups" class="dense-table">
            <el-table-column prop="name" label="店铺组" />
            <el-table-column prop="shop_count" label="店铺数" width="100" />
            <el-table-column prop="status" label="状态" width="120">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column prop="id" label="ID" min-width="220" />
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>接入店铺</h2>
            <p>保存时 app_secret 只发送到 Tauri 后端加密入库，前端不展示、不回填。</p>
          </div>
          <div class="form-grid">
            <el-input v-model="shopForm.name" placeholder="店铺名称" />
            <el-input v-model="shopForm.appid" placeholder="微信小店 appid" />
            <el-input
              v-model="shopForm.app_secret"
              placeholder="微信小店 app_secret"
              type="password"
              show-password
              autocomplete="new-password"
            />
            <el-select v-model="shopForm.group_id" placeholder="选择店铺组">
              <el-option
                v-for="group in selectedGroupOptions"
                :key="group.value"
                :label="group.label"
                :value="group.value"
              />
            </el-select>
            <el-button type="primary" :icon="Plus" @click="createShop">保存店铺</el-button>
          </div>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>店铺接入状态</h2>
            <el-tag>{{ shops.length }} 店</el-tag>
          </div>
          <el-table :data="shops" class="dense-table">
            <el-table-column prop="name" label="店铺" min-width="150" />
            <el-table-column prop="appid" label="appid" min-width="170" />
            <el-table-column prop="group_name" label="店铺组" min-width="130" />
            <el-table-column label="微信资料" min-width="170">
              <template #default="{ row }">
                <span>{{ row.wechat_nickname || "-" }}</span>
                <small class="subtext">{{ row.wechat_status || "未同步" }}</small>
              </template>
            </el-table-column>
            <el-table-column prop="status" label="状态" width="130">
              <template #default="{ row }">
                <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="密钥" width="100">
              <template #default="{ row }">
                <el-tag :type="row.has_secret ? 'success' : 'warning'">
                  {{ row.has_secret ? "已保存" : "缺失" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="token 到期" min-width="190">
              <template #default="{ row }">
                {{ formatDateTime(row.token_expires_at) }}
              </template>
            </el-table-column>
            <el-table-column label="接口额度" width="110">
              <template #default="{ row }">
                {{ row.last_quota_remain ?? "-" }}
              </template>
            </el-table-column>
            <el-table-column label="操作" width="250">
              <template #default="{ row }">
                <el-button size="small" :disabled="!row.has_secret" @click="verifyShop(row)">
                  验证
                </el-button>
                <el-button size="small" :disabled="!row.has_secret" @click="syncShopBasicInfo(row)">
                  同步资料
                </el-button>
                <el-button size="small" :disabled="!row.has_secret" @click="checkShopQuota(row)">
                  查额度
                </el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
