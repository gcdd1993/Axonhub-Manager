import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Activity, ArrowUpRight, CheckCircle2, ChevronRight, CircleAlert,
  Code2, Download, ExternalLink, FileCode2, FolderOpen, Gauge, Github, HardDrive,
  LayoutDashboard, LoaderCircle, Moon, RefreshCw, RotateCcw, Save, Server,
  Settings, ShieldCheck, SquareTerminal, Sun, Workflow,
} from "lucide-react";
import { getSnapshot, readConfig, run, saveConfig } from "./api";
import type { Lifecycle, Snapshot } from "./types";

type View = "overview" | "config" | "logs" | "settings";
const labels: Record<Lifecycle, string> = {
  not_installed: "未安装", stopped: "已停止", starting: "启动中", running: "运行中",
  installing: "安装中", error: "异常",
};

function Status({ snapshot }: { snapshot: Snapshot }) {
  const tone = snapshot.lifecycle === "error" ? "danger" : snapshot.healthy ? "success" : snapshot.busy ? "progress" : "neutral";
  return <span className={`status ${tone}`}><i />{snapshot.lifecycle === "running" && !snapshot.healthy ? "运行中 · 未就绪" : labels[snapshot.lifecycle]}</span>;
}

export default function App() {
  const [view, setView] = useState<View>("overview");
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [pending, setPending] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [config, setConfig] = useState("");
  const [savedConfig, setSavedConfig] = useState("");
  const [theme, setTheme] = useState<"dark" | "light">(() => (localStorage.getItem("axonhub-theme") as "dark" | "light") || "dark");

  const refresh = useCallback(async () => {
    try { setSnapshot(await getSnapshot()); } catch (cause) { setError(String(cause)); }
  }, []);

  useEffect(() => { void refresh(); const timer = window.setInterval(refresh, 2000); return () => clearInterval(timer); }, [refresh]);
  useEffect(() => { document.documentElement.dataset.theme = theme; localStorage.setItem("axonhub-theme", theme); }, [theme]);
  useEffect(() => { if (view === "config") void readConfig().then((value) => { setConfig(value); setSavedConfig(value); }).catch((cause) => setError(String(cause))); }, [view]);

  const action = useCallback(async (command: string) => {
    setPending(command); setError(null);
    try { setSnapshot(await run(command)); } catch (cause) { setError(String(cause)); await refresh(); }
    finally { setPending(null); }
  }, [refresh]);

  const save = useCallback(async () => {
    setPending("save_config"); setError(null);
    try { const next = await saveConfig(config); setSnapshot(next); setSavedConfig(config); }
    catch (cause) { setError(String(cause)); } finally { setPending(null); }
  }, [config]);

  const filteredLogs = useMemo(() => snapshot?.logs.slice(-250).reverse() ?? [], [snapshot]);
  if (!snapshot) return <div className="boot"><LoaderCircle className="spin" />正在连接本地控制平面…</div>;

  const installed = Boolean(snapshot.installed_version);
  const installLabel = !installed ? "安装 AxonHub" : "升级到最新版本";

  return <div className="shell">
    <a className="skip-link" href="#main">跳到主要内容</a>
    <aside className="sidebar">
      <div className="brand"><div className="brand-mark"><Workflow /></div><div><strong>AxonHub</strong><span>MANAGER</span></div></div>
      <nav aria-label="主导航">
        {([
          ["overview", LayoutDashboard, "概览"], ["config", FileCode2, "配置"],
          ["logs", SquareTerminal, "日志"], ["settings", Settings, "设置"],
        ] as const).map(([id, Icon, label]) => <button key={id} className={view === id ? "active" : ""} aria-current={view === id ? "page" : undefined} onClick={() => setView(id)}><Icon /><span>{label}</span></button>)}
      </nav>
      <div className="sidebar-foot"><span className={snapshot.healthy ? "online" : ""} /><div><strong>{snapshot.healthy ? "服务在线" : "服务离线"}</strong><small>127.0.0.1:{snapshot.port}</small></div></div>
    </aside>

    <main id="main" className="main" tabIndex={-1}>
      <header className="topbar"><div><span className="kicker">LOCAL AI CONTROL PLANE</span><h1>{view === "overview" ? "运行概览" : view === "config" ? "AxonHub 配置" : view === "logs" ? "运行日志" : "应用设置"}</h1></div><div className="top-actions">
        <button className="icon-btn" aria-label="切换主题" title="切换主题" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>{theme === "dark" ? <Sun /> : <Moon />}</button>
        <button className="secondary" disabled={Boolean(pending)} onClick={() => action("check_updates")}><RefreshCw className={pending === "check_updates" ? "spin" : ""} />检查版本</button>
        {(!installed || snapshot.update_available) && <button className="primary" disabled={snapshot.busy || Boolean(pending)} onClick={() => action("install_latest")}>{snapshot.busy ? <LoaderCircle className="spin" /> : <Download />}{installLabel}</button>}
      </div></header>

      {error && <div className="error-banner" role="alert"><CircleAlert /><span>{error}</span><button aria-label="关闭" onClick={() => setError(null)}>×</button></div>}

      {view === "overview" && <>
        <section className="hero-card">
          <div className="hero-copy"><div className="hero-icon"><Server /></div><div><div className="eyebrow">AXONHUB INSTANCE</div><h2>{snapshot.healthy ? "网关运行稳定" : installed ? "实例正在由 Manager 托管" : "部署你的本地 AI 网关"}</h2><p>统一管理 OpenAI、Anthropic、Gemini 等协议的本地接入入口。</p></div></div>
          <Status snapshot={snapshot} />
          <div className="hero-actions"><button className="secondary" disabled={!snapshot.healthy} onClick={() => action("open_management")}><ExternalLink />打开管理后台</button><button className="icon-btn" disabled={!snapshot.previous_version || snapshot.busy} title="回滚上一版本" aria-label="回滚上一版本" onClick={() => action("rollback_version")}><RotateCcw /></button></div>
        </section>

        {snapshot.busy && <div className="progress-card"><div><span>{snapshot.progress_label ?? "处理中"}</span><strong>{snapshot.progress_percent ?? 0}%</strong></div><div className="progress-track"><i style={{ width: `${snapshot.progress_percent ?? 3}%` }} /></div></div>}

        <section className="metric-grid">
          <article><div className="metric-icon green"><Activity /></div><span>健康状态</span><strong>{snapshot.healthy ? "Healthy" : "Offline"}</strong><small>{snapshot.uptime ? `已运行 ${snapshot.uptime}` : "等待服务响应"}</small></article>
          <article><div className="metric-icon blue"><Code2 /></div><span>当前版本</span><strong>{snapshot.installed_version ?? "—"}</strong><small>{snapshot.update_available ? `可升级至 ${snapshot.latest_version}` : "当前已是最新"}</small></article>
          <article><div className="metric-icon violet"><Gauge /></div><span>监听端口</span><strong>{snapshot.port}</strong><small>{snapshot.pid ? `PID ${snapshot.pid}` : "无活动进程"}</small></article>
          <article><div className="metric-icon amber"><ShieldCheck /></div><span>访问范围</span><strong>仅本机</strong><small>默认安全配置</small></article>
        </section>

        <section className="split-grid">
          <article className="panel quick"><div className="panel-head"><div><span className="kicker">QUICK ACCESS</span><h3>常用入口</h3></div></div>
            <button disabled={!snapshot.healthy} onClick={() => action("open_management")}><span className="action-icon"><LayoutDashboard /></span><div><strong>AxonHub 管理后台</strong><small>渠道、模型、API Key 与请求追踪</small></div><ChevronRight /></button>
            <button onClick={() => setView("config")}><span className="action-icon"><FileCode2 /></span><div><strong>编辑运行配置</strong><small>端口、数据库、日志与缓存</small></div><ChevronRight /></button>
            <button onClick={() => action("open_data_directory")}><span className="action-icon"><FolderOpen /></span><div><strong>打开数据目录</strong><small>数据库、配置、版本和日志</small></div><ChevronRight /></button>
          </article>
          <article className="panel release"><div className="panel-head"><div><span className="kicker">RELEASE CHANNEL</span><h3>版本状态</h3></div><Github /></div>
            <div className="version-line"><span>已安装</span><strong>{snapshot.installed_version ?? "尚未安装"}</strong></div>
            <div className="version-line"><span>上游最新</span><strong>{snapshot.latest_version ?? "尚未检查"}</strong></div>
            <div className="version-line"><span>可回滚版本</span><strong>{snapshot.previous_version ?? "无"}</strong></div>
            <a href="https://github.com/looplj/axonhub/releases" target="_blank" rel="noreferrer">查看 AxonHub Releases <ArrowUpRight /></a>
          </article>
        </section>
      </>}

      {view === "config" && <section className="config-layout">
        <div className="config-toolbar"><div><h2>config.yml</h2><p>保存前会调用 AxonHub 自带的 <code>config validate</code> 校验。</p></div><div><button className="secondary" disabled={!installed || Boolean(pending)} onClick={() => action("validate_config")}><CheckCircle2 />验证配置</button><button className="primary" disabled={config === savedConfig || Boolean(pending)} onClick={save}><Save />保存配置</button></div></div>
        <div className="editor-shell"><div className="editor-head"><span>{snapshot.config_path}</span><span className={config === savedConfig ? "saved" : "changed"}>{config === savedConfig ? "已保存" : "有未保存更改"}</span></div><textarea aria-label="AxonHub YAML 配置" spellCheck={false} value={config} onChange={(event) => setConfig(event.target.value)} /></div>
      </section>}

      {view === "logs" && <section className="logs-panel"><div className="terminal-head"><div><i /><i /><i /></div><span>axonhub.log</span><button onClick={() => action("open_log_directory")}><FolderOpen />打开目录</button></div><div className="terminal-body">{filteredLogs.length ? filteredLogs.map((entry, index) => <div className={`log-row ${entry.level}`} key={`${entry.timestamp}-${index}`}><time>{new Date(entry.timestamp).toLocaleTimeString("zh-CN", { hour12: false })}</time><span>{entry.level.toUpperCase()}</span><p>{entry.message}</p></div>) : <div className="empty">暂无日志。启动 AxonHub 后，进程输出会显示在这里。</div>}</div></section>}

      {view === "settings" && <section className="settings-grid">
        <article><div className="setting-title"><HardDrive /><div><strong>数据目录</strong><span>托管版本、SQLite 数据库、配置与日志</span></div></div><code>{snapshot.data_directory}</code><button className="secondary" onClick={() => action("open_data_directory")}><FolderOpen />打开目录</button></article>
        <article><div className="setting-title"><Server /><div><strong>运行端点</strong><span>桌面管理器连接的本地 AxonHub 地址</span></div></div><code>{snapshot.management_url}</code><button className="secondary" disabled={!snapshot.healthy} onClick={() => action("open_management")}><ExternalLink />打开页面</button></article>
        <article><div className="setting-title"><Github /><div><strong>上游项目</strong><span>Release 资产来自 looplj/axonhub</span></div></div><code>github.com/looplj/axonhub</code><a className="secondary link-btn" href="https://github.com/looplj/axonhub" target="_blank" rel="noreferrer"><Github />查看源码</a></article>
      </section>}

      <footer><span>AxonHub Manager · Tauri + Rust</span><span>{snapshot.platform} / {snapshot.architecture}</span></footer>
    </main>
  </div>;
}
