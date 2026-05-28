<script setup lang="ts">
import type { WxXdAppContext } from "../composables/useWxXdApp";

const props = defineProps<{ ctx: WxXdAppContext }>();
const {
  collectionFilePath,
  collectionFileName,
  collectionImporting,
  collectionImportVisible,
  clearCollectionExcelFile,
  selectCollectionExcelFile,
  startExcelImport,
  UploadFilled,
} = props.ctx;
</script>

<template>
      <!-- 批量导入采集铺货 Dialog -->
    <el-dialog
      v-model="collectionImportVisible"
      title="批量导入采集铺货"
      width="600px"
      append-to-body
    >
      <div class="collection-import-form" style="display: flex; flex-direction: column; gap: 20px;">
        <div>
          <span style="font-weight: bold; display: block; margin-bottom: 8px;">1. 选择 Excel 文件</span>
          <div class="excel-file-picker">
            <el-button type="primary" :icon="UploadFilled" @click="selectCollectionExcelFile">
              选择文件
            </el-button>
            <el-button :disabled="!collectionFilePath" @click="clearCollectionExcelFile">
              清除
            </el-button>
            <div class="excel-file-summary" :class="{ empty: !collectionFilePath }">
              <span class="file-name">{{ collectionFileName || "尚未选择文件" }}</span>
              <span v-if="collectionFilePath" class="file-path">{{ collectionFilePath }}</span>
            </div>
          </div>
          <p style="font-size: 12px; color: #8c6b30; margin-top: 6px;">
            提示：Excel 文件无表头。第一列商品名称，第二列淘宝链接，第三列微信类目路径（用 > 连接）。
          </p>
        </div>

        <div>
          <span style="font-weight: bold; display: block; margin-bottom: 8px;">2. 采集后选择铺货目标</span>
          <p style="font-size: 13px; color: #6c685e; margin: 0; line-height: 1.5;">
            导入只会创建淘宝采集任务。采集完成后，可在采集结果列表里多选商品和微信小店，再创建铺货任务。
          </p>
        </div>

        <div style="background-color: rgba(195, 138, 33, 0.08); border-left: 4px solid #c38a21; padding: 12px; border-radius: 4px;">
          <p style="font-size: 13px; color: #7f5f19; margin: 0; line-height: 1.5;">
            <strong>安全提示：</strong>采集淘宝商品需要模拟浏览器环境。如果遇到反爬限流，请先点击铺货任务页面上的<b>“淘宝登录”</b>按钮，在弹出的浏览器中手动登录一次淘宝以建立登录态。
          </p>
        </div>
      </div>
      
      <template #footer>
        <span class="dialog-footer">
          <el-button @click="collectionImportVisible = false">取消</el-button>
          <el-button
            type="primary"
            :loading="collectionImporting"
            @click="startExcelImport"
          >
            开始导入并采集
          </el-button>
        </span>
      </template>
    </el-dialog>
</template>
