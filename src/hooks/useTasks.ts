import { useEffect, useState, useCallback, useRef } from "react";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import type { Device, TaskInfo, ProgressPayload } from "../types";

export function useDeviceList() {
  const [devices, setDevices] = useState<Device[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await invoke<Device[]>("list_devices");
      setDevices(list);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { devices, loading, error, refresh };
}

export function useTasks() {
  const [tasks, setTasks] = useState<TaskInfo[]>([]);
  const tasksRef = useRef<TaskInfo[]>([]);
  // 事件缓冲：Rust 侧事件可能先于前端任务卡片创建到达，先暂存，卡片创建后立即应用
  const eventBufferRef = useRef<Map<number, ProgressPayload>>(new Map());

  const mergePayload = useCallback((t: TaskInfo, p: ProgressPayload): TaskInfo => {
    const active = ["flashing", "verifying", "exporting", "preparing"].includes(p.stage);
    const finished = ["done", "error", "cancelled"].includes(p.stage);
    return {
      ...t,
      ...p,
      started_at: t.started_at ?? (active ? Date.now() : undefined),
      finished_at: t.finished_at ?? (finished ? Date.now() : undefined),
    };
  }, []);

  const updateTask = useCallback(
    (payload: ProgressPayload) => {
      setTasks((prev) => {
        const idx = prev.findIndex((t) => t.task_id === payload.task_id);
        if (idx === -1) {
          // 卡片尚未创建，先缓存事件
          eventBufferRef.current.set(payload.task_id, payload);
          return prev;
        }
        const next = [...prev];
        next[idx] = mergePayload(prev[idx], payload);
        tasksRef.current = next;
        return next;
      });
    },
    [mergePayload]
  );

  useEffect(() => {
    let unlisten: UnlistenFn | undefined;
    (async () => {
      unlisten = await listen<ProgressPayload>("flash:progress", (e) => {
        updateTask(e.payload);
      });
    })();
    return () => {
      unlisten?.();
    };
  }, [updateTask]);

  const appendTask = useCallback(
    (task: TaskInfo) => {
      setTasks((prev) => {
        const buffered = eventBufferRef.current.get(task.task_id);
        eventBufferRef.current.delete(task.task_id);
        const next = [...prev, buffered ? mergePayload(task, buffered) : task];
        tasksRef.current = next;
        return next;
      });
    },
    [mergePayload]
  );

  const removeTasksForDevice = useCallback((devicePath: string) => {
    setTasks((prev) => {
      const next = prev.filter((t) => t.device_path !== devicePath);
      tasksRef.current = next;
      return next;
    });
  }, []);

  const clearFinishedTasks = useCallback(() => {
    setTasks((prev) => {
      const next = prev.filter(
        (t) => !["done", "error", "cancelled"].includes(t.stage)
      );
      tasksRef.current = next;
      return next;
    });
  }, []);

  return { tasks, appendTask, updateTask, removeTasksForDevice, clearFinishedTasks };
}
