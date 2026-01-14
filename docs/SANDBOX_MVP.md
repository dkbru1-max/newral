Sandbox MVP

Scope
The MVP sandbox executes Python tasks inside a temporary workspace directory with basic guardrails. It is designed for safety and resource control, not for hardened isolation.

Guarantees (MVP)
- Each task runs in a dedicated workspace under the OS temp directory.
- Script and input files are stored only inside the workspace.
- Hard timeout terminates execution.
- Workspace size is monitored and execution is terminated on limit breach.
- Stdout/stderr output is size-limited to prevent agent overload.
- Process priority is lowered on Linux via `nice`.

Non-guarantees (MVP)
- No strong OS-level isolation (no containers, no seccomp, no VM).
- CPU usage is not strictly limited; only a placeholder monitor exists.
- No network sandboxing beyond standard OS policies.
- No protection against malicious Python code beyond file/path constraints.

Future hardening
- Run tasks in containers or microVMs with strict resource quotas.
- Add syscall filtering (seccomp) or OS sandbox APIs.
- Enforce CPU and memory limits via cgroups or job objects.
- Dedicated per-project sandboxes and signed task bundles.
