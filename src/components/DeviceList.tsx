import type { Device, TaskInfo } from "../types";
import { useI18n } from "../i18n";

const PART_COLORS = [
  "#3b82f6",
  "#8b5cf6",
  "#10b981",
  "#f59e0b",
  "#f43f5e",
  "#06b6d4",
  "#84cc16",
  "#ec4899",
];

function formatSize(bytes: number): string {
  if (bytes === 0) return "0";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

function formatDuration(ms?: number): string {
  if (!ms || ms < 0) return "—";
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  if (m > 0) return `${m}m${(s % 60).toString().padStart(2, "0")}s`;
  return `${s}s`;
}

function formatEta(secs: number): string {
  if (secs <= 0 || !isFinite(secs)) return "—";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return m > 0 ? `${m}m${s.toString().padStart(2, "0")}s` : `${s}s`;
}

const ACTIVE_STAGES = ["preparing", "flashing", "verifying", "exporting"];
const FINISHED_STAGES = ["done", "error", "cancelled"];

function StageInfo({ task }: { task?: TaskInfo }) {
  const { t } = useI18n();
  if (!task) return null;

  if (task.stage === "queued") {
    return (
      <div className="rounded-lg bg-slate-100 px-2.5 py-1 text-xs text-slate-500">
        {t("stage.queued")}
      </div>
    );
  }

  if (task.stage === "preparing") {
    return (
      <div className="text-right">
        <div className="text-xl font-bold tabular-nums text-amber-600">
          {Math.round(task.percent)}%
        </div>
        <div className="text-[10px] text-slate-400">{t("stage.preparing")}</div>
      </div>
    );
  }

  if (task.stage === "flashing" || task.stage === "exporting") {
    const label =
      task.stage === "flashing" ? t("stage.flashing") : t("stage.exporting");
    return (
      <div className="text-right">
        <div className="text-xl font-bold tabular-nums text-blue-600">
          {Math.round(task.percent)}%
        </div>
        <div className="text-[10px] tabular-nums text-slate-400">
          {t("speed.eta", {
            s: task.speed_mbps.toFixed(1),
            e: formatEta(task.eta_seconds),
          })}
        </div>
        <div className="text-[10px] text-blue-400">{label}</div>
      </div>
    );
  }

  if (task.stage === "verifying") {
    return (
      <div className="text-right">
        <div className="text-xl font-bold tabular-nums text-violet-600">
          {Math.round(task.percent)}%
        </div>
        <div className="text-[10px] tabular-nums text-slate-400">
          {t("speed.eta", {
            s: task.speed_mbps.toFixed(1),
            e: formatEta(task.eta_seconds),
          })}
        </div>
        <div className="text-[10px] text-violet-400">{t("stage.verifying")}</div>
      </div>
    );
  }

  if (task.stage === "done") {
    return (
      <div className="text-right">
        <div className="text-sm font-bold text-emerald-600">{t("stage.done")}</div>
        <div className="text-[10px] tabular-nums text-slate-400">
          {t("duration", {
            d: formatDuration(
              task.finished_at
                ? task.finished_at - (task.started_at ?? task.finished_at)
                : undefined
            ),
          })}
        </div>
      </div>
    );
  }

  if (task.stage === "error") {
    return (
      <div className="max-w-[140px] text-right">
        <div className="text-sm font-bold text-red-600">{t("stage.error")}</div>
        {task.error && (
          <div className="truncate text-[10px] text-red-400" title={task.error}>
            {task.error}
          </div>
        )}
      </div>
    );
  }

  if (task.stage === "cancelled") {
    return <div className="text-xs text-slate-400">{t("stage.cancelled")}</div>;
  }
  return null;
}

function PartitionBar({ device }: { device: Device }) {
  const parts = device.partitions;
  if (parts.length === 0) return null;
  const total = device.size_bytes || 1;
  return (
    <div className="relative mt-1.5 h-2 w-full max-w-[300px] rounded-full bg-slate-100">
      {parts.map((p, i) => (
        <div
          key={p.name}
          className="absolute top-0 h-full rounded-full"
          style={{
            left: `${Math.min(100, (p.start / total) * 100)}%`,
            width: `${Math.max(0.5, (p.size / total) * 100)}%`,
            backgroundColor: PART_COLORS[i % PART_COLORS.length],
          }}
          title={`${p.name} · ${p.content ?? ""} · ${formatSize(p.size)}`.trim()}
        />
      ))}
    </div>
  );
}

function PartitionInfo({ device }: { device: Device }) {
  const { t } = useI18n();
  if (device.partitions.length === 0) return null;
  return (
    <span className="group relative inline-flex items-center">
      <span>{t("partitions", { n: device.partitions.length })}</span>
      <span className="ml-1 flex h-3.5 w-3.5 cursor-help items-center justify-center rounded-full bg-slate-200 text-[9px] font-bold leading-none text-slate-500 transition group-hover:bg-blue-500 group-hover:text-white">
        i
      </span>
      <div className="pointer-events-none absolute left-full top-1/2 z-30 ml-2 hidden w-64 -translate-y-1/2 rounded-xl bg-slate-800/95 p-3 text-xs text-white shadow-xl backdrop-blur group-hover:block">
        {device.partitions.map((p, i) => (
          <div
            key={p.name}
            className="flex items-center justify-between gap-3 py-1"
          >
            <span className="flex min-w-0 items-center gap-1.5">
              <span
                className="h-2 w-2 shrink-0 rounded-full"
                style={{ backgroundColor: PART_COLORS[i % PART_COLORS.length] }}
              />
              <span className="truncate font-medium">{p.name}</span>
              {p.content && (
                <span className="shrink-0 rounded bg-white/15 px-1.5 py-0.5 text-[10px] text-slate-200">
                  {p.content}
                </span>
              )}
            </span>
            <span className="shrink-0 tabular-nums text-slate-300">
              {formatSize(p.size)}
            </span>
          </div>
        ))}
      </div>
    </span>
  );
}

interface Props {
  devices: Device[];
  selected: Set<string>;
  onToggle: (path: string) => void;
  onSelectAll: () => void;
  onClear: () => void;
  loading: boolean;
  error: string | null;
  onRefresh: () => void;
  disabled: boolean;
  tasks: TaskInfo[];
  single?: boolean;
}

export default function DeviceList({
  devices,
  selected,
  onToggle,
  onSelectAll,
  onClear,
  loading,
  error,
  onRefresh,
  disabled,
  tasks,
  single,
}: Props) {
  const { t } = useI18n();
  return (
    <div className="rounded-2xl bg-white p-5 shadow-sm ring-1 ring-slate-200">
      <div className="mb-3 flex items-center justify-between">
        <h2 className="text-sm font-semibold text-slate-700">{t("header.target")}</h2>
        <div className="flex items-center gap-2">
          <button
            onClick={onRefresh}
            disabled={loading}
            className="rounded-lg px-2.5 py-1 text-xs font-medium text-slate-500 transition hover:bg-slate-100 disabled:opacity-50"
          >
            {loading ? "…" : t("btn.refresh")}
          </button>
          {!single && (
            <>
              <button
                onClick={onSelectAll}
                disabled={disabled}
                className="rounded-lg px-2.5 py-1 text-xs font-medium text-slate-500 transition hover:bg-slate-100 disabled:opacity-50"
              >
                {t("btn.selectAll")}
              </button>
              <button
                onClick={onClear}
                disabled={disabled}
                className="rounded-lg px-2.5 py-1 text-xs font-medium text-slate-500 transition hover:bg-slate-100 disabled:opacity-50"
              >
                {t("btn.clear")}
              </button>
            </>
          )}
        </div>
      </div>

      {error && (
        <div className="mb-3 rounded-lg bg-red-50 px-3 py-2 text-xs text-red-600">{error}</div>
      )}
      {!error && devices.length === 0 && !loading && (
        <div className="rounded-lg border-2 border-dashed border-slate-200 py-8 text-center text-sm text-slate-400">
          {t("no.devices")}
        </div>
      )}

      <div className="max-h-[420px] space-y-1.5 overflow-y-auto pr-1">
        {devices.map((d) => {
          const checked = selected.has(d.device_path);
          const task = tasks.find((t) => t.device_path === d.device_path);
          const hasProgress = task !== undefined && !FINISHED_STAGES.includes(task.stage);
          const percent = Math.min(100, Math.max(0, task?.percent ?? 0));
          const taskActive = task !== undefined && ACTIVE_STAGES.includes(task.stage);
          const taskDone = task?.stage === "done";
          const taskError = task?.stage === "error";
          const stage = task?.stage === "verifying"
            ? "violet"
            : task?.stage === "preparing"
              ? "amber"
              : "blue";
          const barClass =
            taskDone
              ? "bg-emerald-500/15"
              : taskError
                ? "bg-red-500/10"
                : stage === "violet"
                  ? "bg-gradient-to-r from-violet-500/20 to-violet-500/35"
                  : stage === "amber"
                    ? "bg-gradient-to-r from-amber-500/20 to-amber-500/35"
                    : "bg-gradient-to-r from-blue-500/20 to-blue-500/35";
          const borderClass = taskDone
            ? "border-emerald-400 bg-emerald-50/40"
            : taskError
              ? "border-red-400 bg-red-50/40"
              : taskActive
                ? stage === "violet"
                  ? "border-violet-400 bg-violet-50/40"
                  : stage === "amber"
                    ? "border-amber-400 bg-amber-50/40"
                    : "border-blue-400 bg-blue-50/40"
                : checked
                  ? "border-blue-400 bg-blue-50/60"
                  : "border-slate-200 hover:border-slate-300 hover:bg-slate-50";

          return (
            <label
              key={d.device_path}
              className={`relative flex min-h-[88px] items-center gap-3 rounded-xl border px-4 py-4 transition ${borderClass}`}
            >
              {hasProgress && (
                <div
                  className={`absolute inset-y-0 left-0 rounded-l-xl transition-all duration-300 ${barClass}`}
                  style={{ width: `${percent}%` }}
                />
              )}

              <input
                type={single ? "radio" : "checkbox"}
                name={single ? "target-device" : undefined}
                checked={checked}
                onChange={() => onToggle(d.device_path)}
                disabled={disabled || taskActive}
                className="relative h-4 w-4 shrink-0 accent-blue-600"
              />
              <div className="relative min-w-0 flex-1">
                <div className="flex items-center gap-2">
                  <span className="truncate text-sm font-medium text-slate-800">
                    {d.display_name}
                  </span>
                  {d.vendor && (
                    <span className="shrink-0 rounded bg-slate-100 px-1.5 py-0.5 text-[10px] font-medium text-slate-500">
                      {d.vendor}
                    </span>
                  )}
                </div>
                <div className="mt-0.5 text-xs text-slate-400">
                  {d.device_path} · {formatSize(d.size_bytes)}
                  {d.partitions.length > 0 && (
                    <span className="ml-1 inline-flex items-center">
                      · <span className="ml-1"><PartitionInfo device={d} /></span>
                    </span>
                  )}
                </div>
                <PartitionBar device={d} />
              </div>
              <div className="relative flex h-14 w-32 shrink-0 items-center justify-end">
                <StageInfo task={task} />
              </div>
            </label>
          );
        })}
      </div>
    </div>
  );
}
