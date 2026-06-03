<script setup lang="ts">
import { computed, ref } from "vue";
import type { WxXdAppContext } from "../../composables/useWxXdApp";
import type { CategoryCacheView, CategoryRelationView } from "../../types/app";

type CategoryViewMode = "tree" | "list";
type CategoryTreeNode = CategoryCacheView & {
  children?: CategoryTreeNode[];
  hasMissingParent: boolean;
  treeKey: string;
};
type CategoryTreeRow = {
  node: CategoryTreeNode;
  depth: number;
};

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  categoryCache,
  categoryCatalogShops,
  categoryKeyword,
  categoryRelations,
  categoryRuleCatId,
  formatDateTime,
  formatUnixTime,
  freightTemplates,
  Refresh,
  refreshCategoryCatalog,
  selectedCategoryShopId,
  shops,
  syncSelectedCategoryRules,
  syncSelectedShopCategoryCatalog,
} = props.ctx;

const categoryViewMode = ref<CategoryViewMode>("tree");
const expandedCategoryKeys = ref<string[]>([]);

const categoryTree = computed(() => buildCategoryTree(categoryCache.value));
const expandedCategoryKeySet = computed(() => new Set(expandedCategoryKeys.value));
const shouldExpandCategoryTree = computed(() => categoryKeyword.value.trim().length > 0);
const categoryTreeRows = computed(() => {
  return flattenCategoryTree(categoryTree.value, expandedCategoryKeySet.value, shouldExpandCategoryTree.value);
});
const categoryEmptyText = computed(() => {
  return categoryKeyword.value.trim() ? "没有找到匹配的类目" : "暂无类目缓存";
});
const categoryRelationsEmptyText = computed(() => {
  return categoryKeyword.value.trim() ? "没有找到匹配的店铺类目权限" : "暂无店铺类目权限";
});

function categoryRowKey(row: CategoryCacheView) {
  return `${row.shop_id}:${row.cat_id}`;
}

function buildCategoryTree(items: CategoryCacheView[]): CategoryTreeNode[] {
  const nodes = new Map<string, CategoryTreeNode>();
  const roots: CategoryTreeNode[] = [];

  for (const item of items) {
    nodes.set(categoryRowKey(item), {
      ...item,
      children: [],
      hasMissingParent: false,
      treeKey: categoryRowKey(item),
    });
  }

  for (const node of nodes.values()) {
    const parentKey = node.parent_cat_id ? `${node.shop_id}:${node.parent_cat_id}` : "";
    const parent = parentKey ? nodes.get(parentKey) : null;
    if (parent && parent.cat_id !== node.cat_id) {
      parent.children?.push(node);
    } else {
      node.hasMissingParent = Boolean(node.parent_cat_id);
      roots.push(node);
    }
  }

  sortCategoryNodes(roots);
  return roots;
}

function sortCategoryNodes(nodes: CategoryTreeNode[]) {
  nodes.sort((left, right) => {
    const levelDiff = (left.level ?? 0) - (right.level ?? 0);
    if (levelDiff !== 0) {
      return levelDiff;
    }
    return left.name.localeCompare(right.name, "zh-Hans-CN") || left.cat_id - right.cat_id;
  });

  for (const node of nodes) {
    if (node.children && node.children.length > 0) {
      sortCategoryNodes(node.children);
    } else {
      delete node.children;
    }
  }
}

function flattenCategoryTree(
  nodes: CategoryTreeNode[],
  expandedKeys: Set<string>,
  expandAll: boolean,
  depth = 0,
): CategoryTreeRow[] {
  const rows: CategoryTreeRow[] = [];
  for (const node of nodes) {
    rows.push({ node, depth });
    if (node.children?.length && (expandAll || expandedKeys.has(node.treeKey))) {
      rows.push(...flattenCategoryTree(node.children, expandedKeys, expandAll, depth + 1));
    }
  }
  return rows;
}

function isCategoryNodeExpanded(node: CategoryTreeNode) {
  return shouldExpandCategoryTree.value || expandedCategoryKeySet.value.has(node.treeKey);
}

function toggleCategoryNode(node: CategoryTreeNode) {
  if (!node.children?.length || shouldExpandCategoryTree.value) {
    return;
  }
  const nextKeys = new Set(expandedCategoryKeys.value);
  if (nextKeys.has(node.treeKey)) {
    nextKeys.delete(node.treeKey);
  } else {
    nextKeys.add(node.treeKey);
  }
  expandedCategoryKeys.value = Array.from(nextKeys);
}

function categoryPermissionTagType(row: CategoryCacheView) {
  return row.is_available_for_shop ? "success" : "info";
}

function categoryPermissionLabel(row: CategoryCacheView) {
  return row.is_available_for_shop ? "生效权限" : "路径节点";
}

function relationStatusTagType(row: CategoryRelationView) {
  return row.status === 1 ? "success" : "info";
}

function relationStatusLabel(row: CategoryRelationView) {
  if (row.status === 1) {
    return "生效中";
  }
  if (row.status === 2) {
    return "已失效";
  }
  return `状态 ${row.status}`;
}
</script>

<template>
      <section class="content-stack">
        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>类目规则缓存</h2>
              <p>按店铺同步生效类目权限、类目详情、发品规则和运费模板，给批量铺货补齐参数使用。</p>
            </div>
            <div class="button-group">
              <el-button :icon="Refresh" @click="refreshCategoryCatalog">刷新</el-button>
              <el-button type="primary" :icon="Refresh" @click="syncSelectedShopCategoryCatalog">同步店铺类目</el-button>
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
                店铺权限 {{ summary.active_category_relation_count }} / 类目节点 {{ summary.category_count }} / 详情 {{ summary.detail_count }} / 发品规则 {{ summary.product_rule_count }} / 运费模板 {{ summary.freight_template_count }}
              </dd>
            </div>
          </dl>
        </div>

        <div class="panel">
          <div class="panel-title">
            <div>
              <h2>店铺类目权限</h2>
              <p>来自微信店铺类目权限接口；铺货只能选择这里处于生效中的类目。</p>
            </div>
            <el-tag>{{ categoryRelations.length }} 条</el-tag>
          </div>
          <el-table
            :data="categoryRelations"
            :empty-text="categoryRelationsEmptyText"
            class="dense-table"
          >
            <el-table-column label="类目" min-width="220" show-overflow-tooltip>
              <template #default="{ row }">
                <div class="category-name-cell">
                  <strong>{{ row.category_name || "未同步类目路径" }}</strong>
                  <span>cat_id {{ row.cat_id }}</span>
                </div>
              </template>
            </el-table-column>
            <el-table-column label="状态" width="110">
              <template #default="{ row }">
                <el-tag :type="relationStatusTagType(row)">
                  {{ relationStatusLabel(row) }}
                </el-tag>
              </template>
            </el-table-column>
            <el-table-column label="资质 ID" width="120">
              <template #default="{ row }">{{ row.qua_id || "-" }}</template>
            </el-table-column>
            <el-table-column label="生效时间" min-width="150">
              <template #default="{ row }">{{ formatUnixTime(row.effective_time) }}</template>
            </el-table-column>
            <el-table-column label="失效原因" min-width="180" show-overflow-tooltip>
              <template #default="{ row }">{{ row.uneffective_reason || "-" }}</template>
            </el-table-column>
            <el-table-column label="同步时间" min-width="170">
              <template #default="{ row }">{{ formatDateTime(row.synced_at) }}</template>
            </el-table-column>
          </el-table>
        </div>

        <div class="panel">
          <div class="panel-title category-panel-title">
            <div>
              <h2>类目</h2>
              <p>树形展示按父子类目展开；搜索类目名称或 cat_id 时会保留命中的上下文。</p>
            </div>
            <div class="category-panel-actions">
              <el-tag>{{ categoryCache.length }} 条</el-tag>
              <el-radio-group v-model="categoryViewMode" size="small">
                <el-radio-button label="tree">树形</el-radio-button>
                <el-radio-button label="list">列表</el-radio-button>
              </el-radio-group>
            </div>
          </div>
          <div
            v-if="categoryViewMode === 'tree'"
            class="category-tree-list"
          >
            <div class="category-tree-header">
              <span>类目</span>
              <span>层级</span>
              <span>权限</span>
              <span>详情</span>
              <span>属性/资质</span>
              <span>规则</span>
              <span>操作</span>
            </div>
            <div v-if="categoryTreeRows.length === 0" class="category-tree-empty">
              {{ categoryEmptyText }}
            </div>
            <div
              v-for="{ node, depth } in categoryTreeRows"
              :key="node.treeKey"
              class="category-tree-row"
            >
              <div class="category-tree-main" :style="{ paddingLeft: `${depth * 22}px` }">
                <button
                  v-if="node.children?.length"
                  class="category-tree-toggle"
                  type="button"
                  :aria-expanded="isCategoryNodeExpanded(node)"
                  @click="toggleCategoryNode(node)"
                >
                  {{ isCategoryNodeExpanded(node) ? "-" : "+" }}
                </button>
                <span v-else class="category-tree-toggle-placeholder" />
                <div class="category-name-cell">
                  <strong>{{ node.name }}</strong>
                  <span>
                    cat_id {{ node.cat_id }}
                    <template v-if="node.parent_cat_id"> / 父类目 {{ node.parent_cat_id }}</template>
                    <template v-if="node.children?.length"> / 子类目 {{ node.children.length }}</template>
                    <template v-if="node.hasMissingParent"> / 父级未在结果内</template>
                  </span>
                </div>
              </div>
              <span>L{{ node.level || "-" }}</span>
              <span>
                <el-tag :type="categoryPermissionTagType(node)">
                  {{ categoryPermissionLabel(node) }}
                </el-tag>
              </span>
              <span>
                <el-tag :type="node.has_detail ? 'success' : 'warning'">
                  {{ node.has_detail ? "已同步" : "未同步" }}
                </el-tag>
              </span>
              <span>{{ node.product_attr_count }} 商品 / {{ node.sale_attr_count }} 销售 / {{ node.product_qua_count }} 资质</span>
              <span>
                <el-tag :type="node.has_product_rule ? 'success' : 'warning'">发品</el-tag>
                <el-tag :type="node.has_delivery_rule ? 'success' : 'warning'">发货</el-tag>
              </span>
              <span>
                <el-button size="small" @click="syncSelectedCategoryRules(node.cat_id)">同步规则</el-button>
              </span>
            </div>
          </div>
          <el-table
            v-else
            :data="categoryCache"
            :empty-text="categoryEmptyText"
            :row-key="categoryRowKey"
            class="dense-table"
          >
            <el-table-column prop="name" label="类目" min-width="160" />
            <el-table-column prop="cat_id" label="cat_id" min-width="130" />
            <el-table-column prop="parent_cat_id" label="父类目" min-width="120">
              <template #default="{ row }">
                {{ row.parent_cat_id || "-" }}
              </template>
            </el-table-column>
            <el-table-column prop="level" label="层级" width="80" />
            <el-table-column label="权限" width="120">
              <template #default="{ row }">
                <el-tag :type="categoryPermissionTagType(row)">
                  {{ categoryPermissionLabel(row) }}
                </el-tag>
              </template>
            </el-table-column>
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
