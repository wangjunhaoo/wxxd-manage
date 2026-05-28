<script setup lang="ts">
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  categoryCache,
  categoryCatalogShops,
  categoryKeyword,
  categoryRuleCatId,
  formatDateTime,
  freightTemplates,
  Refresh,
  refreshCategoryCatalog,
  selectedCategoryShopId,
  shops,
  syncSelectedCategoryRules,
  syncSelectedShopCategoryCatalog,
} = props.ctx;
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>类目规则缓存</h2>
              <p>按店铺同步微信类目树、类目详情、发品规则和运费模板，给批量铺货补齐参数使用。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshCategoryCatalog">刷新</el-button>
              <el-button type="primary" :icon="Refresh" @click="syncSelectedShopCategoryCatalog">同步类目树</el-button>
            </div>
          </div>
          <div class="form-grid compact-form">
            <el-select v-model="selectedCategoryShopId" placeholder="选择店铺" @change="refreshCategoryCatalog">
              <el-option
                v-for="shop in shops"
                :key="shop.id"
                :label="shop.name"
                :value="shop.id"
              />
            </el-select>
            <el-input
              v-model="categoryKeyword"
              placeholder="搜索类目名称或 cat_id"
              clearable
              @change="refreshCategoryCatalog"
            />
            <el-input v-model="categoryRuleCatId" placeholder="同步规则 cat_id" />
            <el-button :icon="Refresh" @click="syncSelectedCategoryRules()">同步选中类目规则</el-button>
          </div>
          <dl class="status-list compact">
            <div v-for="summary in categoryCatalogShops" :key="summary.shop_id">
              <dt>{{ summary.shop_name }}</dt>
              <dd>
                类目 {{ summary.category_count }} / 详情 {{ summary.detail_count }} / 发品规则 {{ summary.product_rule_count }} / 运费模板 {{ summary.freight_template_count }}
              </dd>
            </div>
          </dl>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>类目列表</h2>
            <el-tag>{{ categoryCache.length }} 条</el-tag>
          </div>
          <el-table :data="categoryCache" class="dense-table">
            <el-table-column prop="name" label="类目" min-width="160" />
            <el-table-column prop="cat_id" label="cat_id" min-width="130" />
            <el-table-column prop="parent_cat_id" label="父类目" min-width="120">
              <template #default="{ row }">
                {{ row.parent_cat_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="level" label="层级" width="80" />
            <el-table-column label="详情" width="100">
              <template #default="{ row }">
                <el-tag :type="row.has_detail ? 'success' : 'warning'">
                  {{ row.has_detail ? "已同步" : "未同步" }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="属性/资质" min-width="160">
              <template #default="{ row }">
                <span>{{ row.product_attr_count }} 商品 / {{ row.sale_attr_count }} 销售 / {{ row.product_qua_count }} 资质</span>
              </template>
            </el-table-column>
            <el-table-column label="规则" min-width="150">
              <template #default="{ row }">
                <el-tag :type="row.has_product_rule ? 'success' : 'warning'">发品</el-tag>
                <el-tag :type="row.has_delivery_rule ? 'success' : 'warning'">发货</el-tag>
              </template>
            </el-table-column>
            <el-table-column label="操作" width="130">
              <template #default="{ row }">
                <el-button size="small" @click="syncSelectedCategoryRules(row.cat_id)">同步规则</el-button>
              </template>
            </el-table-column>
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title">
            <h2>运费模板</h2>
            <el-tag>{{ freightTemplates.length }} 个</el-tag>
          </div>
          <el-table :data="freightTemplates" class="dense-table">
            <el-table-column prop="shop_name" label="店铺" min-width="150" />
            <el-table-column prop="template_id" label="模板 ID" min-width="180" />
            <el-table-column label="同步时间" min-width="190">
              <template #default="{ row }">{{ formatDateTime(row.synced_at) }}</template>
            </el-table-column>
          </el-table>
        </div>
      </section>
</template>
