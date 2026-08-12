export interface Partition {
  name: string;
  start: number;
  size: number;
  content: string | null;
}

export interface Device {
  device_path: string;
  display_name: string;
  vendor: string | null;
  usb_product_name: string | null;
  size_bytes: number;
  removable: boolean;
  is_system: boolean;
  partitions: Partition[];
}

export type TaskStage = "queued" | "preparing" | "flashing" | "verifying" | "exporting" | "done" | "error" | "cancelled";

export interface ProgressPayload {
  task_id: number;
  stage: TaskStage;
  percent: number;
  speed_mbps: number;
  eta_seconds: number;
  bytes_done: number;
  bytes_total: number;
  message?: string;
}

export interface TaskInfo {
  task_id: number;
  device_path: string;
  device_name: string;
  image_name?: string;
  mode: "flash" | "export";
  stage: TaskStage;
  percent: number;
  speed_mbps: number;
  eta_seconds: number;
  error?: string;
  started_at?: number;
  finished_at?: number;
}

export interface ExportRange {
  start: number;
  length: number;
}

export interface FlashStartRequest {
  mode: "flash" | "export";
  image_path?: string;
  device_paths: string[];
  export_range?: ExportRange | null;
}
