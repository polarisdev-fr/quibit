import { useState, type ChangeEvent, type FormEvent } from "react";
import {
  getVmStatus,
  startVm,
  vmApiErrorMessage,
  type QemuConfig,
  type VmStatus,
} from "./lib/vmApi";
import "./App.css";

interface AppProps {
  appName: string;
}

type Feedback =
  | { kind: "success"; message: string }
  | { kind: "error"; message: string }
  | null;

const initialConfig: QemuConfig = {
  machine: "virt",
  accelerator: "hvf",
  cpu: "host",
  memoryMb: 512,
};

function App({ appName }: AppProps) {
  const [vmId, setVmId] = useState("vm-1");
  const [config, setConfig] = useState<QemuConfig>(initialConfig);
  const [status, setStatus] = useState<VmStatus | null>(null);
  const [feedback, setFeedback] = useState<Feedback>(null);
  const [pendingAction, setPendingAction] = useState<"start" | "status" | null>(
    null,
  );
  const isPending = pendingAction !== null;
  const canSubmit = vmId.trim().length > 0 && !isPending;

  function updateConfig(
    field: keyof QemuConfig,
    value: string | number,
  ): void {
    setConfig((current) => ({ ...current, [field]: value }));
  }

  async function handleStart(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault();
    if (!canSubmit) {
      return;
    }

    setPendingAction("start");
    setFeedback(null);
    setStatus(null);
    try {
      await startVm(vmId.trim(), config);
      setFeedback({ kind: "success", message: `VM "${vmId.trim()}" started.` });
    } catch (error: unknown) {
      setFeedback({ kind: "error", message: vmApiErrorMessage(error) });
    } finally {
      setPendingAction(null);
    }
  }

  async function handleStatus(): Promise<void> {
    if (!canSubmit) {
      return;
    }

    setPendingAction("status");
    setFeedback(null);
    setStatus(null);
    try {
      const result = await getVmStatus(vmId.trim());
      setStatus(result);
    } catch (error: unknown) {
      setFeedback({ kind: "error", message: vmApiErrorMessage(error) });
    } finally {
      setPendingAction(null);
    }
  }

  function handleTextChange(
    field: "machine" | "accelerator" | "cpu",
    event: ChangeEvent<HTMLInputElement>,
  ): void {
    updateConfig(field, event.currentTarget.value);
  }

  return (
    <main className="app-shell">
      <header className="topbar">
        <div className="brand-mark" aria-hidden="true">
          N
        </div>
        <div className="topbar-title">
          <span className="eyebrow">Virtualization</span>
          <h1>{appName}</h1>
        </div>
        <span className="environment-tag">Local host</span>
      </header>

      <div className="workspace">
        <aside className="sidebar" aria-label="Navigation">
          <span className="sidebar-heading">Datacenter</span>
          <div className="tree-item tree-root">
            <span className="tree-chevron" aria-hidden="true">
              ▾
            </span>
            <span className="host-icon" aria-hidden="true">
              ▦
            </span>
            <span>nimbus</span>
          </div>
          <div className="tree-item tree-child" aria-current="page">
            <span className="tree-dot" aria-hidden="true" />
            <span>VM test bench</span>
          </div>
        </aside>

        <section className="main-panel">
          <div className="page-heading">
            <div>
              <p className="eyebrow">Virtual machines</p>
              <h2>QEMU test bench</h2>
              <p className="page-description">
                Start a local VM and inspect its state through QMP.
              </p>
            </div>
            <span className="status-chip">
              <span className="status-dot" aria-hidden="true" />
              Backend controls
            </span>
          </div>

          <form className="vm-card" onSubmit={handleStart}>
            <div className="card-heading">
              <div>
                <h3>Virtual machine configuration</h3>
                <p>Configure the QEMU process for this test session.</p>
              </div>
              <span className="card-index">01</span>
            </div>

            <div className="field-grid">
              <label className="field field-wide">
                <span>VM ID</span>
                <input
                  autoComplete="off"
                  disabled={isPending}
                  onChange={(event) => setVmId(event.currentTarget.value)}
                  placeholder="e.g. vm-1"
                  required
                  value={vmId}
                />
              </label>

              <label className="field">
                <span>Machine</span>
                <input
                  disabled={isPending}
                  onChange={(event) => handleTextChange("machine", event)}
                  required
                  value={config.machine}
                />
              </label>

              <label className="field">
                <span>Accelerator</span>
                <input
                  disabled={isPending}
                  onChange={(event) => handleTextChange("accelerator", event)}
                  required
                  value={config.accelerator}
                />
              </label>

              <label className="field">
                <span>CPU</span>
                <input
                  disabled={isPending}
                  onChange={(event) => handleTextChange("cpu", event)}
                  required
                  value={config.cpu}
                />
              </label>

              <label className="field">
                <span>Memory (MiB)</span>
                <input
                  disabled={isPending}
                  max={65535}
                  min={1}
                  onChange={(event) =>
                    updateConfig("memoryMb", Number(event.currentTarget.value))
                  }
                  required
                  type="number"
                  value={config.memoryMb}
                />
              </label>
            </div>

            <div className="card-actions">
              <button
                className="button button-primary"
                disabled={!canSubmit}
                type="submit"
              >
                {pendingAction === "start" ? "Starting…" : "Start VM"}
              </button>
              <button
                className="button button-secondary"
                disabled={!canSubmit}
                onClick={handleStatus}
                type="button"
              >
                {pendingAction === "status" ? "Checking…" : "Get status"}
              </button>
            </div>
          </form>

          <section className="result-card" aria-live="polite">
            <div className="card-heading">
              <div>
                <h3>VM status</h3>
                <p>Latest response from the QMP status query.</p>
              </div>
              <span className="card-index">02</span>
            </div>
            {feedback ? (
              <div className={`feedback feedback-${feedback.kind}`} role="status">
                <span className="feedback-icon" aria-hidden="true">
                  {feedback.kind === "error" ? "!" : "✓"}
                </span>
                <span>{feedback.message}</span>
              </div>
            ) : status ? (
              <div className="status-result">
                <div>
                  <span className="result-label">State</span>
                  <strong>{status.status}</strong>
                </div>
                <div>
                  <span className="result-label">Running</span>
                  <strong>{status.running ? "Yes" : "No"}</strong>
                </div>
                <span
                  className={`running-indicator${status.running ? " is-running" : ""}`}
                  aria-label={status.running ? "VM is running" : "VM is stopped"}
                />
              </div>
            ) : (
              <p className="empty-state">
                No response yet. Start a VM or request its status.
              </p>
            )}
          </section>

          <p className="footnote">
            QEMU runs locally on this Mac. Closing the application ends managed
            VM processes.
          </p>
        </section>
      </div>
    </main>
  );
}

export default App;
