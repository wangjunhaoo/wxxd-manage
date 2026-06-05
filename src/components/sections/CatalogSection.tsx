/* ============================================================================
   类目规则缓存（catalog）—— 按店铺同步并缓存类目权限、类目树/详情、发品/发货规则、
   运费模板，为批量铺货补齐参数提供数据底座。忠实保留 4 个面板的 IA 与交互，套用 Soft。
   ============================================================================ */
import { useMemo, useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { PageHead, Button, Pill, Segmented, Select, Empty } from "../primitives";
import type { CategoryCacheView, CategoryRelationView } from "../../types/app";

type CategoryViewMode = "tree" | "list";

// 树节点：在缓存视图上挂子节点与缺父标记
type CategoryTreeNode = CategoryCacheView & {
  children?: CategoryTreeNode[];
  hasMissingParent: boolean;
  treeKey: string;
};
type CategoryTreeRow = {
  node: CategoryTreeNode;
  depth: number;
};

// key = `${shop_id}:${cat_id}`，唯一标识一个缓存行/树节点
function categoryRowKey(row: CategoryCacheView): string {
  return `${row.shop_id}:${row.cat_id}`;
}

// 按 parent_cat_id 连边构建森林；父级缺失或自指 → 作为根并标记 hasMissingParent
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

// 排序：先 level 升序，再 name（zh-Hans-CN 本地化）、再 cat_id；无子节点时移除空 children
function sortCategoryNodes(nodes: CategoryTreeNode[]): void {
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

// 按展开集合 / 强制全展开把树拍平成可渲染的行序列
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

// 关系状态 pill：1→生效中(success)，2→已失效(info)，其它→状态 N(info)
function relationStatusTone(row: CategoryRelationView): "success" | "info" {
  return row.status === 1 ? "success" : "info";
}
function relationStatusLabel(row: CategoryRelationView): string {
  if (row.status === 1) {
    return "生效中";
  }
  if (row.status === 2) {
    return "已失效";
  }
  return `状态 ${row.status}`;
}

// 类目权限 pill：是否对店铺生效
function categoryPermissionTone(row: CategoryCacheView): "success" | "info" {
  return row.is_available_for_shop ? "success" : "info";
}
function categoryPermissionLabel(row: CategoryCacheView): string {
  return row.is_available_for_shop ? "生效权限" : "路径节点";
}

export default function CatalogSection() {
  const ctx = useApp();

  // ---- ctx 响应式数据 ----
  const categoryCache = ctx.categoryCache.value;
  const categoryCatalogShops = ctx.categoryCatalogShops.value;
  const categoryRelations = ctx.categoryRelations.value;
  const freightTemplates = ctx.freightTemplates.value;
  const shops = ctx.shops.value;
  const keyword = ctx.categoryKeyword.value;

  // ---- 本地视图状态 ----
  const [viewMode, setViewMode] = useState<CategoryViewMode>("tree");
  const [expandedKeys, setExpandedKeys] = useState<string[]>([]);

  // 关键词非空 → 强制全展开且禁用手动折叠
  const shouldExpandTree = keyword.trim().length > 0;
  const expandedKeySet = useMemo(() => new Set(expandedKeys), [expandedKeys]);

  const categoryTree = useMemo(() => buildCategoryTree(categoryCache), [categoryCache]);
  const categoryTreeRows = useMemo(
    () => flattenCategoryTree(categoryTree, expandedKeySet, shouldExpandTree),
    [categoryTree, expandedKeySet, shouldExpandTree],
  );

  // 空态文案随关键词动态切换
  const categoryEmptyText = keyword.trim() ? "没有找到匹配的类目" : "暂无类目缓存";
  const categoryRelationsEmptyText = keyword.trim()
    ? "没有找到匹配的店铺类目权限"
    : "暂无店铺类目权限";

  const isNodeExpanded = (node: CategoryTreeNode): boolean =>
    shouldExpandTree || expandedKeySet.has(node.treeKey);

  const toggleNode = (node: CategoryTreeNode): void => {
    if (!node.children?.length || shouldExpandTree) {
      return;
    }
    const next = new Set(expandedKeys);
    if (next.has(node.treeKey)) {
      next.delete(node.treeKey);
    } else {
      next.add(node.treeKey);
    }
    setExpandedKeys(Array.from(next));
  };

  return (
    <div className="pad">
      <div className="wrap-wide">
        <div className="stack">
          {/* ============ Panel 1 — 类目规则缓存（控制面板 + KPI 条） ============ */}
          <div className="panel">
            <PageHead
              eyebrow="类目规则缓存 · Asia/Shanghai"
              title="类目规则缓存"
              desc="按店铺同步生效类目权限、类目详情、发品规则和运费模板，给批量铺货补齐参数使用。"
              actions={
                <>
                  <Button icon="refresh" onClick={() => ctx.refreshCategoryCatalog()}>
                    刷新
                  </Button>
                  <Button
                    variant="graphite"
                    icon="refresh"
                    onClick={() => ctx.syncSelectedShopCategoryCatalog()}
                  >
                    同步店铺类目
                  </Button>
                </>
              }
            />

            {/* 筛选/表单行 */}
            <div className="form-grid cols-2" style={{ marginTop: 16 }}>
              <Select
                value={ctx.selectedCategoryShopId.value}
                placeholder="选择店铺"
                options={shops.map((shop) => ({ label: shop.name, value: shop.id }))}
                onChange={(v: string) => {
                  ctx.selectedCategoryShopId.value = v;
                  ctx.refreshCategoryCatalog();
                }}
              />
              <input
                className="inp"
                placeholder="搜索类目名称或 cat_id"
                value={ctx.categoryKeyword.value}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  ctx.categoryKeyword.value = e.target.value;
                }}
                onBlur={() => ctx.refreshCategoryCatalog()}
                onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => {
                  if (e.key === "Enter") {
                    ctx.refreshCategoryCatalog();
                  }
                }}
              />
              <input
                className="inp"
                placeholder="同步规则 cat_id"
                value={ctx.categoryRuleCatId.value}
                onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                  ctx.categoryRuleCatId.value = e.target.value;
                }}
              />
              <Button icon="refresh" onClick={() => ctx.syncSelectedCategoryRules()}>
                同步选中类目规则
              </Button>
            </div>

            {/* KPI 条：各店铺缓存汇总 */}
            <div className="kv-grid" style={{ marginTop: 16 }}>
              {categoryCatalogShops.map((summary) => (
                <div className="kv" key={summary.shop_id}>
                  <dt>{summary.shop_name}</dt>
                  <dd>
                    店铺权限 {summary.active_category_relation_count} / 类目节点{" "}
                    {summary.category_count} / 详情 {summary.detail_count} / 发品规则{" "}
                    {summary.product_rule_count} / 运费模板 {summary.freight_template_count}
                  </dd>
                </div>
              ))}
            </div>
          </div>

          {/* ============ Panel 2 — 店铺类目权限（表格） ============ */}
          <div className="panel">
            <div className="ph">
              <div>
                <h3>店铺类目权限</h3>
                <p>来自微信店铺类目权限接口；铺货只能选择这里处于生效中的类目。</p>
              </div>
              <Pill tone="info">{categoryRelations.length} 条</Pill>
            </div>
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>类目</th>
                    <th>状态</th>
                    <th>资质 ID</th>
                    <th>生效时间</th>
                    <th>失效原因</th>
                    <th>同步时间</th>
                  </tr>
                </thead>
                <tbody>
                  {categoryRelations.length === 0 ? (
                    <tr>
                      <td colSpan={6}>
                        <Empty>{categoryRelationsEmptyText}</Empty>
                      </td>
                    </tr>
                  ) : (
                    categoryRelations.map((row) => (
                      <tr key={`${row.shop_id}:${row.cat_id}`}>
                        <td>
                          <div className="cell-main">
                            <strong>{row.category_name || "未同步类目路径"}</strong>
                            <span className="mono">cat_id {row.cat_id}</span>
                          </div>
                        </td>
                        <td>
                          <Pill tone={relationStatusTone(row)}>{relationStatusLabel(row)}</Pill>
                        </td>
                        <td>{row.qua_id || "-"}</td>
                        <td>{ctx.formatUnixTime(row.effective_time)}</td>
                        <td>{row.uneffective_reason || "-"}</td>
                        <td>{ctx.formatDateTime(row.synced_at)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>

          {/* ============ Panel 3 — 类目（树形 / 列表 双视图） ============ */}
          <div className="panel">
            <div className="ph">
              <div>
                <h3>类目</h3>
                <p>树形展示按父子类目展开；搜索类目名称或 cat_id 时会保留命中的上下文。</p>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                <Pill tone="info">{categoryCache.length} 条</Pill>
                <Segmented
                  value={viewMode}
                  onChange={(v: string) => setViewMode(v as CategoryViewMode)}
                  options={[
                    { label: "树形", value: "tree" },
                    { label: "列表", value: "list" },
                  ]}
                />
              </div>
            </div>

            {viewMode === "tree" ? (
              <div className="tbl-wrap">
                <table className="tbl">
                  <thead>
                    <tr>
                      <th>类目</th>
                      <th>层级</th>
                      <th>权限</th>
                      <th>详情</th>
                      <th>属性/资质</th>
                      <th>规则</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {categoryTreeRows.length === 0 ? (
                      <tr>
                        <td colSpan={7}>
                          <Empty>{categoryEmptyText}</Empty>
                        </td>
                      </tr>
                    ) : (
                      categoryTreeRows.map(({ node, depth }) => (
                        <tr key={node.treeKey}>
                          <td>
                            <div
                              style={{
                                display: "flex",
                                alignItems: "flex-start",
                                gap: 8,
                                paddingLeft: depth * 22,
                              }}
                            >
                              {node.children?.length ? (
                                <button
                                  type="button"
                                  className="tree-toggle"
                                  aria-expanded={isNodeExpanded(node)}
                                  onClick={() => toggleNode(node)}
                                  style={{
                                    width: 20,
                                    height: 20,
                                    lineHeight: "18px",
                                    border: "1px solid var(--line)",
                                    borderRadius: 6,
                                    background: "transparent",
                                    cursor: "pointer",
                                    flex: "0 0 auto",
                                  }}
                                >
                                  {isNodeExpanded(node) ? "-" : "+"}
                                </button>
                              ) : (
                                <span style={{ width: 20, flex: "0 0 auto" }} />
                              )}
                              <div className="cell-main">
                                <strong>{node.name}</strong>
                                <span className="mono">
                                  cat_id {node.cat_id}
                                  {node.parent_cat_id ? ` / 父类目 ${node.parent_cat_id}` : ""}
                                  {node.children?.length
                                    ? ` / 子类目 ${node.children.length}`
                                    : ""}
                                  {node.hasMissingParent ? " / 父级未在结果内" : ""}
                                </span>
                              </div>
                            </div>
                          </td>
                          <td>L{node.level || "-"}</td>
                          <td>
                            <Pill tone={categoryPermissionTone(node)}>
                              {categoryPermissionLabel(node)}
                            </Pill>
                          </td>
                          <td>
                            <Pill tone={node.has_detail ? "success" : "warning"}>
                              {node.has_detail ? "已同步" : "未同步"}
                            </Pill>
                          </td>
                          <td>
                            {node.product_attr_count} 商品 / {node.sale_attr_count} 销售 /{" "}
                            {node.product_qua_count} 资质
                          </td>
                          <td>
                            <span style={{ display: "inline-flex", gap: 6 }}>
                              <Pill tone={node.has_product_rule ? "success" : "warning"}>发品</Pill>
                              <Pill tone={node.has_delivery_rule ? "success" : "warning"}>发货</Pill>
                            </span>
                          </td>
                          <td>
                            <div className="row-actions">
                              <Button
                                size="sm"
                                onClick={() => ctx.syncSelectedCategoryRules(node.cat_id)}
                              >
                                同步规则
                              </Button>
                            </div>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            ) : (
              <div className="tbl-wrap">
                <table className="tbl">
                  <thead>
                    <tr>
                      <th>类目</th>
                      <th>cat_id</th>
                      <th>父类目</th>
                      <th>层级</th>
                      <th>权限</th>
                      <th>详情</th>
                      <th>属性/资质</th>
                      <th>规则</th>
                      <th>操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {categoryCache.length === 0 ? (
                      <tr>
                        <td colSpan={9}>
                          <Empty>{categoryEmptyText}</Empty>
                        </td>
                      </tr>
                    ) : (
                      categoryCache.map((row) => (
                        <tr key={categoryRowKey(row)}>
                          <td>{row.name}</td>
                          <td>{row.cat_id}</td>
                          <td>{row.parent_cat_id || "-"}</td>
                          <td>{row.level}</td>
                          <td>
                            <Pill tone={categoryPermissionTone(row)}>
                              {categoryPermissionLabel(row)}
                            </Pill>
                          </td>
                          <td>
                            <Pill tone={row.has_detail ? "success" : "warning"}>
                              {row.has_detail ? "已同步" : "未同步"}
                            </Pill>
                          </td>
                          <td>
                            {row.product_attr_count} 商品 / {row.sale_attr_count} 销售 /{" "}
                            {row.product_qua_count} 资质
                          </td>
                          <td>
                            <span style={{ display: "inline-flex", gap: 6 }}>
                              <Pill tone={row.has_product_rule ? "success" : "warning"}>发品</Pill>
                              <Pill tone={row.has_delivery_rule ? "success" : "warning"}>发货</Pill>
                            </span>
                          </td>
                          <td>
                            <div className="row-actions">
                              <Button
                                size="sm"
                                onClick={() => ctx.syncSelectedCategoryRules(row.cat_id)}
                              >
                                同步规则
                              </Button>
                            </div>
                          </td>
                        </tr>
                      ))
                    )}
                  </tbody>
                </table>
              </div>
            )}
          </div>

          {/* ============ Panel 4 — 运费模板（表格） ============ */}
          <div className="panel">
            <div className="ph">
              <div>
                <h3>运费模板</h3>
              </div>
              <Pill tone="info">{freightTemplates.length} 个</Pill>
            </div>
            <div className="tbl-wrap">
              <table className="tbl">
                <thead>
                  <tr>
                    <th>店铺</th>
                    <th>模板 ID</th>
                    <th>同步时间</th>
                  </tr>
                </thead>
                <tbody>
                  {freightTemplates.length === 0 ? (
                    <tr>
                      <td colSpan={3}>
                        <Empty>暂无运费模板</Empty>
                      </td>
                    </tr>
                  ) : (
                    freightTemplates.map((row) => (
                      <tr key={`${row.shop_id}:${row.template_id}`}>
                        <td>{row.shop_name}</td>
                        <td>{row.template_id}</td>
                        <td>{ctx.formatDateTime(row.synced_at)}</td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
