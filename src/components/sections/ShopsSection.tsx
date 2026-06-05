/* ============================================================================
   店铺与密钥管理 —— Soft 设计移植
   功能：(1) 店铺组管理，(2) 接入新店铺，(3) 已接入店铺总览 + 逐店操作
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill, Select } from "../primitives";

export default function ShopsSection() {
  const ctx = useApp();

  const groups = ctx.groups.value;
  const shops = ctx.shops.value;

  return (
    <section className="stack">
      {/* Panel 1 — 店铺组 */}
      <div className="panel">
        <div className="ph">
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <h3>店铺组</h3>
            <Pill tone="info">{groups.length} 组</Pill>
          </div>
        </div>

        {/* 新建店铺组 inline 表单 */}
        <div className="toolbar" style={{ marginBottom: 12 }}>
          <input
            className="inp"
            value={ctx.groupForm.name}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              ctx.groupForm.name = e.target.value;
            }}
            placeholder="例如：默认铺货组"
            style={{ flex: 1, maxWidth: 320 }}
          />
          <Button variant="accent" icon="plus" onClick={() => ctx.createGroup()}>
            新建店铺组
          </Button>
        </div>

        {/* 店铺组表格 */}
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th>店铺组</th>
                <th style={{ width: 100 }}>店铺数</th>
                <th style={{ width: 120 }}>状态</th>
                <th style={{ minWidth: 220 }}>ID</th>
              </tr>
            </thead>
            <tbody>
              {groups.length === 0 ? (
                <tr>
                  <td colSpan={4}>
                    <div className="empty">暂无数据</div>
                  </td>
                </tr>
              ) : (
                groups.map((row) => (
                  <tr key={row.id}>
                    <td>{row.name}</td>
                    <td>{row.shop_count}</td>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>{row.status}</Pill>
                    </td>
                    <td className="mono">{row.id}</td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>

      {/* Panel 2 — 接入店铺 */}
      <div className="panel">
        <div className="ph">
          <div>
            <h3>接入店铺</h3>
            <p>保存时 app_secret 只发送到 Tauri 后端加密入库，前端不展示、不回填。</p>
          </div>
        </div>

        <div className="form-grid cols-3" style={{ marginBottom: 12 }}>
          <input
            className="inp"
            value={ctx.shopForm.name}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              ctx.shopForm.name = e.target.value;
            }}
            placeholder="店铺名称"
          />
          <input
            className="inp"
            value={ctx.shopForm.appid}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              ctx.shopForm.appid = e.target.value;
            }}
            placeholder="微信小店 appid"
          />
          <input
            className="inp"
            type="password"
            value={ctx.shopForm.app_secret}
            onChange={(e: React.ChangeEvent<HTMLInputElement>) => {
              ctx.shopForm.app_secret = e.target.value;
            }}
            placeholder="微信小店 app_secret"
            autoComplete="new-password"
          />
          <Select
            value={ctx.shopForm.group_id}
            onChange={(v: string) => {
              ctx.shopForm.group_id = v;
            }}
            options={ctx.selectedGroupOptions.value}
            placeholder="选择店铺组"
          />
          <div className="form-actions" style={{ gridColumn: "span 3" }}>
            <Button variant="accent" icon="plus" onClick={() => ctx.createShop()}>
              保存店铺
            </Button>
          </div>
        </div>
      </div>

      {/* Panel 3 — 店铺接入状态 */}
      <div className="panel">
        <div className="ph">
          <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
            <h3>店铺接入状态</h3>
            <Pill tone="info">{shops.length} 店</Pill>
          </div>
        </div>

        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ minWidth: 150 }}>店铺</th>
                <th style={{ minWidth: 170 }}>appid</th>
                <th style={{ minWidth: 130 }}>店铺组</th>
                <th style={{ minWidth: 170 }}>微信资料</th>
                <th style={{ width: 130 }}>状态</th>
                <th style={{ width: 100 }}>密钥</th>
                <th style={{ minWidth: 190 }}>token 到期</th>
                <th style={{ width: 110 }}>接口额度</th>
                <th style={{ width: 250 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {shops.length === 0 ? (
                <tr>
                  <td colSpan={9}>
                    <div className="empty">暂无数据</div>
                  </td>
                </tr>
              ) : (
                shops.map((row) => (
                  <tr key={row.id}>
                    <td>{row.name}</td>
                    <td className="mono">{row.appid}</td>
                    <td>{row.group_name}</td>
                    <td>
                      <div className="cell-main">
                        <strong>{row.wechat_nickname || "-"}</strong>
                        <span className="subtext">{row.wechat_status || "未同步"}</span>
                      </div>
                    </td>
                    <td>
                      <Pill tone={ctx.statusType(row.status)}>{row.status}</Pill>
                    </td>
                    <td>
                      <Pill tone={row.has_secret ? "success" : "warning"}>
                        {row.has_secret ? "已保存" : "缺失"}
                      </Pill>
                    </td>
                    <td className="mono">{ctx.formatDateTime(row.token_expires_at)}</td>
                    <td>{row.last_quota_remain ?? "-"}</td>
                    <td>
                      <div className="row-actions">
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
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </section>
  );
}
