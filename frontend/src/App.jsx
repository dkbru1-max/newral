import { useCallback, useEffect, useMemo, useState } from "react";
import {
  BrowserRouter,
  Link,
  Route,
  Routes,
  useLocation,
  useNavigate,
  useParams
} from "react-router-dom";

const baseKpis = [
  { label: "Connected Agents", trend: "live" },
  { label: "Tasks Completed", trend: "live" },
  { label: "Queue Depth", trend: "live" },
  { label: "AI Mode", trend: "policy gated" },
  { label: "Storage Usage", value: "42%", trend: "object store" }
];

const emptyDashboardSnapshot = {
  tasks_last_24h: [],
  tasks_total_24h: 0,
  agent_availability: { online: 0, idle: 0, blocked: 0 },
  storage_io: { disk_read_mb: 0, disk_write_mb: 0, net_rx_mb: 0, net_tx_mb: 0 },
  throughput: { completed_last_min: 0, completed_last_hour: 0 },
  trust: { blocked_agents: 0, total_agents: 0 }
};

const navItems = [
  { id: "dashboard", title: "Dashboard", anchor: "dashboard" },
  { id: "demo", title: "Demo", anchor: "demo" },
  { id: "agents", title: "Agents", anchor: "agents" },
  { id: "tasks", title: "Tasks", anchor: "tasks" },
  { id: "projects", title: "Projects", anchor: "projects" },
  { id: "ai-mode", title: "AI Mode", anchor: "ai-mode" },
  { id: "log-center", title: "Log Center", anchor: "log-center" },
  { id: "settings", title: "Settings", anchor: "settings" },
  { id: "integrations", title: "Integrations", anchor: "integrations" },
  { id: "system-health", title: "System Health", anchor: "system-health" }
];

const formatGb = (valueGb, valueMb) => {
  if (typeof valueGb === "number" && !Number.isNaN(valueGb)) {
    return `${valueGb.toFixed(1)} GB`;
  }
  if (typeof valueMb === "number" && !Number.isNaN(valueMb)) {
    return `${(valueMb / 1024).toFixed(1)} GB`;
  }
  return "—";
};

const initialAgents = [];

const initialTasks = [
  { id: "task-1001", status: "queued", priority: "high" },
  { id: "task-1002", status: "running", priority: "normal" },
  { id: "task-1003", status: "waiting", priority: "low" }
];

const seedProjects = [
  { id: null, name: "Prime Search", status: "active", owner: "Research", progress: 68 },
  { id: null, name: "LLM Fine-tuning", status: "staged", owner: "AI Lab", progress: 22 },
  { id: null, name: "Bio Sim", status: "draft", owner: "BioTech", progress: 9 }
];

const logSeed = [
  {
    id: "log-01",
    level: "info",
    source: "scheduler",
    message: "Live summary broadcast to portal subscribers",
    time: "2m ago"
  },
  {
    id: "log-02",
    level: "success",
    source: "validator",
    message: "Recheck completed, result verified",
    time: "6m ago"
  },
  {
    id: "log-03",
    level: "warn",
    source: "agent",
    message: "Agent node-27 heartbeat delayed",
    time: "10m ago"
  },
  {
    id: "log-04",
    level: "error",
    source: "sandbox",
    message: "Workspace size limit exceeded for task-8912",
    time: "18m ago"
  }
];

const healthTargets = [
  { id: "identity", name: "Identity Service", base: "/api/identity" },
  { id: "scheduler", name: "Scheduler Service", base: "/api/scheduler" },
  { id: "validator", name: "Validator Service", base: "/api/validator" },
  { id: "telemetry", name: "Telemetry Service", base: "/api/telemetry" }
];

const integrationTargets = [
  { name: "Kafka", status: "connected", note: "events + orchestration" },
  { name: "Postgres", status: "connected", note: "task state + audit" },
  { name: "MinIO", status: "connected", note: "object storage" },
  { name: "Slack", status: "pending", note: "alerts" }
];

const bpswTaskTypes = [
  "main_odds",
  "large_numbers",
  "chernick",
  "pomerance_lite",
  "pomerance_modular",
  "lambda_plus_one"
];

async function readApiError(response) {
  try {
    const text = await response.text();
    if (!text) {
      return "";
    }
    try {
      const json = JSON.parse(text);
      return json?.message || json?.error || "";
    } catch (error) {
      return text;
    }
  } catch (error) {
    return "";
  }
}

const projectPath = (name) => `/projects/${encodeURIComponent(name)}`;

function Breadcrumbs({ liveAgents, liveTasks, projects }) {
  const location = useLocation();
  const segments = location.pathname.split("/").filter(Boolean);
  const breadcrumbs = [
    { label: "Dashboard", path: "/" },
    ...segments.map((segment, index) => {
      const path = `/${segments.slice(0, index + 1).join("/")}`;
      const decoded = decodeURIComponent(segment);
      let label = decoded;
      if (segments[0] === "agents" && index === 1) {
        label =
          liveAgents.find((agent) => agent.id === decoded)?.id || decoded;
      }
      if (segments[0] === "tasks" && index === 1) {
        label = liveTasks.find((task) => task.id === decoded)?.id || decoded;
      }
      if (segments[0] === "projects" && index === 1) {
        label =
          projects.find((project) => project.name === decoded)?.name || decoded;
      }
      return {
        label: label.replace(/-/g, " "),
        path
      };
    })
  ];

  return (
    <div className="breadcrumbs">
      {breadcrumbs.map((crumb, index) => (
        <span key={crumb.path}>
          <Link to={crumb.path}>{crumb.label}</Link>
          {index < breadcrumbs.length - 1 && <span> / </span>}
        </span>
      ))}
    </div>
  );
}

function Layout({
  children,
  liveAgents,
  liveTasks,
  projects,
  theme,
  onToggleTheme,
  onCreateProject,
  createProjectState,
  portalVersion,
  activeSection,
  onNavClick
}) {
  const navigate = useNavigate();
  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img src="/newral_big_logo.png" alt="Newral" className="logo" />
          <div className="brand-copy">
            <p className="eyebrow">
              Newral Portal{portalVersion ? ` v ${portalVersion}` : ""}
            </p>
            <h1>Admin Console</h1>
          </div>
        </div>
        <nav className="nav-links">
          {navItems.map((item) => (
            <a
              key={item.id}
              href={`/#${item.anchor}`}
              className={activeSection === item.anchor ? "active" : undefined}
              onClick={() => onNavClick(item.anchor)}
            >
              {item.title}
            </a>
          ))}
        </nav>
      </aside>

      <div className="main">
        <header className="topbar">
          <div>
            <Breadcrumbs liveAgents={liveAgents} liveTasks={liveTasks} projects={projects} />
            <h2>Distributed compute, policy-first.</h2>
          </div>
          <div className="topbar-actions">
            {portalVersion && <span className="version-chip">v{portalVersion}</span>}
            <button className={`btn primary create-btn ${createProjectState}`} onClick={onCreateProject}>
              {createProjectState === "loading"
                ? "Creating..."
                : createProjectState === "success"
                ? "Created"
                : "Create project"}
            </button>
            <button className="btn ghost" onClick={onToggleTheme}>
              Theme: {theme === "dark" ? "Dark" : "Light"}
            </button>
            <button className="btn ghost" onClick={() => navigate(-1)}>
              Back
            </button>
          </div>
        </header>
        <main className="content">{children}</main>
        <footer className="footer">
          <span>Newral Portal{portalVersion ? ` v${portalVersion}` : ""}</span>
          <span>Admin Console</span>
        </footer>
      </div>
    </div>
  );
}

function DashboardPage({
  kpis,
  summaryError,
  summaryLoaded,
  liveLoad,
  dashboard,
  sectionId
}) {
  const taskBuckets = dashboard?.tasks_last_24h ?? [];
  const resolvedBuckets =
    taskBuckets.length > 0
      ? taskBuckets
      : Array.from({ length: 7 }, (_, idx) => ({
          label: `-${24 - idx * 4}h`,
          value: 0
        }));
  const maxBucket = Math.max(1, ...resolvedBuckets.map((bucket) => bucket.value));
  const availability = dashboard?.agent_availability ?? emptyDashboardSnapshot.agent_availability;
  const totalAgents = availability.online + availability.idle + availability.blocked;
  const storage = dashboard?.storage_io ?? emptyDashboardSnapshot.storage_io;
  const storageTotal = Math.max(
    1,
    storage.disk_read_mb + storage.disk_write_mb + storage.net_rx_mb + storage.net_tx_mb
  );
  const trust = dashboard?.trust ?? emptyDashboardSnapshot.trust;
  const throughput =
    dashboard?.throughput ?? emptyDashboardSnapshot.throughput;
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Dashboard</h2>
          <p>Snapshot of orchestration, trust, and throughput.</p>
        </div>
        <div className="pill">Auto-refresh on</div>
      </div>
      {summaryError && <p className="error">{summaryError}</p>}
      <div className="metric-grid">
        {summaryLoaded
          ? kpis.map((metric) => (
              <div key={metric.label} className="metric-card">
                <span>{metric.label}</span>
                <strong>{metric.value}</strong>
                <em>{metric.trend}</em>
              </div>
            ))
          : kpis.map((metric) => (
              <div key={metric.label} className="metric-card skeleton-card">
                <span className="skeleton-line"></span>
                <strong className="skeleton-line"></strong>
                <em className="skeleton-line"></em>
              </div>
            ))}
      </div>
      <div className="chart-grid">
        <div className="chart-card">
          <div className="chart-header">
            <h3>Tasks (last 24h)</h3>
            <span>{dashboard?.tasks_total_24h ?? 0} completed</span>
          </div>
          <div className="chart-bars">
            {resolvedBuckets.map((bucket) => (
              <div key={bucket.label} className="chart-bar">
                <div
                  className="chart-bar-fill"
                  style={{
                    height: `${Math.max(
                      0,
                      Math.min(
                        100,
                        maxBucket ? (bucket.value / maxBucket) * 100 : 0
                      )
                    )}%`
                  }}
                ></div>
                <span>{bucket.label}</span>
              </div>
            ))}
          </div>
        </div>
        <div className="chart-card">
          <div className="chart-header">
            <h3>Agent availability</h3>
            <span>{totalAgents} total</span>
          </div>
          <div className="chart-meter">
            <div
              className="chart-meter-bar online"
              style={{
                width: `${totalAgents ? (availability.online / totalAgents) * 100 : 0}%`
              }}
            ></div>
            <div
              className="chart-meter-bar idle"
              style={{
                width: `${totalAgents ? (availability.idle / totalAgents) * 100 : 0}%`
              }}
            ></div>
            <div
              className="chart-meter-bar blocked"
              style={{
                width: `${totalAgents ? (availability.blocked / totalAgents) * 100 : 0}%`
              }}
            ></div>
          </div>
          <div className="chart-legend">
            <span>Online: {availability.online}</span>
            <span>Idle: {availability.idle}</span>
            <span>Blocked: {availability.blocked}</span>
          </div>
        </div>
        <div className="chart-card">
          <div className="chart-header">
            <h3>Storage IO</h3>
            <span>MB totals</span>
          </div>
          <div className="chart-stack">
            <div>
              <span>Disk read</span>
              <div className="chart-meter">
                <div
                  className="chart-meter-bar primary"
                  style={{ width: `${(storage.disk_read_mb / storageTotal) * 100}%` }}
                ></div>
              </div>
            </div>
            <div>
              <span>Disk write</span>
              <div className="chart-meter">
                <div
                  className="chart-meter-bar accent"
                  style={{ width: `${(storage.disk_write_mb / storageTotal) * 100}%` }}
                ></div>
              </div>
            </div>
            <div>
              <span>Net RX</span>
              <div className="chart-meter">
                <div
                  className="chart-meter-bar neutral"
                  style={{ width: `${(storage.net_rx_mb / storageTotal) * 100}%` }}
                ></div>
              </div>
            </div>
            <div>
              <span>Net TX</span>
              <div className="chart-meter">
                <div
                  className="chart-meter-bar neutral"
                  style={{ width: `${(storage.net_tx_mb / storageTotal) * 100}%` }}
                ></div>
              </div>
            </div>
          </div>
          <div className="chart-legend">
            <span>{storage.disk_read_mb.toFixed(1)} MB read</span>
            <span>{storage.disk_write_mb.toFixed(1)} MB write</span>
          </div>
        </div>
        <div className="chart-card">
          <div className="chart-header">
            <h3>Throughput</h3>
            <span>live</span>
          </div>
          <div className="chart-stat">
            <div>
              <strong>{throughput.completed_last_min}</strong>
              <span>completed / min</span>
            </div>
            <div>
              <strong>{throughput.completed_last_hour}</strong>
              <span>completed / hour</span>
            </div>
          </div>
        </div>
        <div className="chart-card">
          <div className="chart-header">
            <h3>Trust</h3>
            <span>policy</span>
          </div>
          <div className="chart-stat">
            <div>
              <strong>{trust.blocked_agents}</strong>
              <span>blocked agents</span>
            </div>
            <div>
              <strong>{trust.total_agents}</strong>
              <span>total agents</span>
            </div>
          </div>
        </div>
      </div>
      <div className="vision-card">
        <h3>Roadmap / Vision</h3>
        <ul>
          <li>Security-first compute with continuous verification.</li>
          <li>Multi-region orchestration with policy guardrails.</li>
          <li>AI-guided scheduling with deterministic overrides.</li>
        </ul>
      </div>
      <div className="monitor-grid">
        <div className="monitor-card">
          <h3>Metrics preview</h3>
          <div className="bar-list">
            <div>
              <span>Task latency</span>
              <div className="bar">
                <div style={{ width: "62%" }}></div>
              </div>
            </div>
            <div>
              <span>Verification</span>
              <div className="bar">
                <div style={{ width: "48%" }}></div>
              </div>
            </div>
            <div>
              <span>Network load</span>
              <div className="bar">
                <div style={{ width: `${Math.min(100, ((liveLoad.running + liveLoad.queued) / 20) * 100)}%` }}></div>
              </div>
            </div>
          </div>
          <p className="muted">
            Running: {liveLoad.running} · Queued: {liveLoad.queued} · Throughput/min: {liveLoad.completed_last_min}
          </p>
        </div>
      </div>
    </section>
  );
}

function DemoPage({
  demoStatus,
  demoResult,
  demoLoading,
  demoError,
  onStartDemo,
  sectionId
}) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Demo Project</h2>
          <p>Launch the wordcount demo and track its progress.</p>
        </div>
        <div className="chip">Sandboxed Python</div>
      </div>
      <div className="demo-card">
        <div>
          <h3>Wordcount pipeline</h3>
          <p className="muted">Python sandbox tasks split into 5 parts with aggregation.</p>
        </div>
        <button className="btn primary" onClick={onStartDemo} disabled={demoLoading}>
          {demoLoading ? "Starting..." : "Start Demo"}
        </button>
      </div>
      {demoError && <p className="error">{demoError}</p>}
      <div className="progress-card">
        <div className="progress-header">
          <span>Progress</span>
          <strong>
            {demoStatus.completed}/{demoStatus.total}
          </strong>
        </div>
        <div className="progress-bar">
          <div
            style={{
              width:
                demoStatus.total > 0
                  ? `${Math.min(100, (demoStatus.completed / demoStatus.total) * 100)}%`
                  : "0%"
            }}
          ></div>
        </div>
      </div>
      <div className="table-card">
        <table>
          <thead>
            <tr>
              <th>Top word</th>
              <th>Count</th>
            </tr>
          </thead>
          <tbody>
            {demoResult.length === 0 ? (
              <tr>
                <td colSpan="2" className="muted">
                  Run the demo to see results.
                </td>
              </tr>
            ) : (
              demoResult.map((entry) => (
                <tr key={entry.word}>
                  <td>{entry.word}</td>
                  <td>{entry.count}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function AgentsPage({ agents, summaryLoaded, sectionId }) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Agents</h2>
          <p>Live status and hardware inventory for connected nodes.</p>
        </div>
        <div className="chip">Auto heartbeat</div>
      </div>
      <div className="table-card table-scroll">
        <table>
          <thead>
            <tr>
              <th>Node</th>
              <th>Status</th>
              <th>CPU</th>
              <th>RAM</th>
              <th>GPU</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody>
            {summaryLoaded && agents.length === 0 ? (
              <tr>
                <td colSpan="6" className="muted">
                  No agents connected.
                </td>
              </tr>
            ) : (
              agents.map((agent) => (
                <tr key={agent.id}>
                  <td>{agent.id}</td>
                  <td>
                    <span className={`status-dot ${agent.status}`} title={agent.status}></span>
                    <span className={`status-label ${agent.status}`}>{agent.status}</span>
                  </td>
                  <td>{agent.hardware?.cpu_model ?? "—"}</td>
                  <td>
                    {formatGb(
                      agent.hardware?.ram_total_gb,
                      agent.hardware?.ram_total_mb
                    )}
                  </td>
                  <td>{agent.hardware?.gpu_model ?? "—"}</td>
                  <td>
                    <Link to={`/agents/${agent.id}`} className="link">View</Link>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function AgentDetailPage({ agents }) {
  const { agentId } = useParams();
  const agent = agents.find((entry) => entry.id === agentId);
  if (!agent) {
    return <p className="error">Agent not found.</p>;
  }
  return (
    <section className="section">
      <div className="section-header">
        <div>
          <h2>Agent {agent.id}</h2>
          <p>Hardware snapshot, live metrics, and trust status.</p>
        </div>
        <div className="chip">Details</div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Status</h3>
          <p className="muted">{agent.status}</p>
        </div>
        <div className="stat-card">
          <h3>Last seen</h3>
          <p className="muted">{agent.last_seen ?? "unknown"}</p>
        </div>
        <div className="stat-card">
          <h3>Blocked</h3>
          <p className="muted">{agent.blocked ? "yes" : "no"}</p>
        </div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Hardware</h3>
          <p className="muted">
            CPU: {agent.hardware?.cpu_model ?? "—"} · RAM:{" "}
            {formatGb(
              agent.hardware?.ram_total_gb,
              agent.hardware?.ram_total_mb
            )}
          </p>
          <p className="muted">
            GPU: {agent.hardware?.gpu_model ?? "—"} · Disk:{" "}
            {formatGb(
              agent.hardware?.disk_total_gb,
              agent.hardware?.disk_total_mb
            )}
          </p>
        </div>
        <div className="stat-card">
          <h3>Recent metrics</h3>
          <p className="muted">
            CPU {agent.metrics?.cpu_load?.toFixed(1) ?? "—"}% · RAM{" "}
            {formatGb(
              agent.metrics?.ram_used_mb
                ? agent.metrics.ram_used_mb / 1024
                : null,
              null
            )}{" "}
            · Net {agent.metrics?.net_rx_bytes ?? "—"} B
          </p>
        </div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Actions</h3>
          <div className="pill-row">
            <button className="btn ghost">Block agent</button>
            <button className="btn ghost">Message</button>
          </div>
        </div>
      </div>
    </section>
  );
}

function TasksPage({
  tasks,
  summaryLoaded,
  taskQuery,
  taskStatus,
  onQueryChange,
  onStatusChange,
  visibleTasks,
  onLoadMore,
  sectionId
}) {
  const filteredTasks = tasks.filter((task) => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    const matchesQuery =
      !normalizedQuery ||
      task.id.toLowerCase().includes(normalizedQuery) ||
      task.project.toLowerCase().includes(normalizedQuery);
    const matchesStatus = taskStatus === "all" ? true : task.status === taskStatus;
    return matchesQuery && matchesStatus;
  });

  const visibleRows = filteredTasks.slice(0, visibleTasks);

  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Tasks</h2>
          <p>Queue preview with filters and live status.</p>
        </div>
        <div className="filter-row">
          <input
            className="input"
            placeholder="Search by task or project"
            value={taskQuery}
            onChange={(event) => onQueryChange(event.target.value)}
          />
          <select
            className="input"
            value={taskStatus}
            onChange={(event) => onStatusChange(event.target.value)}
          >
            <option value="all">All</option>
            <option value="queued">Queued</option>
            <option value="running">Running</option>
            <option value="done">Done</option>
            <option value="waiting">Waiting</option>
          </select>
        </div>
      </div>
      <div className="table-card table-scroll">
        <table>
          <thead>
            <tr>
              <th>Task</th>
              <th>Project</th>
              <th>Agent</th>
              <th>Status</th>
              <th>Priority</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody>
            {summaryLoaded && visibleRows.length === 0 ? (
              <tr>
                <td colSpan="6" className="muted">
                  No tasks matching the filter.
                </td>
              </tr>
            ) : (
              visibleRows.map((task) => (
                <tr key={task.id}>
                  <td>{task.id}</td>
                  <td>{task.project}</td>
                  <td>{task.agent}</td>
                  <td>
                    <span className={`status-label ${task.status}`}>{task.status}</span>
                  </td>
                  <td>{task.priority}</td>
                  <td>
                  <Link to={`/tasks/${task.id}`} className="link">View</Link>
                </td>
              </tr>
            ))
          )}
          </tbody>
        </table>
      </div>
      {filteredTasks.length > visibleRows.length && (
        <button className="btn ghost" onClick={onLoadMore}>
          Load more
        </button>
      )}
    </section>
  );
}

function TaskDetailPage({ tasks }) {
  const { taskId } = useParams();
  const task = tasks.find((entry) => entry.id === taskId);
  if (!task) {
    return <p className="error">Task not found.</p>;
  }
  return (
    <section className="section">
      <div className="section-header">
        <div>
          <h2>{task.id}</h2>
          <p>Task details, status, and verification trail.</p>
        </div>
        <div className="chip">{task.status}</div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Project</h3>
          <p className="muted">{task.project}</p>
        </div>
        <div className="stat-card">
          <h3>Agent</h3>
          <p className="muted">{task.agent}</p>
        </div>
        <div className="stat-card">
          <h3>Priority</h3>
          <p className="muted">{task.priority}</p>
        </div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Result</h3>
          <p className="muted">Awaiting validation.</p>
        </div>
        <div className="stat-card">
          <h3>History</h3>
          <p className="muted">Issued 5m ago, running on {task.agent}.</p>
        </div>
      </div>
    </section>
  );
}

function ProjectsPage({
  onCreateProject,
  bpswForm,
  onUpdateBpswForm,
  onToggleBpswType,
  onStartBpsw,
  onProjectAction,
  onSyncBpsw,
  bpswState,
  projectActions,
  sectionId,
  projects
}) {
  const bpswProject = projects.find((project) => project.name === "bpsw_hunter");
  const bpswStatus = bpswProject?.status ?? "inactive";
  const bpswActionState = bpswProject?.id ? projectActions[bpswProject.id] ?? {} : {};
  const bpswIsActive = bpswStatus === "active";
  const bpswActionLoading = Boolean(bpswActionState.loading);
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Projects</h2>
          <p>Portfolio view for active and staged workloads.</p>
        </div>
        <button className="btn ghost" onClick={onCreateProject}>
          Create project
        </button>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>BPSW Hunter</h3>
          <p className="muted">Launch distributed search tasks from the portal.</p>
          <div className="pill-row">
            <span className="pill">{bpswStatus}</span>
            <span className="pill">
              Project ID: {bpswProject?.id ?? "—"}
            </span>
            {bpswActionState.info && <span className="pill">{bpswActionState.info}</span>}
          </div>
          <div className="form-grid">
            <label className="field">
              <span>Start (optional)</span>
              <input
                className="input"
                value={bpswForm.start}
                placeholder="100000000000000000001"
                onChange={(event) =>
                  onUpdateBpswForm({ start: event.target.value })
                }
              />
            </label>
            <label className="field">
              <span>End (optional)</span>
              <input
                className="input"
                value={bpswForm.end}
                placeholder="100000000000000000501"
                onChange={(event) =>
                  onUpdateBpswForm({ end: event.target.value })
                }
              />
            </label>
            <label className="field">
              <span>Chunk size</span>
              <input
                className="input"
                value={bpswForm.chunkSize}
                onChange={(event) =>
                  onUpdateBpswForm({ chunkSize: event.target.value })
                }
              />
            </label>
          </div>
          <div className="checkbox-grid">
            {bpswTaskTypes.map((taskType) => (
              <label key={taskType} className="checkbox">
                <input
                  type="checkbox"
                  checked={bpswForm.taskTypes[taskType]}
                  onChange={() => onToggleBpswType(taskType)}
                />
                <span>{taskType.replace(/_/g, " ")}</span>
              </label>
            ))}
          </div>
          {bpswState.error && <p className="error">{bpswState.error}</p>}
          {bpswState.success && (
            <p className="muted">Started tasks: {bpswState.success}</p>
          )}
          {bpswActionState.error && <p className="error">{bpswActionState.error}</p>}
          <div className="form-actions">
            <button
              className="btn ghost"
              onClick={onSyncBpsw}
              disabled={bpswState.syncing || bpswActionLoading}
            >
              {bpswState.syncing ? "Syncing..." : "Sync scripts"}
            </button>
            <button
              className="btn primary"
              onClick={onStartBpsw}
              disabled={bpswState.loading || bpswActionLoading}
            >
              {bpswState.loading ? "Starting..." : bpswIsActive ? "Stop" : "Start"}
            </button>
            <button
              className="btn ghost"
              onClick={() => bpswProject && onProjectAction(bpswProject, "pause")}
              disabled={!bpswProject || bpswStatus === "stopped" || bpswActionLoading}
            >
              Pause
            </button>
          </div>
        </div>
      </div>
      <div className="table-card">
        <table>
          <thead>
            <tr>
              <th>ID</th>
              <th>Project</th>
              <th>Status</th>
              <th>Owner</th>
              <th>Progress</th>
              <th>Actions</th>
              <th>Details</th>
            </tr>
          </thead>
          <tbody>
            {projects.map((project) => {
              const actionState = project.id ? projectActions[project.id] ?? {} : {};
              const isActive = project.status === "active";
              const actionLabel = isActive ? "Stop" : "Start";
              const action = isActive ? "stop" : "start";
              return (
              <tr key={project.name}>
                <td className="mono">{project.id ?? "—"}</td>
                <td>{project.name}</td>
                <td className="status-label">{project.status}</td>
                <td>{project.owner}</td>
                <td>
                  <div className="mini-bar">
                    <div style={{ width: `${project.progress}%` }}></div>
                  </div>
                </td>
                <td>
                  <div className="pill-row">
                    <button
                      className="btn ghost"
                      onClick={() => onProjectAction(project, action)}
                      disabled={!project.id || actionState.loading}
                    >
                      {actionLabel}
                    </button>
                    <button
                      className="btn ghost"
                      onClick={() => onProjectAction(project, "pause")}
                      disabled={!project.id || project.status === "stopped" || actionState.loading}
                    >
                      Pause
                    </button>
                  </div>
                </td>
                <td>
                  <Link to={projectPath(project.name)} className="link">View</Link>
                </td>
              </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function ProjectDetailPage({ projects, onProjectAction, projectActions }) {
  const { projectName } = useParams();
  const decodedName = decodeURIComponent(projectName ?? "");
  const project = projects.find((entry) => entry.name === decodedName);
  if (!project) {
    return <p className="error">Project not found.</p>;
  }
  const actionState = project.id ? projectActions[project.id] ?? {} : {};
  const isActive = project.status === "active";
  const actionLabel = isActive ? "Stop" : "Start";
  const action = isActive ? "stop" : "start";
  return (
    <section className="section">
      <div className="section-header">
        <div>
          <h2>{project.name}</h2>
          <p>Project scope, tasks, and participants.</p>
        </div>
        <div className="chip">{project.status}</div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Project ID</h3>
          <p className="muted mono">{project.id ?? "—"}</p>
        </div>
        <div className="stat-card">
          <h3>Owner</h3>
          <p className="muted">{project.owner}</p>
        </div>
        <div className="stat-card">
          <h3>Progress</h3>
          <p className="muted">{project.progress}%</p>
        </div>
        <div className="stat-card">
          <h3>Participants</h3>
          <p className="muted">12 active agents</p>
        </div>
      </div>
      <div className="card-grid">
        <div className="stat-card">
          <h3>Actions</h3>
          <div className="pill-row">
            <button
              className="btn ghost"
              onClick={() => onProjectAction(project, action)}
              disabled={!project.id || actionState.loading}
            >
              {actionLabel}
            </button>
            <button
              className="btn ghost"
              onClick={() => onProjectAction(project, "pause")}
              disabled={!project.id || project.status === "stopped" || actionState.loading}
            >
              Pause
            </button>
          </div>
          {actionState.error && <p className="error">{actionState.error}</p>}
          {actionState.info && <p className="muted">{actionState.info}</p>}
        </div>
      </div>
    </section>
  );
}

function AiModePage({ liveAiMode, sectionId }) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>AI Mode</h2>
          <p>Governance settings with policy enforcement.</p>
        </div>
        <div className="chip">Policy engine active</div>
      </div>
      <div className="card-grid">
        {["AI_OFF", "AI_ADVISORY", "AI_ASSISTED", "AI_FULL"].map((mode) => (
          <div key={mode} className={`mode-card${liveAiMode === mode ? " active" : ""}`}>
            <h3>{mode}</h3>
            <p>
              {mode === "AI_OFF" && "Deterministic scheduling only, no AI proposals."}
              {mode === "AI_ADVISORY" && "AI suggests actions, policy gate still decides."}
              {mode === "AI_ASSISTED" && "Low-risk actions automated, critical tasks gated."}
              {mode === "AI_FULL" && "End-to-end AI orchestration with hard limits."}
            </p>
          </div>
        ))}
      </div>
    </section>
  );
}

function LogCenterPage({
  logFilters,
  onToggleFilter,
  visibleLogs,
  onLoadMore,
  sectionId
}) {
  const filteredLogs = logSeed.filter((entry) => logFilters[entry.level]);
  const visibleRows = filteredLogs.slice(0, visibleLogs);

  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Log Center</h2>
          <p>Audit trail across agents, sandboxes, and services.</p>
        </div>
        <div className="pill-row">
          {Object.keys(logFilters).map((level) => (
            <button
              key={level}
              className={`chip ${logFilters[level] ? "active" : ""}`}
              onClick={() => onToggleFilter(level)}
              aria-pressed={logFilters[level]}
            >
              {level}
            </button>
          ))}
        </div>
      </div>
      <div className="table-card table-scroll">
        <table>
          <thead>
            <tr>
              <th>Level</th>
              <th>Source</th>
              <th>Message</th>
              <th>Time</th>
            </tr>
          </thead>
          <tbody>
            {visibleRows.map((entry) => (
              <tr key={entry.id} className={`log-row ${entry.level}`}>
                <td>
                  <span className={`log-pill ${entry.level}`}>{entry.level}</span>
                </td>
                <td>{entry.source}</td>
                <td>{entry.message}</td>
                <td>{entry.time}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {filteredLogs.length > visibleRows.length && (
        <button className="btn ghost" onClick={onLoadMore}>
          Load more
        </button>
      )}
    </section>
  );
}

function SettingsPage({ settingsTab, onTabChange, sectionId }) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Settings</h2>
          <p>Organization profile, security, and access control.</p>
        </div>
        <div className="chip">Role-based access</div>
      </div>
      <div className="tab-row">
        {[
          { id: "general", label: "General" },
          { id: "security", label: "Security" },
          { id: "access", label: "Access" }
        ].map((tab) => (
          <button
            key={tab.id}
            className={`tab ${settingsTab === tab.id ? "active" : ""}`}
            onClick={() => onTabChange(tab.id)}
          >
            {tab.label}
          </button>
        ))}
      </div>
      <div className="card-grid">
        {settingsTab === "general" && (
          <>
            <div className="stat-card">
              <h3>Organization</h3>
              <p className="muted">Newral Labs, internal staging.</p>
              <button className="btn ghost">Edit profile</button>
            </div>
            <div className="stat-card">
              <h3>Resource Limits</h3>
              <p className="muted">Max concurrent tasks: 10</p>
              <button className="btn ghost">Adjust limits</button>
            </div>
          </>
        )}
        {settingsTab === "security" && (
          <>
            <div className="stat-card">
              <h3>API Keys</h3>
              <p className="muted">Rotate service keys every 30 days.</p>
              <button className="btn ghost">Manage keys</button>
            </div>
            <div className="stat-card">
              <h3>Certificates</h3>
              <p className="muted">TLS required for external agents.</p>
              <button className="btn ghost">Upload certs</button>
            </div>
          </>
        )}
        {settingsTab === "access" && (
          <>
            <div className="stat-card">
              <h3>Users</h3>
              <p className="muted">14 active users, 3 admins.</p>
              <button className="btn ghost">Invite user</button>
            </div>
            <div className="stat-card">
              <h3>Roles</h3>
              <p className="muted">Admin, Operator, Observer.</p>
              <button className="btn ghost">Edit roles</button>
            </div>
          </>
        )}
      </div>
    </section>
  );
}

function IntegrationsPage({ sectionId }) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>Integrations</h2>
          <p>Infrastructure backbone and outbound alerts.</p>
        </div>
        <div className="chip">Docker compose stack</div>
      </div>
      <div className="card-grid">
        {integrationTargets.map((integration) => (
          <div key={integration.name} className="stat-card">
            <h3>{integration.name}</h3>
            <p className="muted">{integration.note}</p>
            <span className={`pill ${integration.status}`}>{integration.status}</span>
          </div>
        ))}
      </div>
    </section>
  );
}

function SystemHealthPage({ health, healthLoaded, sectionId }) {
  return (
    <section className="section" id={sectionId}>
      <div className="section-header">
        <div>
          <h2>System Health</h2>
          <p>Live readiness checks proxied through the gateway.</p>
        </div>
        <div className="chip">SSE enabled</div>
      </div>
      <div className="health-grid">
        {healthTargets.map((service) => {
          const status = health[service.id]?.status ?? "loading";
          return (
            <div key={service.id} className="health-card">
              <div>
                <h3>{service.name}</h3>
                <p className="muted">{service.base}</p>
              </div>
              <span className={`pill ${healthLoaded ? status : "loading"}`}>
                {healthLoaded ? status : "loading"}
              </span>
            </div>
          );
        })}
        <div className="health-card">
          <div>
            <h3>Object Storage</h3>
            <p className="muted">MinIO /data</p>
          </div>
          <span className="pill ok">ok</span>
        </div>
      </div>
    </section>
  );
}

function PortalApp() {
  const location = useLocation();
  const [health, setHealth] = useState({});
  const [healthLoaded, setHealthLoaded] = useState(false);
  const [liveAgents, setLiveAgents] = useState(initialAgents);
  const [agentsData, setAgentsData] = useState(initialAgents);
  const [liveTasks, setLiveTasks] = useState(initialTasks);
  const [liveQueue, setLiveQueue] = useState({
    queued: 0,
    running: 0,
    completed: 0
  });
  const [liveLoad, setLiveLoad] = useState({
    running: 0,
    queued: 0,
    completed_last_min: 0
  });
  const [projectsData, setProjectsData] = useState(seedProjects);
  const [liveAiMode, setLiveAiMode] = useState("AI_OFF");
  const [demoStatus, setDemoStatus] = useState({
    total: 0,
    completed: 0,
    running: 0,
    queued: 0
  });
  const [demoResult, setDemoResult] = useState([]);
  const [demoLoading, setDemoLoading] = useState(false);
  const [demoError, setDemoError] = useState("");
  const [summaryError, setSummaryError] = useState("");
  const [summaryLoaded, setSummaryLoaded] = useState(false);
  const [taskQuery, setTaskQuery] = useState("");
  const [taskStatus, setTaskStatus] = useState("all");
  const [visibleTasks, setVisibleTasks] = useState(6);
  const [visibleLogs, setVisibleLogs] = useState(6);
  const [logFilters, setLogFilters] = useState({
    info: true,
    success: true,
    warn: true,
    error: true
  });
  const [settingsTab, setSettingsTab] = useState("general");
  const [theme, setTheme] = useState("light");
  const [createProjectState, setCreateProjectState] = useState("idle");
  const [portalVersion, setPortalVersion] = useState("");
  const [summaryErrorCount, setSummaryErrorCount] = useState(0);
  const [activeSection, setActiveSection] = useState("dashboard");
  const [dashboardSnapshot, setDashboardSnapshot] = useState(
    emptyDashboardSnapshot
  );
  const [bpswForm, setBpswForm] = useState(() => ({
    start: "",
    end: "",
    chunkSize: "10000",
    taskTypes: bpswTaskTypes.reduce((acc, item) => {
      acc[item] = true;
      return acc;
    }, {})
  }));
  const [bpswState, setBpswState] = useState({
    loading: false,
    syncing: false,
    error: "",
    success: ""
  });
  const [projectActions, setProjectActions] = useState({});

  useEffect(() => {
    const saved = window.localStorage.getItem("newral-theme");
    if (saved) {
      setTheme(saved);
      return;
    }
    const prefersDark = window.matchMedia?.("(prefers-color-scheme: dark)").matches;
    setTheme(prefersDark ? "dark" : "light");
  }, []);

  const logPortalEvent = useCallback(async (level, message, context = {}) => {
    try {
      await fetch("/api/scheduler/v1/portal/logs", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ level, message, context })
      });
    } catch (error) {
      // Avoid cascading failures on logging.
    }
  }, []);

  useEffect(() => {
    let isMounted = true;
    const loadVersion = async () => {
      try {
        const response = await fetch("/version.txt", { cache: "no-store" });
        if (!response.ok) {
          return;
        }
        const text = (await response.text()).trim();
        if (text && isMounted) {
          setPortalVersion(text);
        }
      } catch (error) {
        // Keep silent to avoid breaking UI.
      }
    };
    loadVersion();
    return () => {
      isMounted = false;
    };
  }, []);

  useEffect(() => {
    document.documentElement.setAttribute("data-theme", theme);
    window.localStorage.setItem("newral-theme", theme);
  }, [theme]);

  useEffect(() => {
    if (location.pathname !== "/") {
      setActiveSection("");
      return;
    }
    const anchors = navItems.map((item) => item.anchor);
    const sections = anchors
      .map((anchor) => document.getElementById(anchor))
      .filter(Boolean);
    const updateActive = () => {
      const offset = 140;
      const scrollTop = window.scrollY + offset;
      let current = anchors[0];
      for (const section of sections) {
        if (section.offsetTop <= scrollTop) {
          current = section.id;
        } else {
          break;
        }
      }
      setActiveSection(current);
    };
    updateActive();
    window.addEventListener("scroll", updateActive, { passive: true });
    window.addEventListener("hashchange", updateActive);
    return () => {
      window.removeEventListener("scroll", updateActive);
      window.removeEventListener("hashchange", updateActive);
    };
  }, [location.pathname]);

  useEffect(() => {
    let isMounted = true;

    const fetchStatus = async (target) => {
      try {
        const [healthz, readyz] = await Promise.all([
          fetch(`${target.base}/healthz`),
          fetch(`${target.base}/readyz`)
        ]);
        const ok = healthz.ok && readyz.ok;
        const partial = healthz.ok && !readyz.ok;
        const status = ok ? "ok" : partial ? "warn" : "down";
        return { status };
      } catch (error) {
        return { status: "down" };
      }
    };

    const refresh = async () => {
      const results = await Promise.all(
        healthTargets.map(async (target) => [target.id, await fetchStatus(target)])
      );
      if (isMounted) {
        setHealth(Object.fromEntries(results));
        setHealthLoaded(true);
      }
    };

    refresh();
    const interval = setInterval(refresh, 15000);

    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let isMounted = true;
    const fetchAgents = async () => {
      try {
        const response = await fetch("/api/scheduler/v1/agents");
        if (!response.ok) {
          return;
        }
        const data = await response.json();
        if (!Array.isArray(data) || !isMounted) {
          return;
        }
        const mapped = data.map((agent) => ({
          id: agent.agent_uid,
          status: agent.status ?? "idle",
          last_seen: agent.last_seen ?? null,
          blocked: agent.blocked ?? false,
          blocked_reason: agent.blocked_reason ?? null,
          hardware: agent.hardware ?? null,
          metrics: agent.metrics ?? null
        }));
        setAgentsData(mapped);
      } catch (error) {
        // Keep silent to avoid breaking UI.
      }
    };
    fetchAgents();
    const interval = setInterval(fetchAgents, 10000);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let isMounted = true;
    const fetchProjects = async () => {
      try {
        const response = await fetch("/api/scheduler/v1/projects");
        if (!response.ok) {
          return;
        }
        const data = await response.json();
        if (!Array.isArray(data) || !isMounted) {
          return;
        }
        const mapped = data.map((project) => ({
          id: project.id ?? null,
          name: project.name,
          status: project.status ?? (project.is_demo ? "demo" : "active"),
          owner: project.owner_id ?? "core",
          progress: project.is_demo ? 100 : 0,
          is_demo: project.is_demo ?? false
        }));
        if (mapped.length > 0) {
          setProjectsData(mapped);
        }
      } catch (error) {
        // Keep silent to avoid breaking UI.
      }
    };
    fetchProjects();
    const interval = setInterval(fetchProjects, 30000);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  useEffect(() => {
    let isMounted = true;
    let eventSource = null;
    let reconnectTimer = null;

    const applySummary = (summary) => {
      if (!summary || !isMounted) {
        return;
      }
      if (Array.isArray(summary.agents)) {
        setLiveAgents(summary.agents);
      }
      if (Array.isArray(summary.tasks)) {
        setLiveTasks(summary.tasks);
      }
      if (summary.queue) {
        setLiveQueue({
          queued: summary.queue.queued ?? 0,
          running: summary.queue.running ?? 0,
          completed: summary.queue.completed ?? 0
        });
      }
      if (summary.load) {
        setLiveLoad({
          running: summary.load.running ?? 0,
          queued: summary.load.queued ?? 0,
          completed_last_min: summary.load.completed_last_min ?? 0
        });
      }
      if (summary.dashboard) {
        setDashboardSnapshot({
          ...emptyDashboardSnapshot,
          ...summary.dashboard,
          agent_availability: {
            ...emptyDashboardSnapshot.agent_availability,
            ...(summary.dashboard.agent_availability ?? {})
          },
          storage_io: {
            ...emptyDashboardSnapshot.storage_io,
            ...(summary.dashboard.storage_io ?? {})
          },
          throughput: {
            ...emptyDashboardSnapshot.throughput,
            ...(summary.dashboard.throughput ?? {})
          },
          trust: {
            ...emptyDashboardSnapshot.trust,
            ...(summary.dashboard.trust ?? {})
          }
        });
      }
      if (summary.ai_mode) {
        setLiveAiMode(summary.ai_mode);
      }
      if (!portalVersion && summary.version) {
        setPortalVersion(summary.version);
      }
      setSummaryLoaded(true);
    };

    const fetchSummary = async () => {
      try {
        const response = await fetch("/api/scheduler/v1/summary");
        if (!response.ok) {
          throw new Error("summary fetch failed");
        }
        const data = await response.json();
        applySummary(data);
        setSummaryError("");
        setSummaryErrorCount(0);
      } catch (error) {
        setSummaryErrorCount((prev) => {
          const next = prev + 1;
          if (next >= 2) {
            setSummaryError("Live summary unavailable");
            setSummaryLoaded(true);
            logPortalEvent("warn", "live_summary_unavailable", {
              message: error?.message ?? "summary fetch failed"
            });
          }
          return next;
        });
      }
    };

    const connect = () => {
      eventSource = new EventSource("/api/scheduler/v1/stream");
      eventSource.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          if (!data?.error) {
            applySummary(data);
            setSummaryError("");
            setSummaryErrorCount(0);
          }
        } catch (error) {
          setSummaryError("Live summary parse error");
        }
      };
      eventSource.onerror = () => {
        eventSource.close();
        fetchSummary();
        reconnectTimer = window.setTimeout(connect, 2000);
      };
    };

    fetchSummary();
    connect();

    return () => {
      isMounted = false;
      if (eventSource) {
        eventSource.close();
      }
      if (reconnectTimer) {
        window.clearTimeout(reconnectTimer);
      }
    };
  }, []);

  useEffect(() => {
    let isMounted = true;

    const refresh = async () => {
      try {
        const statusResponse = await fetch(
          "/api/scheduler/v1/demo/wordcount/status"
        );
        if (!statusResponse.ok) {
          return;
        }
        const status = await statusResponse.json();
        if (isMounted) {
          setDemoStatus(status);
        }

        if (status.total > 0 && status.completed >= status.total) {
          const resultResponse = await fetch(
            "/api/scheduler/v1/demo/wordcount/result"
          );
          if (resultResponse.ok) {
            const result = await resultResponse.json();
            if (isMounted) {
              setDemoResult(result.top_words || []);
            }
          }
        }
      } catch (error) {
        // Keep silent to avoid breaking UI.
      }
    };

    refresh();
    const interval = setInterval(refresh, 12000);
    return () => {
      isMounted = false;
      clearInterval(interval);
    };
  }, []);

  const handleStartDemo = async () => {
    setDemoLoading(true);
    setDemoError("");
    logPortalEvent("info", "demo_start_requested", { parts: 5 });
    try {
      const response = await fetch(
        "/api/scheduler/v1/demo/wordcount/start?parts=5",
        { method: "POST" }
      );
      if (!response.ok) {
        const message = await readApiError(response);
        setDemoError("Failed to start demo");
        logPortalEvent("error", "demo_start_failed", { status: response.status, message });
      }
    } catch (error) {
      setDemoError("Failed to start demo");
      logPortalEvent("error", "demo_start_failed", { message: error?.message });
    } finally {
      setDemoLoading(false);
    }
  };

  const handleCreateProject = () => {
    if (createProjectState === "loading") {
      return;
    }
    setCreateProjectState("loading");
    logPortalEvent("info", "create_project_clicked");
    window.setTimeout(() => {
      setCreateProjectState("success");
      window.setTimeout(() => setCreateProjectState("idle"), 1200);
    }, 800);
  };

  const updateBpswForm = (patch) => {
    setBpswForm((prev) => ({
      ...prev,
      ...patch
    }));
  };

  const toggleBpswType = (taskType) => {
    setBpswForm((prev) => ({
      ...prev,
      taskTypes: {
        ...prev.taskTypes,
        [taskType]: !prev.taskTypes[taskType]
      }
    }));
  };

  const validateBpswRange = () => {
    if ((bpswForm.start && !bpswForm.end) || (!bpswForm.start && bpswForm.end)) {
      return "Start and end must both be provided, or left empty.";
    }
    return "";
  };

  const handleSyncBpsw = async () => {
    if (bpswState.syncing) {
      return;
    }
    setBpswState((prev) => ({
      ...prev,
      syncing: true,
      error: "",
      success: ""
    }));
    logPortalEvent("info", "bpsw_sync_requested");
    try {
      const response = await fetch(
        "/api/scheduler/v1/projects/bpsw/scripts/sync",
        { method: "POST" }
      );
      if (!response.ok) {
        const message = await readApiError(response);
        setBpswState((prev) => ({
          ...prev,
          error: message || "Failed to sync scripts"
        }));
        logPortalEvent("error", "bpsw_sync_failed", { status: response.status, message });
      }
    } catch (error) {
      setBpswState((prev) => ({
        ...prev,
        error: "Failed to sync scripts"
      }));
      logPortalEvent("error", "bpsw_sync_failed", { message: error?.message });
    } finally {
      setBpswState((prev) => ({
        ...prev,
        syncing: false
      }));
    }
  };


  const handleStartBpsw = async () => {
    if (bpswState.loading) {
      return;
    }
    const bpswProject = projectsData.find((project) => project.name === "bpsw_hunter");
    if (bpswProject && bpswProject.status === "active") {
      await handleProjectAction(bpswProject, "stop");
      return;
    }
    if (bpswProject && bpswProject.status === "paused") {
      await handleProjectAction(bpswProject, "start");
      return;
    }
    const validationError = validateBpswRange();
    if (validationError) {
      setBpswState((prev) => ({
        ...prev,
        error: validationError,
        success: ""
      }));
      return;
    }
    setBpswState((prev) => ({
      ...prev,
      loading: true,
      error: "",
      success: ""
    }));
    logPortalEvent("info", "bpsw_start_requested");
    try {
      const selectedTypes = Object.keys(bpswForm.taskTypes).filter(
        (taskType) => bpswForm.taskTypes[taskType]
      );
      const payload = {};
      if (bpswForm.start) {
        payload.start = bpswForm.start.trim();
      }
      if (bpswForm.end) {
        payload.end = bpswForm.end.trim();
      }
      if (bpswForm.chunkSize) {
        const chunkSize = Number(bpswForm.chunkSize);
        if (!Number.isNaN(chunkSize) && chunkSize > 0) {
          payload.chunk_size = Math.floor(chunkSize);
        }
      }
      if (selectedTypes.length > 0) {
        payload.task_types = selectedTypes;
      }

      const response = await fetch(
        "/api/scheduler/v1/projects/bpsw/start",
        {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload)
        }
      );
      if (!response.ok) {
        const message = await readApiError(response);
        setBpswState((prev) => ({
          ...prev,
          error: message || "Failed to start project"
        }));
        logPortalEvent("error", "bpsw_start_failed", { status: response.status, message });
        return;
      }
      const result = await response.json();
      setBpswState((prev) => ({
        ...prev,
        success: result.total_tasks?.toString() || "ok"
      }));
      if (result.project_id) {
        setProjectsData((prev) =>
          prev.map((entry) =>
            entry.id === result.project_id
              ? { ...entry, status: "active" }
              : entry
          )
        );
      }
      logPortalEvent("info", "bpsw_start_success", { total_tasks: result.total_tasks });
    } catch (error) {
      setBpswState((prev) => ({
        ...prev,
        error: "Failed to start project"
      }));
      logPortalEvent("error", "bpsw_start_failed", { message: error?.message });
    } finally {
      setBpswState((prev) => ({
        ...prev,
        loading: false
      }));
    }
  };

  const updateProjectActionState = (projectId, patch) => {
    setProjectActions((prev) => ({
      ...prev,
      [projectId]: {
        ...(prev[projectId] ?? {}),
        ...patch
      }
    }));
  };

  const handleProjectAction = async (project, action) => {
    if (!project?.id) {
      return;
    }
    updateProjectActionState(project.id, { loading: action, error: "" });
    logPortalEvent("info", "project_action", { project_id: project.id, action });
    try {
      const response = await fetch(`/api/scheduler/v1/projects/${project.id}/${action}`, {
        method: "POST"
      });
      if (!response.ok) {
        const message = await readApiError(response);
        updateProjectActionState(project.id, {
          loading: "",
          error: message || "Project action failed"
        });
        logPortalEvent("error", "project_action_failed", {
          project_id: project.id,
          action,
          status: response.status,
          message
        });
        return;
      }
      const data = await response.json();
      if (data?.project) {
        setProjectsData((prev) =>
          prev.map((entry) =>
            entry.id === data.project.id
              ? {
                  ...entry,
                  status: data.project.status,
                  name: data.project.name,
                  owner: data.project.owner_id ?? entry.owner
                }
              : entry
          )
        );
      }
      if (action === "stop" && data?.affected_tasks) {
        updateProjectActionState(project.id, {
          loading: "",
          info: `Stopped tasks: ${data.affected_tasks}`
        });
      } else {
        updateProjectActionState(project.id, { loading: "", info: "" });
      }
      logPortalEvent("info", "project_action_success", {
        project_id: project.id,
        action
      });
    } catch (error) {
      updateProjectActionState(project.id, {
        loading: "",
        error: "Project action failed"
      });
      logPortalEvent("error", "project_action_failed", {
        project_id: project.id,
        action,
        message: error?.message
      });
    }
  };

  const connectedAgents = agentsData.filter((agent) => agent.status === "online")
    .length;
  const kpis = baseKpis.map((metric) => {
    if (metric.label === "Connected Agents") {
      return {
        ...metric,
        value: connectedAgents.toLocaleString()
      };
    }
    if (metric.label === "Tasks Completed") {
      return {
        ...metric,
        value: liveQueue.completed.toLocaleString()
      };
    }
    if (metric.label === "Queue Depth") {
      return {
        ...metric,
        value: (liveQueue.queued + liveQueue.running).toLocaleString()
      };
    }
    if (metric.label === "AI Mode") {
      return {
        ...metric,
        value: liveAiMode
      };
    }
    if (metric.label === "Storage Usage") {
      const totalMb =
        (dashboardSnapshot.storage_io?.disk_read_mb ?? 0) +
        (dashboardSnapshot.storage_io?.disk_write_mb ?? 0);
      const value =
        totalMb >= 1024
          ? `${(totalMb / 1024).toFixed(1)} GB`
          : `${totalMb.toFixed(1)} MB`;
      return {
        ...metric,
        value
      };
    }
    return metric;
  });

  const tasks = useMemo(() => {
    return liveTasks.map((task, index) => {
      const agentId =
        agentsData.length > 0
          ? agentsData[index % agentsData.length]?.id
          : "auto";
      return {
        ...task,
        project: projectsData[index % projectsData.length]?.name ?? "Demo",
        agent: agentId ?? "auto",
        priority: task.priority ?? "normal"
      };
    });
  }, [liveTasks, agentsData, projectsData]);

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"));
  };

  const toggleLogFilter = (level) => {
    setLogFilters((prev) => ({
      ...prev,
      [level]: !prev[level]
    }));
  };

  const handleNavClick = (anchor) => {
    setActiveSection(anchor);
    logPortalEvent("info", "nav_click", { anchor });
  };

  return (
    <Layout
      liveAgents={liveAgents}
      liveTasks={tasks}
      projects={projectsData}
      theme={theme}
      onToggleTheme={toggleTheme}
      onCreateProject={handleCreateProject}
      createProjectState={createProjectState}
      portalVersion={portalVersion}
      activeSection={activeSection}
      onNavClick={handleNavClick}
    >
      <Routes>
        <Route
          path="/"
          element={
            <>
              <DashboardPage
                kpis={kpis}
                summaryError={summaryError}
                summaryLoaded={summaryLoaded}
                liveLoad={liveLoad}
                dashboard={dashboardSnapshot}
                sectionId="dashboard"
              />
              <DemoPage
                demoStatus={demoStatus}
                demoResult={demoResult}
                demoLoading={demoLoading}
                demoError={demoError}
                onStartDemo={handleStartDemo}
                sectionId="demo"
              />
              <AgentsPage
                agents={agentsData}
                summaryLoaded={summaryLoaded}
                sectionId="agents"
              />
              <TasksPage
                tasks={tasks}
                summaryLoaded={summaryLoaded}
                taskQuery={taskQuery}
                taskStatus={taskStatus}
                onQueryChange={setTaskQuery}
                onStatusChange={setTaskStatus}
                visibleTasks={visibleTasks}
                onLoadMore={() => setVisibleTasks((prev) => prev + 6)}
                sectionId="tasks"
              />
              <ProjectsPage
                onCreateProject={handleCreateProject}
                bpswForm={bpswForm}
                onUpdateBpswForm={updateBpswForm}
                onToggleBpswType={toggleBpswType}
                onStartBpsw={handleStartBpsw}
                onProjectAction={handleProjectAction}
                onSyncBpsw={handleSyncBpsw}
                bpswState={bpswState}
                projectActions={projectActions}
                sectionId="projects"
                projects={projectsData}
              />
              <AiModePage liveAiMode={liveAiMode} sectionId="ai-mode" />
              <LogCenterPage
                logFilters={logFilters}
                onToggleFilter={toggleLogFilter}
                visibleLogs={visibleLogs}
                onLoadMore={() => setVisibleLogs((prev) => prev + 6)}
                sectionId="log-center"
              />
              <SettingsPage
                settingsTab={settingsTab}
                onTabChange={setSettingsTab}
                sectionId="settings"
              />
              <IntegrationsPage sectionId="integrations" />
              <SystemHealthPage
                health={health}
                healthLoaded={healthLoaded}
                sectionId="system-health"
              />
            </>
          }
        />
        <Route
          path="/demo"
          element={
            <DemoPage
              demoStatus={demoStatus}
              demoResult={demoResult}
              demoLoading={demoLoading}
              demoError={demoError}
              onStartDemo={handleStartDemo}
            />
          }
        />
        <Route
          path="/agents"
          element={<AgentsPage agents={agentsData} summaryLoaded={summaryLoaded} />}
        />
        <Route path="/agents/:agentId" element={<AgentDetailPage agents={agentsData} />} />
        <Route
          path="/tasks"
          element={
            <TasksPage
              tasks={tasks}
              summaryLoaded={summaryLoaded}
              taskQuery={taskQuery}
              taskStatus={taskStatus}
              onQueryChange={setTaskQuery}
              onStatusChange={setTaskStatus}
              visibleTasks={visibleTasks}
              onLoadMore={() => setVisibleTasks((prev) => prev + 6)}
            />
          }
        />
        <Route path="/tasks/:taskId" element={<TaskDetailPage tasks={tasks} />} />
        <Route
          path="/projects"
          element={
            <ProjectsPage
              onCreateProject={handleCreateProject}
              bpswForm={bpswForm}
              onUpdateBpswForm={updateBpswForm}
              onToggleBpswType={toggleBpswType}
              onStartBpsw={handleStartBpsw}
              onProjectAction={handleProjectAction}
              onSyncBpsw={handleSyncBpsw}
              bpswState={bpswState}
              projectActions={projectActions}
              projects={projectsData}
            />
          }
        />
        <Route
          path="/projects/:projectName"
          element={
            <ProjectDetailPage
              projects={projectsData}
              onProjectAction={handleProjectAction}
              projectActions={projectActions}
            />
          }
        />
        <Route path="/ai-mode" element={<AiModePage liveAiMode={liveAiMode} />} />
        <Route
          path="/log-center"
          element={
            <LogCenterPage
              logFilters={logFilters}
              onToggleFilter={toggleLogFilter}
              visibleLogs={visibleLogs}
              onLoadMore={() => setVisibleLogs((prev) => prev + 6)}
            />
          }
        />
        <Route
          path="/settings"
          element={<SettingsPage settingsTab={settingsTab} onTabChange={setSettingsTab} />}
        />
        <Route path="/integrations" element={<IntegrationsPage />} />
        <Route
          path="/system-health"
          element={<SystemHealthPage health={health} healthLoaded={healthLoaded} />}
        />
      </Routes>
    </Layout>
  );
}

export default function App() {
  return (
    <BrowserRouter>
      <PortalApp />
    </BrowserRouter>
  );
}
