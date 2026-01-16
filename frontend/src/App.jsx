import { useEffect, useMemo, useRef, useState } from "react";

const baseKpis = [
  { label: "Connected Agents", trend: "live" },
  { label: "Tasks Completed", trend: "live" },
  { label: "Queue Depth", trend: "live" },
  { label: "AI Mode", trend: "policy gated" },
  { label: "Storage Usage", value: "42%", trend: "object store" }
];

const sections = [
  { id: "dashboard", title: "Dashboard" },
  { id: "demo", title: "Demo" },
  { id: "agents", title: "Agents" },
  { id: "tasks", title: "Tasks" },
  { id: "projects", title: "Projects" },
  { id: "ai-mode", title: "AI Mode" },
  { id: "log-center", title: "Log Center" },
  { id: "settings", title: "Settings" },
  { id: "integrations", title: "Integrations" },
  { id: "system-health", title: "System Health" }
];

const initialAgents = [
  { id: "node-01", status: "online", region: "EU-West", reputation: "0.8" },
  { id: "node-14", status: "online", region: "NA-East", reputation: "0.9" },
  { id: "node-27", status: "idle", region: "AP-South", reputation: "0.7" }
];

const initialTasks = [
  { id: "task-1001", status: "queued", priority: "high" },
  { id: "task-1002", status: "running", priority: "normal" },
  { id: "task-1003", status: "waiting", priority: "low" }
];

const projects = [
  { name: "Prime Search", status: "active", owner: "Research", progress: 68 },
  { name: "LLM Fine-tuning", status: "staged", owner: "AI Lab", progress: 22 },
  { name: "Bio Sim", status: "draft", owner: "BioTech", progress: 9 }
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

function App() {
  const [health, setHealth] = useState({});
  const [healthLoaded, setHealthLoaded] = useState(false);
  const [liveAgents, setLiveAgents] = useState(initialAgents);
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
  const [scrollProgress, setScrollProgress] = useState(0);
  const [showScrollTop, setShowScrollTop] = useState(false);
  const [createProjectState, setCreateProjectState] = useState("idle");

  const manualTimerRef = useRef(null);
  const flashTimerRef = useRef(null);
  const createTimerRef = useRef({ loading: null, success: null });

  const [activeSection, setActiveSection] = useState(sections[0]?.id ?? "");
  const [manualActive, setManualActive] = useState("");
  const [flashSection, setFlashSection] = useState("");

  useEffect(() => {
    if (typeof window === "undefined") {
      return;
    }
    const saved = window.localStorage.getItem("newral-theme");
    if (saved) {
      setTheme(saved);
      return;
    }
    const prefersDark = window.matchMedia?.("(prefers-color-scheme: dark)").matches;
    setTheme(prefersDark ? "dark" : "light");
  }, []);

  useEffect(() => {
    if (typeof document === "undefined") {
      return;
    }
    document.documentElement.setAttribute("data-theme", theme);
    window.localStorage.setItem("newral-theme", theme);
  }, [theme]);

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
      if (summary.ai_mode) {
        setLiveAiMode(summary.ai_mode);
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
      } catch (error) {
        setSummaryError("Live summary unavailable");
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

  useEffect(() => {
    const handleScroll = () => {
      const scrollTop = window.scrollY || document.documentElement.scrollTop;
      const docHeight =
        document.documentElement.scrollHeight -
        document.documentElement.clientHeight;
      const progress = docHeight > 0 ? Math.min(1, scrollTop / docHeight) : 0;
      setScrollProgress(progress);
      setShowScrollTop(scrollTop > 240);
    };

    handleScroll();
    window.addEventListener("scroll", handleScroll, { passive: true });
    window.addEventListener("resize", handleScroll);
    return () => {
      window.removeEventListener("scroll", handleScroll);
      window.removeEventListener("resize", handleScroll);
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

  const handleNavClick = (sectionId) => {
    // Keep the clicked item active even if the scroll position barely moves.
    setManualActive(sectionId);
    setActiveSection(sectionId);
    setFlashSection(sectionId);
    if (manualTimerRef.current) {
      clearTimeout(manualTimerRef.current);
    }
    if (flashTimerRef.current) {
      clearTimeout(flashTimerRef.current);
    }
    manualTimerRef.current = setTimeout(() => {
      setManualActive("");
    }, 2000);
    flashTimerRef.current = setTimeout(() => {
      setFlashSection("");
    }, 1600);
  };

  const handleCreateProject = () => {
    if (createProjectState === "loading") {
      return;
    }
    setCreateProjectState("loading");
    if (createTimerRef.current.loading) {
      clearTimeout(createTimerRef.current.loading);
    }
    if (createTimerRef.current.success) {
      clearTimeout(createTimerRef.current.success);
    }
    createTimerRef.current.loading = setTimeout(() => {
      setCreateProjectState("success");
    }, 900);
    createTimerRef.current.success = setTimeout(() => {
      setCreateProjectState("idle");
    }, 2400);
  };

  useEffect(() => {
    return () => {
      if (createTimerRef.current.loading) {
        clearTimeout(createTimerRef.current.loading);
      }
      if (createTimerRef.current.success) {
        clearTimeout(createTimerRef.current.success);
      }
    };
  }, []);

  useEffect(() => {
    const handleScroll = () => {
      if (manualActive) {
        return;
      }
      const viewportMarker = window.innerHeight * 0.35;
      let current = sections[0]?.id ?? "";
      let fallback = sections[0]?.id ?? "";
      for (const section of sections) {
        const element = document.getElementById(section.id);
        if (!element) {
          continue;
        }
        const rect = element.getBoundingClientRect();
        if (rect.top <= viewportMarker) {
          fallback = section.id;
        }
        if (rect.top <= viewportMarker && rect.bottom > viewportMarker) {
          current = section.id;
          break;
        }
      }
      setActiveSection(current || fallback);
    };

    handleScroll();
    window.addEventListener("scroll", handleScroll, { passive: true });
    window.addEventListener("resize", handleScroll);
    return () => {
      window.removeEventListener("scroll", handleScroll);
      window.removeEventListener("resize", handleScroll);
    };
  }, [manualActive]);

  const connectedAgents = liveAgents.filter(
    (agent) => agent.status === "online"
  ).length;

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
    return metric;
  });

  const loadPercent = Math.min(
    100,
    Math.round(((liveLoad.running + liveLoad.queued) / 20) * 100)
  );

  const taskRows = useMemo(() => {
    return liveTasks.map((task, index) => {
      const shard = (index % 4) + 1;
      return {
        ...task,
        project: projects[index % projects.length]?.name ?? "Demo",
        agent: liveAgents[index % liveAgents.length]?.id ?? "auto",
        progress: task.status === "done" ? 100 : task.status === "running" ? 62 : 18,
        started: "2m ago",
        completed: task.status === "done" ? "just now" : "-",
        shard
      };
    });
  }, [liveTasks, liveAgents]);

  const filteredTasks = useMemo(() => {
    const normalizedQuery = taskQuery.trim().toLowerCase();
    return taskRows.filter((task) => {
      const matchesQuery =
        !normalizedQuery ||
        task.id.toLowerCase().includes(normalizedQuery) ||
        task.project.toLowerCase().includes(normalizedQuery);
      const matchesStatus =
        taskStatus === "all" ? true : task.status === taskStatus;
      return matchesQuery && matchesStatus;
    });
  }, [taskQuery, taskRows, taskStatus]);

  const visibleTaskRows = filteredTasks.slice(0, visibleTasks);

  const filteredLogs = useMemo(() => {
    return logSeed.filter((entry) => logFilters[entry.level]);
  }, [logFilters]);

  const visibleLogRows = filteredLogs.slice(0, visibleLogs);

  const toggleLogFilter = (level) => {
    setLogFilters((prev) => ({
      ...prev,
      [level]: !prev[level]
    }));
  };

  const agentLoad = (id) => {
    let hash = 0;
    for (let i = 0; i < id.length; i += 1) {
      hash = (hash * 31 + id.charCodeAt(i)) % 100;
    }
    return 20 + (hash % 70);
  };

  const toggleTheme = () => {
    setTheme((prev) => (prev === "dark" ? "light" : "dark"));
  };

  const scrollToTop = () => {
    window.scrollTo({ top: 0, behavior: "smooth" });
  };

  return (
    <div className="app">
      <aside className="sidebar">
        <div className="brand">
          <img src="/newral_big_logo.png" alt="Newral" className="logo" />
          <div className="brand-copy">
            <p className="eyebrow">Newral Portal</p>
            <h1>Admin Console</h1>
          </div>
        </div>
        <nav className="nav-links">
          {sections.map((section) => (
            <a
              key={section.id}
              href={`#${section.id}`}
              onClick={() => handleNavClick(section.id)}
              className={activeSection === section.id ? "active" : ""}
            >
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
            <button
              className={`btn primary create-btn ${createProjectState}`}
              onClick={handleCreateProject}
            >
              {createProjectState === "loading"
                ? "Creating..."
                : createProjectState === "success"
                ? "Created"
                : "Create project"}
            </button>
            <button className="btn ghost" onClick={toggleTheme}>
              Theme: {theme === "dark" ? "Dark" : "Light"}
            </button>
            <button className="btn" disabled>
              Sign in
            </button>
          </div>
        </header>

        <main className="content">
          <section
            id="dashboard"
            className={`section${
              flashSection === "dashboard" ? " flash" : ""
            }`}
          >
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
                    <div
                      key={metric.label}
                      className="metric-card skeleton-card"
                    >
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
              <div className="chart-card">
                <div className="chart-header">
                  <h3>Storage IO</h3>
                  <span>MinIO</span>
                </div>
                <div className="chart-surface tertiary" aria-hidden="true"></div>
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
          </section>

          <section
            id="demo"
            className={`section${flashSection === "demo" ? " flash" : ""}`}
          >
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

          <section
            id="agents"
            className={`section${
              flashSection === "agents" ? " flash" : ""
            }`}
          >
            <div className="section-header">
              <div>
                <h2>Agents</h2>
                <p>Live status for connected compute nodes.</p>
              </div>
              <div className="chip">Auto heartbeat</div>
            </div>
            <div className="table-card table-scroll">
              <table>
                <thead>
                  <tr>
                    <th>Node</th>
                    <th>Status</th>
                    <th>Region</th>
                    <th>Load</th>
                    <th>Reputation</th>
                    <th>Version</th>
                  </tr>
                </thead>
                <tbody>
                  {summaryLoaded && liveAgents.length === 0 ? (
                    <tr>
                      <td colSpan="6" className="muted">
                        No agents connected.
                      </td>
                    </tr>
                  ) : summaryLoaded ? (
                    liveAgents.map((agent) => (
                      <tr key={agent.id}>
                        <td>{agent.id}</td>
                        <td>
                          <span
                            className={`status-dot ${agent.status}`}
                            title={agent.status}
                          ></span>
                          <span className={`status-label ${agent.status}`}>
                            {agent.status}
                          </span>
                        </td>
                        <td>{agent.region}</td>
                        <td>
                          <div className="mini-bar">
                            <div
                              style={{ width: `${agentLoad(agent.id)}%` }}
                            ></div>
                          </div>
                        </td>
                        <td>{agent.reputation}</td>
                        <td>v0.4.2</td>
                      </tr>
                    ))
                  ) : (
                    Array.from({ length: 4 }).map((_, index) => (
                      <tr key={`agent-skel-${index}`}>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
            <div className="card-grid">
              <div className="stat-card">
                <h3>Node Control</h3>
                <p className="muted">
                  Promote or pause agents during maintenance windows.
                </p>
                <div className="pill-row">
                  <button className="btn ghost">Pause all</button>
                  <button className="btn ghost">Resume all</button>
                </div>
              </div>
              <div className="stat-card">
                <h3>Health Alerts</h3>
                <p className="muted">Last alert: heartbeat drift on node-27.</p>
                <button className="btn ghost">Review alerts</button>
              </div>
            </div>
          </section>

          <section
            id="tasks"
            className={`section${flashSection === "tasks" ? " flash" : ""}`}
          >
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
                  onChange={(event) => setTaskQuery(event.target.value)}
                />
                <select
                  className="input"
                  value={taskStatus}
                  onChange={(event) => setTaskStatus(event.target.value)}
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
                    <th>Progress</th>
                    <th>Started</th>
                    <th>Completed</th>
                  </tr>
                </thead>
                <tbody>
                  {summaryLoaded && visibleTaskRows.length === 0 ? (
                    <tr>
                      <td colSpan="7" className="muted">
                        No tasks matching the filter.
                      </td>
                    </tr>
                  ) : summaryLoaded ? (
                    visibleTaskRows.map((task) => (
                      <tr key={task.id}>
                        <td>{task.id}</td>
                        <td>{task.project}</td>
                        <td>{task.agent}</td>
                        <td>
                          <span className={`status-label ${task.status}`}>
                            {task.status}
                          </span>
                        </td>
                        <td>
                          <div className="mini-bar">
                            <div style={{ width: `${task.progress}%` }}></div>
                          </div>
                        </td>
                        <td>{task.started}</td>
                        <td>{task.completed}</td>
                      </tr>
                    ))
                  ) : (
                    Array.from({ length: 5 }).map((_, index) => (
                      <tr key={`task-skel-${index}`}>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                        <td className="skeleton-line"></td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
            {filteredTasks.length > visibleTaskRows.length && (
              <button
                className="btn ghost"
                onClick={() => setVisibleTasks((prev) => prev + 6)}
              >
                Load more
              </button>
            )}
          </section>

          <section
            id="projects"
            className={`section${
              flashSection === "projects" ? " flash" : ""
            }`}
          >
            <div className="section-header">
              <div>
                <h2>Projects</h2>
                <p>Portfolio view for active and staged workloads.</p>
              </div>
              <button
                className={`btn primary create-btn ${createProjectState}`}
                onClick={handleCreateProject}
              >
                {createProjectState === "loading"
                  ? "Creating..."
                  : createProjectState === "success"
                  ? "Created"
                  : "Create project"}
              </button>
            </div>
            <div className="table-card">
              <table>
                <thead>
                  <tr>
                    <th>Project</th>
                    <th>Status</th>
                    <th>Owner</th>
                    <th>Progress</th>
                  </tr>
                </thead>
                <tbody>
                  {projects.map((project) => (
                    <tr key={project.name}>
                      <td>{project.name}</td>
                      <td className="status-label">{project.status}</td>
                      <td>{project.owner}</td>
                      <td>
                        <div className="mini-bar">
                          <div style={{ width: `${project.progress}%` }}></div>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </section>

          <section
            id="ai-mode"
            className={`section${
              flashSection === "ai-mode" ? " flash" : ""
            }`}
          >
            <div className="section-header">
              <div>
                <h2>AI Mode</h2>
                <p>Governance settings with policy enforcement.</p>
              </div>
              <div className="chip">Policy engine active</div>
            </div>
            <div className="card-grid">
              <div
                className={`mode-card${
                  liveAiMode === "AI_OFF" ? " active" : ""
                }`}
              >
                <h3>AI_OFF</h3>
                <p>Deterministic scheduling only, no AI proposals.</p>
              </div>
              <div
                className={`mode-card${
                  liveAiMode === "AI_ADVISORY" ? " active" : ""
                }`}
              >
                <h3>AI_ADVISORY</h3>
                <p>AI suggests actions, policy gate still decides.</p>
              </div>
              <div
                className={`mode-card${
                  liveAiMode === "AI_ASSISTED" ? " active" : ""
                }`}
              >
                <h3>AI_ASSISTED</h3>
                <p>Low-risk actions automated, critical tasks gated.</p>
              </div>
              <div
                className={`mode-card${
                  liveAiMode === "AI_FULL" ? " active" : ""
                }`}
              >
                <h3>AI_FULL</h3>
                <p>End-to-end AI orchestration with hard limits.</p>
              </div>
            </div>
          </section>

          <section
            id="log-center"
            className={`section${
              flashSection === "log-center" ? " flash" : ""
            }`}
          >
            <div className="section-header">
              <div>
                <h2>Log Center</h2>
                <p>Audit trail across agents, sandboxes, and services.</p>
              </div>
              <div className="pill-row">
                <button
                  className={`chip ${logFilters.info ? "active" : ""}`}
                  onClick={() => toggleLogFilter("info")}
                  aria-pressed={logFilters.info}
                >
                  Info
                </button>
                <button
                  className={`chip ${logFilters.success ? "active" : ""}`}
                  onClick={() => toggleLogFilter("success")}
                  aria-pressed={logFilters.success}
                >
                  Success
                </button>
                <button
                  className={`chip ${logFilters.warn ? "active" : ""}`}
                  onClick={() => toggleLogFilter("warn")}
                  aria-pressed={logFilters.warn}
                >
                  Warn
                </button>
                <button
                  className={`chip ${logFilters.error ? "active" : ""}`}
                  onClick={() => toggleLogFilter("error")}
                  aria-pressed={logFilters.error}
                >
                  Error
                </button>
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
                  {visibleLogRows.map((entry) => (
                    <tr key={entry.id} className={`log-row ${entry.level}`}>
                      <td>
                        <span className={`log-pill ${entry.level}`}>
                          {entry.level}
                        </span>
                      </td>
                      <td>{entry.source}</td>
                      <td>{entry.message}</td>
                      <td>{entry.time}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
            {filteredLogs.length > visibleLogRows.length && (
              <button
                className="btn ghost"
                onClick={() => setVisibleLogs((prev) => prev + 6)}
              >
                Load more
              </button>
            )}
          </section>

          <section
            id="settings"
            className={`section${
              flashSection === "settings" ? " flash" : ""
            }`}
          >
            <div className="section-header">
              <div>
                <h2>Settings</h2>
                <p>Organization profile, security, and access control.</p>
              </div>
              <div className="chip">Role-based access</div>
            </div>
            <div className="tab-row">
              <button
                className={`tab ${settingsTab === "general" ? "active" : ""}`}
                onClick={() => setSettingsTab("general")}
              >
                General
              </button>
              <button
                className={`tab ${settingsTab === "security" ? "active" : ""}`}
                onClick={() => setSettingsTab("security")}
              >
                Security
              </button>
              <button
                className={`tab ${settingsTab === "access" ? "active" : ""}`}
                onClick={() => setSettingsTab("access")}
              >
                Access
              </button>
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

          <section
            id="integrations"
            className={`section${
              flashSection === "integrations" ? " flash" : ""
            }`}
          >
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
                  <span className={`pill ${integration.status}`}>
                    {integration.status}
                  </span>
                </div>
              ))}
            </div>
          </section>

          <section
            id="system-health"
            className={`section${
              flashSection === "system-health" ? " flash" : ""
            }`}
          >
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
                    <span
                      className={`pill ${
                        healthLoaded ? status : "loading"
                      }`}
                    >
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
        </main>
      </div>

      <div className="floating-actions">
        <button
          className={`scroll-top ${showScrollTop ? "show" : ""}`}
          onClick={scrollToTop}
          aria-label="Scroll to top"
        >
          <svg viewBox="0 0 48 48" className="progress-ring">
            <circle className="ring-bg" cx="24" cy="24" r="20" />
            <circle
              className="ring"
              cx="24"
              cy="24"
              r="20"
              style={{
                strokeDashoffset: `${
                  125.6 - 125.6 * scrollProgress
                }px`
              }}
            />
          </svg>
          <span>Top</span>
        </button>
      </div>
    </div>
  );
}

export default App;
