<script setup lang="ts">
import { computed, ref } from "vue";
import { ElMessage } from "element-plus";
import type { WxXdAppContext } from "../../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  createOrderPriceAdjustmentJob,
  currentOrderPriceAdjustmentJob,
  formatCents,
  formatDateTime,
  latestOrderPriceAdjustmentTaskId,
  orderPriceAdjustmentPayload,
  queriedOrderPriceAdjustmentTaskId,
  queryOrderPriceAdjustmentJob,
  Refresh,
  runOrderDetailSyncOnce,
  runOrderPriceAdjustmentOnce,
  runUnpaidOrderSyncOnce,
  Search,
  shops,
  statusType,
  UploadFilled,
} = props.ctx;

type OrderPriceRow = {
  shop_id: string;
  wechat_order_id: string;
  product_id: string;
  sku_id: string;
  change_price_yuan: string;
  change_express: boolean;
  express_fee_yuan: string;
  note: string;
};

const orderRows = ref<OrderPriceRow[]>([
  {
    shop_id: "",
    wechat_order_id: "",
    product_id: "",
    sku_id: "",
    change_price_yuan: "",
    change_express: false,
    express_fee_yuan: "",
    note: "未付款订单人工让利",
  },
]);

const orderPriceStatusCounts = computed(() => {
  const counts = {
    total: 0,
    pending: 0,
    success: 0,
    failed: 0,
  };
  for (const item of currentOrderPriceAdjustmentJob.value?.items ?? []) {
    counts.total += 1;
    if (item.status === "pending" || item.status === "submitting") {
      counts.pending += 1;
    } else if (item.status === "success") {
      counts.success += 1;
    } else if (item.status === "failed") {
      counts.failed += 1;
    }
  }
  return counts;
});

function addOrderRow() {
  const lastRow = orderRows.value[orderRows.value.length - 1];
  orderRows.value.push({
    shop_id: lastRow?.shop_id ?? "",
    wechat_order_id: "",
    product_id: "",
    sku_id: "",
    change_price_yuan: "",
    change_express: false,
    express_fee_yuan: "",
    note: "未付款订单人工让利",
  });
}

function removeOrderRow(index: number) {
  if (orderRows.value.length === 1) {
    return;
  }
  orderRows.value.splice(index, 1);
}

function yuanToCents(value: string) {
  const numberValue = Number(value);
  if (!Number.isFinite(numberValue)) {
    return NaN;
  }
  return Math.round(numberValue * 100);
}

async function createOrderPriceAdjustmentFromForm() {
  const grouped = new Map<string, {
    shop_id: string;
    wechat_order_id: string;
    change_express: boolean;
    express_fee_cents: number | null;
    note: string;
    lines: Array<{ product_id: string; sku_id: string; change_price_cents: number }>;
  }>();

  for (const row of orderRows.value) {
    const shopId = row.shop_id.trim();
    const orderId = row.wechat_order_id.trim();
    const productId = row.product_id.trim();
    const skuId = row.sku_id.trim();
    const changePriceCents = yuanToCents(row.change_price_yuan);
    const expressFeeCents = row.change_express
      ? yuanToCents(row.express_fee_yuan || "0")
      : null;

    if (!shopId || !orderId || !productId || !skuId || !Number.isFinite(changePriceCents) || changePriceCents <= 0) {
      ElMessage.error("请填写店铺、待付款订单号、商品 ID、SKU ID 和大于 0 的商品目标总价");
      return;
    }
    if (row.change_express && (!Number.isFinite(expressFeeCents) || Number(expressFeeCents) < 0)) {
      ElMessage.error("运费目标不能小于 0");
      return;
    }

    const key = `${shopId}:${orderId}`;
    const existing = grouped.get(key);
    if (existing) {
      existing.lines.push({
        product_id: productId,
        sku_id: skuId,
        change_price_cents: changePriceCents,
      });
      if (row.change_express) {
        existing.change_express = true;
        existing.express_fee_cents = Number(expressFeeCents);
      }
      continue;
    }

    grouped.set(key, {
      shop_id: shopId,
      wechat_order_id: orderId,
      change_express: row.change_express,
      express_fee_cents: row.change_express ? Number(expressFeeCents) : null,
      note: row.note.trim() || "未付款订单人工让利",
      lines: [
        {
          product_id: productId,
          sku_id: skuId,
          change_price_cents: changePriceCents,
        },
      ],
    });
  }

  const orders = Array.from(grouped.values());
  if (orders.length === 0) {
    ElMessage.error("请至少填写一个待付款订单");
    return;
  }

  orderPriceAdjustmentPayload.value = JSON.stringify({
    request_id: `order-price-${Date.now()}`,
    orders,
  }, null, 2);
  await createOrderPriceAdjustmentJob();
}
</script>

<template>
  <section class="content-stack">
    <div class="panel">
      <div class="panel-title">
        <div>
          <h2>未付款订单改价</h2>
          <p>按订单填写商品 SKU 的最新总价；微信只支持付款前改低价格，商品未填则不改。</p>
        </div>
      </div>

      <div class="price-builder order-price-builder">
        <div class="order-price-toolbar">
          <el-button :icon="Refresh" @click="runUnpaidOrderSyncOnce">同步待付款订单</el-button>
          <el-button :icon="Refresh" @click="runOrderDetailSyncOnce">同步订单详情</el-button>
          <span>先同步待付款订单和详情，再批量提交改价，避免填错商品 SKU。</span>
        </div>

        <div class="price-product-list">
          <div
            v-for="(row, index) in orderRows"
            :key="index"
            class="price-product-row order-price-row"
          >
            <el-select v-model="row.shop_id" filterable placeholder="选择店铺">
              <el-option
                v-for="shop in shops"
                :key="shop.id"
                :label="`${shop.name}（${shop.group_name}）`"
                :value="shop.id"
              />
            </el-select>
            <el-input v-model="row.wechat_order_id" placeholder="待付款订单号" />
            <el-input v-model="row.product_id" placeholder="订单商品 product_id" />
            <el-input v-model="row.sku_id" placeholder="订单 SKU ID" />
            <el-input v-model="row.change_price_yuan" placeholder="商品目标总价，元" />
            <el-checkbox v-model="row.change_express">改运费</el-checkbox>
            <el-input v-model="row.express_fee_yuan" :disabled="!row.change_express" placeholder="运费目标，元" />
            <el-input v-model="row.note" placeholder="改价备注" />
            <el-button text type="danger" :disabled="orderRows.length === 1" @click="removeOrderRow(index)">
              删除
            </el-button>
          </div>
        </div>
      </div>

      <div class="action-row">
        <el-button type="primary" :icon="UploadFilled" @click="createOrderPriceAdjustmentFromForm">创建订单改价任务</el-button>
        <el-button @click="addOrderRow">添加订单商品</el-button>
        <el-button :icon="UploadFilled" @click="runOrderPriceAdjustmentOnce">提交微信订单改价</el-button>
        <el-tag v-if="latestOrderPriceAdjustmentTaskId">最新任务：{{ latestOrderPriceAdjustmentTaskId }}</el-tag>
      </div>

      <el-collapse class="ops-advanced-collapse">
        <el-collapse-item title="高级 JSON 协议" name="json">
          <el-input
            v-model="orderPriceAdjustmentPayload"
            type="textarea"
            :autosize="{ minRows: 10, maxRows: 18 }"
            spellcheck="false"
            class="json-editor"
          />
          <div class="action-row">
            <el-button type="primary" :icon="UploadFilled" @click="createOrderPriceAdjustmentJob">按 JSON 创建任务</el-button>
          </div>
        </el-collapse-item>
      </el-collapse>
    </div>

    <div class="panel">
      <div class="panel-title">
        <h2>查询订单改价任务</h2>
        <p>可以看到订单是否未付款、商品明细是否匹配，以及微信接口返回的改价失败原因。</p>
      </div>
      <div class="inline-form">
        <el-input v-model="queriedOrderPriceAdjustmentTaskId" placeholder="order-price-xxx" />
        <el-button type="primary" :icon="Search" @click="queryOrderPriceAdjustmentJob">查询</el-button>
      </div>
    </div>

    <div v-if="currentOrderPriceAdjustmentJob" class="panel">
      <div class="panel-title">
        <h2>{{ currentOrderPriceAdjustmentJob.id }}</h2>
        <el-tag :type="statusType(currentOrderPriceAdjustmentJob.status)">{{ currentOrderPriceAdjustmentJob.status }}</el-tag>
      </div>
      <dl class="status-list compact">
        <div>
          <dt>request_id</dt>
          <dd>{{ currentOrderPriceAdjustmentJob.request_id }}</dd>
        </div>
        <div>
          <dt>订单数</dt>
          <dd>{{ currentOrderPriceAdjustmentJob.accepted_order_count }}</dd>
        </div>
        <div>
          <dt>待提交</dt>
          <dd>{{ orderPriceStatusCounts.pending }}</dd>
        </div>
        <div>
          <dt>成功</dt>
          <dd>{{ orderPriceStatusCounts.success }}</dd>
        </div>
        <div>
          <dt>失败</dt>
          <dd>{{ orderPriceStatusCounts.failed }}</dd>
        </div>
      </dl>
      <el-table :data="currentOrderPriceAdjustmentJob.items" class="dense-table">
        <el-table-column prop="wechat_order_id" label="订单号" min-width="190" />
        <el-table-column prop="shop_name" label="店铺" min-width="130" />
        <el-table-column label="商品目标总价" min-width="180">
          <template #default="{ row }">
            <div v-for="line in row.change_order_infos" :key="`${line.product_id}-${line.sku_id}`" class="order-price-line">
              <span>{{ line.product_id }} / {{ line.sku_id }}</span>
              <b>{{ formatCents(line.change_price_cents) }}</b>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="运费" width="120">
          <template #default="{ row }">
            {{ row.change_express ? formatCents(row.express_fee_cents || 0) : "不修改" }}
          </template>
        </el-table-column>
        <el-table-column prop="status" label="状态" width="130">
          <template #default="{ row }">
            <el-tag :type="statusType(row.status)">{{ row.status }}</el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="error_summary" label="处理原因" min-width="260" show-overflow-tooltip>
          <template #default="{ row }">
            {{ row.error_summary || row.error_code || "-" }}
          </template>
        </el-table-column>
        <el-table-column label="更新时间" min-width="190">
          <template #default="{ row }">{{ formatDateTime(row.updated_at) }}</template>
        </el-table-column>
      </el-table>
    </div>
  </section>
</template>
