/* ============================================================================
   数据备份 —— 列出已有备份、创建新备份、定位文件、恢复数据库
   忠实保留原 BackupSection.vue 的 IA/内容/中文文案，套用 Soft 视觉。
   ============================================================================ */
import { useApp } from "../../runtime/AppContext";
import { Button, Pill } from "../primitives";
import type { BackupInfo } from "../../types/app";

export default function BackupSection() {
  const ctx = useApp();

  const backups = ctx.databaseBackups.value;
  const running = ctx.backupRunning.value;
  const dbPath = ctx.dashboard.value?.database_path || "-";
  const latestBackupTime = backups[0]?.created_at
    ? ctx.formatDateTime(backups[0].created_at)
    : "尚未备份";

  return (
    <section className="stack">
      <div className="panel">
        {/* 标题区：左侧文案 + 右侧按钮组 */}
        <div className="ph" style={{ alignItems: "flex-start" }}>
          <div>
            <h3>数据备份</h3>
            <p>
              备份保存到应用数据目录，创建和恢复都会做 SQLite 完整性校验；恢复前会自动生成回滚备份。
            </p>
          </div>
          <div style={{ display: "flex", gap: 8, flexShrink: 0, marginTop: 2 }}>
            <Button
              variant="outline"
              icon="refresh"
              size="sm"
              onClick={() => ctx.refreshDatabaseBackups()}
            >
              刷新
            </Button>
            <Button
              variant="accent"
              icon="upload"
              size="sm"
              disabled={running}
              onClick={() => ctx.createDatabaseBackup()}
            >
              {running ? "备份中…" : "创建备份"}
            </Button>
          </div>
        </div>

        {/* KPI 状态条 */}
        <div className="kv-grid" style={{ marginBottom: 16 }}>
          <div className="kv">
            <dt>当前数据库</dt>
            <dd className="mono" style={{ wordBreak: "break-all" }}>
              {dbPath}
            </dd>
          </div>
          <div className="kv">
            <dt>备份数量</dt>
            <dd>{backups.length}</dd>
          </div>
          <div className="kv">
            <dt>最近备份</dt>
            <dd>{latestBackupTime}</dd>
          </div>
        </div>

        {/* 备份列表表格 */}
        <div className="tbl-wrap">
          <table className="tbl">
            <thead>
              <tr>
                <th style={{ minWidth: 270 }}>备份文件</th>
                <th style={{ width: 110 }}>大小</th>
                <th style={{ minWidth: 190 }}>创建时间</th>
                <th style={{ width: 110 }}>校验</th>
                <th style={{ minWidth: 180 }}>SHA-256</th>
                <th style={{ minWidth: 150 }}>校验信息</th>
                <th style={{ width: 170 }}>操作</th>
              </tr>
            </thead>
            <tbody>
              {backups.length === 0 ? (
                <tr>
                  <td colSpan={7}>
                    <div className="empty">暂无数据</div>
                  </td>
                </tr>
              ) : (
                backups.map((row: BackupInfo) => (
                  <tr key={row.id}>
                    {/* 备份文件名：截断 + title tooltip */}
                    <td
                      style={{
                        maxWidth: 270,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.file_name}
                    >
                      {row.file_name}
                    </td>
                    {/* 大小 */}
                    <td>{ctx.formatBytes(row.size_bytes)}</td>
                    {/* 创建时间 */}
                    <td>{ctx.formatDateTime(row.created_at)}</td>
                    {/* 校验状态 */}
                    <td>
                      <Pill tone={row.integrity_ok ? "success" : "danger"}>
                        {row.integrity_ok ? "通过" : "失败"}
                      </Pill>
                    </td>
                    {/* SHA-256：截断 + title tooltip */}
                    <td
                      className="mono"
                      style={{
                        maxWidth: 180,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.sha256}
                    >
                      {row.sha256}
                    </td>
                    {/* 校验信息：截断 + title tooltip */}
                    <td
                      style={{
                        maxWidth: 150,
                        overflow: "hidden",
                        textOverflow: "ellipsis",
                        whiteSpace: "nowrap",
                      }}
                      title={row.integrity_message}
                    >
                      {row.integrity_message}
                    </td>
                    {/* 操作列 */}
                    <td>
                      <div className="row-actions">
                        <Button
                          size="sm"
                          variant="outline"
                          onClick={() => ctx.revealBackup(row)}
                        >
                          定位
                        </Button>
                        <Button
                          size="sm"
                          variant="graphite"
                          disabled={!row.integrity_ok || running}
                          onClick={() => ctx.restoreDatabaseBackup(row)}
                        >
                          恢复
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
