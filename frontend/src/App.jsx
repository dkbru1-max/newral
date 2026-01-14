import { useEffect, useState } from "react";

const kpis = [
  { label: "Connected Agents", value: "1,284", trend: "+12%" },
  { label: "Tasks Completed", value: "48,210", trend: "+6%" },
  { label: "Avg Task Time", value: "4.2s", trend: "-8%" },
  { label: "Uptime", value: "99.94%", trend: "stable" },
  { label: "AI Mode", value: "AI_OFF", trend: "policy gated" }
];

const sections = [
  { id: "dashboard", title: "Dashboard" },
  { id: "investor-demo", title: "Investor Demo" },
  { id: "agents", title: "Agents" },
  { id: "tasks", title: "Tasks" },
  { id: "monitoring", title: "Monitoring" },
  { id: "ai-mode", title: "AI Mode" },
  { id: "projects", title: "Projects" },
  { id: "system-health", title: "System Health" }
];

const agents = [
  { id: "node-01", status: "online", region: "EU-West", reputation: "0.8" },
  { id: "node-14", status: "online", region: "NA-East", reputation: "0.9" },
  { id: "node-27", status: "idle", region: "AP-South", reputation: "0.7" }
];

const tasks = [
  { id: "task-1001", status: "queued", priority: "high" },
  { id: "task-1002", status: "running", priority: "normal" },
  { id: "task-1003", status: "waiting", priority: "low" }
];

const projects = [
  { name: "Prime Search", status: "active", owner: "Research" },
  { name: "LLM Fine-tuning", status: "staged", owner: "AI Lab" },
  { name: "Bio Sim", status: "draft", owner: "BioTech" }
];

const healthTargets = [
  { id: "identity", name: "Identity Service", base: "/api/identity" },
  { id: "scheduler", name: "Scheduler Service", base: "/api/scheduler" },
  { id: "validator", name: "Validator Service", base: "/api/validator" },
  { id: "telemetry", name: "Telemetry Service", base: "/api/telemetry" }
];

function App() {
  const [health, setHealth] = useState({});
  const [demoStatus, setDemoStatus] = useState({
    total: 0,
    completed: 0,
    running: 0,
    queued: 0
  });
  const [demoResult, setDemoResult] = useState([]);
  const [demoLoading, setDemoLoading] = useState(false);
  const [demoError, setDemoError] = useState("");

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
    try {
      const response = await fetch(
        "/api/scheduler/v1/demo/wordcount/start?parts=5",
        { method: "POST" }
      );
      if (!response.ok) {
        setDemoError("Failed to start demo");
      }
    } catch (error) {
      setDemoError("Failed to start demo");
    } finally {
      setDemoLoading(false);
    }
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          {/* Brand logo from public assets */}
          <img src="/newral_big_logo.png" alt="Newral" className="logo" />
          <div>
            <p className="eyebrow">Newral Portal</p>
            <h1>Investor View</h1>
          </div>
        </div>
        <nav className="nav-links">
          {sections.map((section) => (
            <a key={section.id} href={`#${section.id}`}>
              {section.title}
            </a>
          ))}
        </nav>
      </aside>

      <div className="main">
        <header className="topbar">
          <div>
            <p className="eyebrow">Network Overview</p>
            <h2>Distributed compute, policy-first.</h2>
          </div>
          <div className="topbar-actions">
            {/* Sign in will be enabled once auth is wired. */}
            <button className="btn" disabled>
              Sign in
            </button>
          </div>
        </header>

        <main className="content">
          <section id="dashboard" className="section">
            <div className="section-header">
              <h2>Dashboard</h2>
              <p>Snapshot of the MVP backbone and AI governance.</p>
            </div>
            <div className="metric-grid">
              {kpis.map((metric) => (
                <div key={metric.label} className="metric-card">
                  <span>{metric.label}</span>
                  <strong>{metric.value}</strong>
                  <em>{metric.trend}</em>
                </div>
              ))}
            </div>
            <div className="chart-grid">
              <div className="chart-card">
                <div className="chart-header">
                  <h3>Tasks (last 24h)</h3>
                  <span>placeholder</span>
                </div>
                <div className="chart-surface" aria-hidden="true"></div>
              </div>
              <div className="chart-card">
                <div className="chart-header">
                  <h3>Agent availability</h3>
                  <span>placeholder</span>
                </div>
                <div className="chart-surface alt" aria-hidden="true"></div>
              </div>
            </div>
            <div className="vision-card">
              <h3>Roadmap / Vision</h3>
              <ul>
                <li>Security-first compute with continuous verification.</li>
                <li>Scale to multi-region clusters with elastic orchestration.</li>
                <li>AI-guided scheduling under deterministic policy control.</li>
              </ul>
            </div>
          </section>

          <section id="investor-demo" className="section">
            <div className="section-header">
              <h2>Investor Demo</h2>
              <p>Launch the wordcount demo and track its progress.</p>
            </div>
            <div className="demo-card">
              <div>
                <h3>Wordcount pipeline</h3>
                <p className="muted">
                  Python sandbox tasks split into 5 parts with aggregation.
                </p>
              </div>
              <button
                className="btn primary"
                onClick={handleStartDemo}
                disabled={demoLoading}
              >
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
                        ? `${Math.min(
                            100,
                            (demoStatus.completed / demoStatus.total) * 100
                          )}%`
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

          <section id="agents" className="section">
            <div className="section-header">
              <h2>Agents</h2>
              <p>Live status for connected compute nodes.</p>
            </div>
            <div className="table-card">
              <table>
                <thead>
                  <tr>
                    <th>Node</th>
                    <th>Status</th>
                    <th>Region</th>
                    <th>Reputation</th>
                  </tr>
                </thead>
                <tbody>
                  {agents.map((agent) => (
                    <tr key={agent.id}>
                      <td>{agent.id}</td>
                      <td className={`status ${agent.status}`}>
                        {agent.status}
                      </td>
                      <td>{agent.region}</td>
                      <td>{agent.reputation}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section id="tasks" className="section">
            <div className="section-header">
              <h2>Tasks</h2>
              <p>Queue preview with priority tagging.</p>
            </div>
            <div className="card-grid">
              {tasks.map((task) => (
                <div key={task.id} className="task-card">
                  <span>{task.id}</span>
                  <strong>{task.status}</strong>
                  <em>{task.priority}</em>
                </div>
              ))}
            </div>
          </section>

          <section id="monitoring" className="section">
            <div className="section-header">
              <h2>Monitoring</h2>
              <p>Service readiness, latency bands, and stability signals.</p>
            </div>
            <div className="monitor-grid">
              <div className="monitor-card">
                <h3>Core services</h3>
                <ul>
                  <li>Identity: healthy</li>
                  <li>Scheduler: healthy</li>
                  <li>Validator: healthy</li>
                  <li>Telemetry: healthy</li>
                </ul>
              </div>
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
                      <div style={{ width: "71%" }}></div>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </section>

          <section id="ai-mode" className="section">
            <div className="section-header">
              <h2>AI Mode</h2>
              <p>Governance settings with policy enforcement.</p>
            </div>
            <div className="card-grid">
              <div className="mode-card active">
                <h3>AI_OFF</h3>
                <p>Deterministic scheduling only, no AI proposals.</p>
              </div>
              <div className="mode-card">
                <h3>AI_ADVISORY</h3>
                <p>AI suggests actions, policy gate still decides.</p>
              </div>
              <div className="mode-card">
                <h3>AI_ASSISTED</h3>
                <p>Low-risk actions automated, critical tasks gated.</p>
              </div>
              <div className="mode-card">
                <h3>AI_FULL</h3>
                <p>End-to-end AI orchestration with hard limits.</p>
              </div>
            </div>
          </section>

          <section id="projects" className="section">
            <div className="section-header">
              <h2>Projects</h2>
              <p>Portfolio view for active and staged workloads.</p>
            </div>
            <div className="table-card">
              <table>
                <thead>
                  <tr>
                    <th>Project</th>
                    <th>Status</th>
                    <th>Owner</th>
                  </tr>
                </thead>
                <tbody>
                  {projects.map((project) => (
                    <tr key={project.name}>
                      <td>{project.name}</td>
                      <td className="status">{project.status}</td>
                      <td>{project.owner}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section id="system-health" className="section">
            <div className="section-header">
              <h2>System Health</h2>
              <p>Live readiness checks proxied through the gateway.</p>
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
                    <span className={`pill ${status}`}>{status}</span>
                  </div>
                );
              })}
            </div>
          </section>
        </main>
      </div>
    </div>
  );
}

export default App;
