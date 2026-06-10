/* ============================================================================
   店铺与密钥管理 —— Soft 设计 · 全宽店铺总览
   布局：(1) 顶部概览条（总数/已连接/待处理/缺密钥）
        (2) 店铺总览：全宽详情行卡，逐店验证/同步/查额度（页面主体）
        (3) 接入新店铺：Modal 弹窗（app_secret 仅后端加密入库）
        (4) 店铺组：低频功能，收进「店铺组」Modal（查看 + 新建），不占主视觉
   ============================================================================ */
import { useState } from "react";
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Field, Select, Modal, Callout, Empty } from "../primitives";

// 店铺凭证状态 → 中文标签（取值见 constants.statusTone）
const SHOP_STATUS_LABEL: Record<string, string> = {
  active: "已连接",
  not_verified: "未验证",
  missing_secret: "缺少密钥",
  auth_failed: "认证失败",
  api_failed: "接口异常",
};
function shopStatusLabel(status: string) {
  return SHOP_STATUS_LABEL[status] ?? status;
}

// element 色调（statusType 返回值）→ 本页 badge/统计卡的语义类
function toneClass(tone: string): "ok" | "warn" | "crit" | "info" {
  if (tone === "success") return "ok";
  if (tone === "warning") return "warn";
  if (tone === "danger") return "crit";
  return "info";
}

// 行卡内的「标签 + 值」元信息项
function MetaItem({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <span className="shop-meta-item">
      <span className="m-lab">{label}</span>
      <span className={`m-val ${mono ? "mono" : ""}`}>{value}</span>
    </span>
  );
}

export default function ShopsSection() {
  const ctx = useApp();
  const groups = ctx.groups.value;
  const shops = ctx.shops.value;

  const [shopModalOpen, setShopModalOpen] = useState(false);
  const [groupModalOpen, setGroupModalOpen] = useState(false);

  // 概览统计：已连接 = active；待处理 = 非 active；缺密钥 = 无 app_secret
  const connected = shops.filter((s) => s.status === "active").length;
  const pending = shops.filter((s) => s.status !== "active").length;
  const missingSecret = shops.filter((s) => !s.has_secret).length;

  function openShopModal() {
    // 打开时确保选中一个店铺组（默认第一个）
    if (!ctx.shopForm.group_id && groups.length > 0) {
      ctx.shopForm.group_id = groups[0].id;
    }
    setShopModalOpen(true);
  }

  function closeShopModal() {
    setShopModalOpen(false);
    // 关闭即放弃录入：清空敏感字段，保留所选店铺组
    ctx.shopForm.name = "";
    ctx.shopForm.appid = "";
    ctx.shopForm.app_secret = "";
  }

  async function handleCreateShop() {
    await ctx.createShop();
    // createShop 成功会清空 name/appid；失败（已 toast）则保留，便于修正
    if (!ctx.shopForm.name && !ctx.shopForm.appid) {
      setShopModalOpen(false);
    }
  }

  const canSubmit =
    ctx.shopForm.name.trim().length > 0 && ctx.shopForm.appid.trim().length > 0;

  return (
    <div className="pad">
      <div className="wrap-wide">
        {/* 概览条 */}
        <div className="shops-stats">
          <div className="shops-stat">
            <div className="s-top">
              <span className="s-dot" />
              <span className="s-lab">店铺总数</span>
            </div>
            <div className="s-num">{shops.length}</div>
            <div className="s-hint">已分 {groups.length} 个店铺组</div>
          </div>
          <div className="shops-stat ok">
            <div className="s-top">
              <span className="s-dot" />
              <span className="s-lab">已连接</span>
            </div>
            <div className="s-num">{connected}</div>
            <div className="s-hint">凭证有效，可正常铺货</div>
          </div>
          <div className={`shops-stat ${pending > 0 ? "warn" : ""}`}>
            <div className="s-top">
              <span className="s-dot" />
              <span className="s-lab">待处理</span>
            </div>
            <div className="s-num">{pending}</div>
            <div className="s-hint">需验证或修复凭证</div>
          </div>
          <div className={`shops-stat ${missingSecret > 0 ? "crit" : ""}`}>
            <div className="s-top">
              <span className="s-dot" />
              <span className="s-lab">缺密钥</span>
            </div>
            <div className="s-num">{missingSecret}</div>
            <div className="s-hint">待补充 app_secret</div>
          </div>
        </div>

        {/* 店铺总览（全宽主体） */}
        <div className="shops-main-head">
          <div className="t">
            <h3>店铺总览</h3>
            <Pill tone="info">{shops.length} 店</Pill>
          </div>
          <div className="shops-head-actions">
            <Button
              variant="outline"
              icon="settings"
              onClick={() => setGroupModalOpen(true)}
            >
              店铺组 · {groups.length}
            </Button>
            <Button variant="accent" icon="plus" onClick={openShopModal}>
              接入店铺
            </Button>
          </div>
        </div>

        {shops.length === 0 ? (
          <Empty>还没有接入店铺，点击右上「接入店铺」开始</Empty>
        ) : (
          <div className="shop-rows">
            {shops.map((row) => {
              const tc = toneClass(ctx.statusType(row.status));
              return (
                <div className="shop-row" key={row.id}>
                  <div className={`shop-row-badge ${tc}`}>
                    {row.name?.[0] ?? "店"}
                  </div>
                  <div className="shop-row-body">
                    <div className="shop-row-head">
                      <span className="s-name">{row.name}</span>
                      <Pill tone={ctx.statusType(row.status)}>
                        {shopStatusLabel(row.status)}
                      </Pill>
                      <Pill tone={row.has_secret ? "success" : "warning"}>
                        {row.has_secret ? "密钥已保存" : "密钥缺失"}
                      </Pill>
                    </div>
                    <div className="shop-meta">
                      <MetaItem label="appid" value={row.appid} mono />
                      <MetaItem label="店铺组" value={row.group_name} />
                      <MetaItem
                        label="微信资料"
                        value={row.wechat_nickname || "未同步"}
                      />
                      <MetaItem
                        label="token 到期"
                        value={ctx.formatDateTime(row.token_expires_at)}
                        mono
                      />
                      <MetaItem
                        label="接口额度"
                        value={row.last_quota_remain ?? "-"}
                      />
                    </div>
                  </div>
                  <div className="shop-row-actions">
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={!row.has_secret}
                      onClick={() => ctx.verifyShop(row)}
                    >
                      验证
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={!row.has_secret}
                      onClick={() => ctx.syncShopBasicInfo(row)}
                    >
                      同步资料
                    </Button>
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={!row.has_secret}
                      onClick={() => ctx.checkShopQuota(row)}
                    >
                      查额度
                    </Button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </div>

      {/* 接入新店铺 —— Modal */}
      <Modal
        open={shopModalOpen}
        title="接入新店铺"
        onClose={closeShopModal}
        footer={
          <>
            <Button variant="ghost" onClick={closeShopModal}>
              取消
            </Button>
            <Button
              variant="accent"
              icon="plus"
              disabled={!canSubmit}
              onClick={handleCreateShop}
            >
              保存店铺
            </Button>
          </>
        }
      >
        <Callout tone="info">
          app_secret 仅发送到 Tauri 后端加密入库，前端不展示、不回填。
        </Callout>
        <div className="form-grid cols-2" style={{ marginTop: 16, marginBottom: 0 }}>
          <Field label="店铺名称">
            <input
              className="inp"
              value={ctx.shopForm.name}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.shopForm.name = e.target.value;
              }}
              placeholder="例如：旗舰店"
            />
          </Field>
          <Field label="微信小店 appid">
            <input
              className="inp"
              value={ctx.shopForm.appid}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.shopForm.appid = e.target.value;
              }}
              placeholder="wx 开头的 appid"
            />
          </Field>
          <Field label="app_secret" span={2}>
            <input
              className="inp"
              type="password"
              autoComplete="new-password"
              value={ctx.shopForm.app_secret}
              onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
                ctx.shopForm.app_secret = e.target.value;
              }}
              placeholder="微信小店 app_secret"
            />
          </Field>
          <Field label="店铺组" span={2}>
            <Select
              value={ctx.shopForm.group_id}
              onChange={(v: string) => {
                ctx.shopForm.group_id = v;
              }}
              options={ctx.selectedGroupOptions.value}
              placeholder="选择店铺组"
            />
          </Field>
        </div>
      </Modal>

      {/* 店铺组 —— 低频功能，Modal 内查看与新建 */}
      <Modal
        open={groupModalOpen}
        title="店铺组"
        onClose={() => setGroupModalOpen(false)}
      >
        <Callout tone="info">
          店铺组用于给店铺归类，接入店铺时选择所属组，铺货时可按组选择目标店铺。
        </Callout>
        {groups.length === 0 ? (
          <Empty>还没有店铺组，在下方新建一个</Empty>
        ) : (
          <div className="group-list" style={{ marginTop: 14 }}>
            {groups.map((row) => (
              <div className="group-row" key={row.id}>
                <span className={`g-dot ${toneClass(ctx.statusType(row.status))}`} />
                <div style={{ minWidth: 0 }}>
                  <div className="g-name">{row.name}</div>
                  <div className="g-sub">{row.shop_count} 个店铺</div>
                </div>
              </div>
            ))}
          </div>
        )}
        <div className="group-new">
          <input
            className="inp"
            value={ctx.groupForm.name}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              ctx.groupForm.name = e.target.value;
            }}
            placeholder="新店铺组名称"
            onKeyDown={(e: React.KeyboardEvent<HTMLInputElement>) => {
              if (e.key === "Enter") ctx.createGroup();
            }}
          />
          <Button variant="accent" icon="plus" onClick={() => ctx.createGroup()}>
            新建
          </Button>
        </div>
      </Modal>
    </div>
  );
}
