import { useState, useCallback, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { confirm } from "@tauri-apps/plugin-dialog";
import DeviceList from "./components/DeviceList";
import ImageSelector, { type ImageInfo } from "./components/ImageSelector";
import { AboutDialog, SettingsDialog } from "./components/Dialogs";
import { useDeviceList, useTasks } from "./hooks/useTasks";
import { useI18n } from "./i18n";
import type { TaskInfo } from "./types";

function formatSize(bytes: number): string {
  if (!bytes) return "未知";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

type Mode = "flash" | "export";

export default function App() {
  const { t } = useI18n();
  const [mode, setMode] = useState<Mode>("flash");
  const [image, setImage] = useState<ImageInfo | null>(null);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [running, setRunning] = useState(false);
  const [exportMode, setExportMode] = useState<"range" | "full" | "custom">("range");
  const [customSizeMB, setCustomSizeMB] = useState(1024);
  const [showAbout, setShowAbout] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const { devices, loading, error, refresh } = useDeviceList();
  const { tasks, appendTask, removeTasksForDevice, clearFinishedTasks } = useTasks();

  const refreshDevices = useCallback(() => {
    clearFinishedTasks();
    refresh();
  }, [clearFinishedTasks, refresh]);

  const toggle = useCallback(
    (path: string) => {
      setSelected((prev) => {
        if (mode === "export") {
          // 导出模式单选
          if (prev.has(path)) return new Set();
          return new Set([path]);
        }
        const next = new Set(prev);
        if (next.has(path)) next.delete(path);
        else next.add(path);
        return next;
      });
      removeTasksForDevice(path);
    },
    [mode, removeTasksForDevice]
  );

  // 任务结束后自动取消勾选对应设备（error/cancelled 也取消，方便用户重新操作）
  useEffect(() => {
    const finished = tasks.filter((t) =>
      ["done", "error", "cancelled"].includes(t.stage)
    );
    if (finished.length === 0) return;
    setSelected((prev) => {
      const next = new Set(prev);
      let changed = false;
      for (const f of finished) {
        if (next.delete(f.device_path)) changed = true;
      }
      return changed ? next : prev;
    });
  }, [tasks]);

  useEffect(() => {
    if (
      tasks.length > 0 &&
      tasks.every(
        (t) => t.stage === "done" || t.stage === "error" || t.stage === "cancelled"
      )
    ) {
      setRunning(false);
    }
  }, [tasks]);

  const cancel = useCallback(async () => {
    try {
      await invoke("cancel_tasks", { taskIds: tasks.map((t) => t.task_id) });
    } catch (e) {
      await confirm(String(e), {
        title: t("settings.title"),
        kind: "error",
        okLabel: "OK",
      });
    }
  }, [tasks, t]);

  const canStart = !running && selected.size > 0 && image !== null;

  // 导出范围：有分区表时导出 [0, 最后一个分区末尾]（含分区表），否则导出整卡
  const calcExportRange = useCallback(
    (path: string): { start: number; length: number } | null => {
      const dev = devices.find((d) => d.device_path === path);
      if (!dev || dev.partitions.length === 0) return null;
      const maxEnd = Math.max(...dev.partitions.map((p) => p.start + p.size));
      if (maxEnd <= 0) return null;
      return { start: 0, length: maxEnd };
    },
    [devices]
  );

  const start = useCallback(async () => {
    if (!canStart) return;
    const paths = [...selected];
    const hasParts =
      devices.find((d) => d.device_path === paths[0])?.partitions.length ?? 0;
    const effectiveExportMode =
      mode === "export" && hasParts > 0
        ? exportMode
        : exportMode === "custom"
          ? "custom"
          : "full";
    const exportRange =
      mode === "export" && paths.length === 1
        ? effectiveExportMode === "range"
          ? calcExportRange(paths[0])
          : effectiveExportMode === "custom"
            ? { start: 0, length: customSizeMB * 1_000_000 }
            : null
        : null;

    try {
      await invoke<boolean>("ensure_privileges");
    } catch (e) {
      await confirm(String(e), {
        title: t("settings.title"),
        kind: "error",
        okLabel: "OK",
      });
      return;
    }

    const ok = await confirm(
      mode === "flash"
        ? t("confirm.flash.msg", {
            n: paths.length,
            paths: paths.join("\n"),
          })
        : t("confirm.export.msg", { paths: paths.join("\n") }),
      {
        title: t("confirm.title"),
        kind: "warning",
        okLabel: t("confirm.ok"),
        cancelLabel: t("confirm.cancel"),
      }
    );
    if (!ok) return;

    setRunning(true);
    try {
      const ids = await invoke<number[]>("start_task", {
        request: {
          mode,
          image_path: image ? image.path : null,
          device_paths: paths,
          export_range: exportRange,
          concurrency: Number(localStorage.getItem("concurrency")) || 3,
        },
      });
      const ts = Date.now();
      paths.forEach((p, i) => {
        const dev = devices.find((d) => d.device_path === p);
        const task: TaskInfo = {
          task_id: ids[i] ?? ts + i,
          device_path: p,
          device_name: dev?.display_name ?? p,
          image_name: mode === "flash" ? image?.name : "export.img",
          mode,
          stage: "queued",
          percent: 0,
          speed_mbps: 0,
          eta_seconds: 0,
        };
        appendTask(task);
      });
    } catch (e) {
      setRunning(false);
      await confirm(String(e), {
        title: t("settings.title"),
        kind: "error",
        okLabel: "OK",
      });
    }
  }, [canStart, selected, mode, image, devices, appendTask, exportMode, customSizeMB, calcExportRange, t]);

  return (
    <div className="mx-auto flex h-full max-w-3xl flex-col gap-4 px-6 py-6">
      <header className="flex items-center gap-3">
        <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 text-lg font-bold text-white shadow-md">
          SD
        </div>
        <div className="flex-1">
          <h1 className="text-lg font-bold text-slate-800">SDCardImager</h1>
          <p className="text-xs text-slate-400">{t("app.subtitle")}</p>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            onClick={() => setShowSettings(true)}
            title="Settings"
            className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-slate-200 hover:text-slate-700"
          >
            <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="3" />
              <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
            </svg>
          </button>
          <button
            onClick={() => setShowAbout(true)}
            title="About"
            className="flex h-8 w-8 items-center justify-center rounded-lg text-slate-500 transition hover:bg-slate-200 hover:text-slate-700"
          >
            <svg viewBox="0 0 24 24" className="h-5 w-5" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round">
              <circle cx="12" cy="12" r="10" />
              <path d="M12 16v-4" />
              <path d="M12 8h.01" />
            </svg>
          </button>
        </div>
      </header>

      <div className="flex rounded-xl bg-slate-200/70 p-1">
        {(["flash", "export"] as Mode[]).map((m) => (
          <button
            key={m}
            onClick={() => setMode(m)}
            disabled={running}
            className={`flex-1 rounded-lg py-2 text-sm font-medium transition ${
              mode === m
                ? "bg-white text-slate-800 shadow"
                : "text-slate-500 hover:text-slate-700"
            } disabled:opacity-50`}
          >
            {m === "flash" ? t("mode.flash") : t("mode.export")}
          </button>
        ))}
      </div>

      <ImageSelector
        image={image}
        onSelect={setImage}
        mode={mode}
        disabled={running}
      />

      <DeviceList
        devices={devices}
        selected={selected}
        onToggle={toggle}
        onSelectAll={() => {
          clearFinishedTasks();
          if (selected.size === devices.length) setSelected(new Set());
          else setSelected(new Set(devices.map((d) => d.device_path)));
        }}
        onClear={() => setSelected(new Set())}
        loading={loading}
        error={error}
        onRefresh={refreshDevices}
        disabled={running}
        tasks={tasks}
        single={mode === "export"}
      />

      {!running && mode === "export" && selected.size === 1 && (
        (() => {
          const path = [...selected][0];
          const dev = devices.find((d) => d.device_path === path);
          const hasParts = (dev?.partitions.length ?? 0) > 0;
          const maxMB = Math.max(1, Math.ceil((dev?.size_bytes ?? 1) / 1_000_000));
          const clampedMB = Math.min(maxMB, Math.max(1, customSizeMB));
          const range = calcExportRange(path);
          const customLen = clampedMB * 1_000_000;
          const isRange = exportMode === "range" && range !== null;
          const isCustom = exportMode === "custom";
          return (
            <div className="flex flex-col gap-2">
              <div className="flex rounded-xl bg-slate-200/70 p-1">
                <button
                  onClick={() => setExportMode("range")}
                  disabled={!hasParts}
                  className={`flex-1 rounded-lg py-2 text-sm font-medium transition disabled:opacity-40 ${
                    exportMode === "range"
                      ? "bg-white text-slate-800 shadow"
                      : "text-slate-500 hover:text-slate-700"
                  }`}
                >
                  {t("export.range.only")}
                </button>
                <button
                  onClick={() => setExportMode("full")}
                  className={`flex-1 rounded-lg py-2 text-sm font-medium transition ${
                    exportMode === "full"
                      ? "bg-white text-slate-800 shadow"
                      : "text-slate-500 hover:text-slate-700"
                  }`}
                >
                  {t("export.full")}
                </button>
                <button
                  onClick={() => setExportMode("custom")}
                  className={`flex-1 rounded-lg py-2 text-sm font-medium transition ${
                    exportMode === "custom"
                      ? "bg-white text-slate-800 shadow"
                      : "text-slate-500 hover:text-slate-700"
                  }`}
                >
                  {t("export.custom")}
                </button>
              </div>

              {isCustom && (
                <div className="rounded-2xl bg-white px-5 py-4 shadow-sm ring-1 ring-slate-200">
                  <div className="flex items-center justify-between text-sm">
                    <span className="text-slate-600">{t("export.custom.from")}</span>
                    <div className="flex items-center gap-1.5">
                      <input
                        type="number"
                        min={1}
                        max={maxMB}
                        value={clampedMB}
                        onChange={(e) => {
                          const v = Number(e.target.value);
                          setCustomSizeMB(
                            Number.isFinite(v)
                              ? Math.min(maxMB, Math.max(1, v))
                              : 1
                          );
                        }}
                        className="w-24 rounded-lg border border-slate-300 px-2 py-1 text-right text-sm tabular-nums focus:border-blue-500 focus:outline-none"
                      />
                      <span className="text-slate-400">MB</span>
                    </div>
                  </div>
                  <input
                    type="range"
                    min={1}
                    max={maxMB}
                    value={clampedMB}
                    onChange={(e) => setCustomSizeMB(Number(e.target.value))}
                    className="mt-3 w-full accent-blue-600"
                  />
                  <div className="mt-1 flex justify-between text-xs text-slate-400">
                    <span>1 MB</span>
                    <span>{formatSize(dev?.size_bytes ?? 0)}</span>
                  </div>
                </div>
              )}

              <div className="rounded-2xl border border-blue-200 bg-blue-50 px-5 py-3 text-sm text-blue-700">
                {isCustom
                  ? t("export.custom.info", { size: formatSize(customLen) })
                  : isRange
                    ? t("export.range.info", {
                        size: formatSize(range.length),
                        n: dev?.partitions.length ?? 0,
                      })
                    : t(dev && dev.partitions.length > 0 ? "export.full.info" : "export.full.noparts", {
                        size: formatSize(dev?.size_bytes ?? 0),
                      })}
              </div>
            </div>
          );
        })()
      )}

      {!running && mode === "flash" && selected.size > 0 && (
        <div className="rounded-2xl border border-amber-200 bg-amber-50 px-5 py-3 text-sm text-amber-700">
          {t("warn.overwrite", { n: selected.size })}
        </div>
      )}

      <div className="mt-auto pt-2">
        {running ? (
          <button
            onClick={cancel}
            className="w-full rounded-2xl py-3.5 text-base font-semibold text-white shadow-lg shadow-red-500/30 transition bg-gradient-to-r from-red-500 to-rose-600 hover:opacity-90 active:scale-[0.99]"
          >
            {mode === "flash" ? t("btn.cancel.flash") : t("btn.cancel.export")}
          </button>
        ) : (
          <button
            onClick={start}
            disabled={!canStart}
            className={`w-full rounded-2xl py-3.5 text-base font-semibold text-white shadow-lg transition ${
              canStart
                ? "bg-gradient-to-r from-blue-500 to-indigo-600 shadow-blue-500/30 hover:opacity-90 active:scale-[0.99]"
                : "cursor-not-allowed bg-slate-300 shadow-none"
            }`}
          >
            {mode === "flash" ? t("btn.start.flash") : t("btn.start.export")}
          </button>
        )}
      </div>

      {showAbout && <AboutDialog onClose={() => setShowAbout(false)} />}
      {showSettings && <SettingsDialog onClose={() => setShowSettings(false)} />}
    </div>
  );
}
