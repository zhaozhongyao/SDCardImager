import { useState, useCallback, useEffect } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { useI18n } from "../i18n";

export interface ImageInfo {
  path: string;
  name: string;
  size: number;
}

function formatSize(bytes: number): string {
  if (!bytes) return "0";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let v = bytes;
  let i = 0;
  while (v >= 1000 && i < units.length - 1) {
    v /= 1000;
    i++;
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${units[i]}`;
}

interface Props {
  image: ImageInfo | null;
  onSelect: (info: ImageInfo | null) => void;
  mode: "flash" | "export";
  disabled: boolean;
}

export default function ImageSelector({ image, onSelect, mode, disabled }: Props) {
  const { t } = useI18n();
  const [dragging, setDragging] = useState(false);

  useEffect(() => {
    if (mode !== "flash") return;
    const unlisten = getCurrentWebview().onDragDropEvent((event) => {
      if (event.payload.type === "drop" && event.payload.paths.length > 0) {
        const path = event.payload.paths[0];
        onSelect({
          path,
          name: path.split(/[\\/]/).pop() ?? path,
          size: 0,
        });
        invoke_get_size(path).then((meta) => {
          onSelect({
            path,
            name: path.split(/[\\/]/).pop() ?? path,
            size: meta,
          });
        });
      } else if (event.payload.type === "enter" || event.payload.type === "over") {
        setDragging(true);
      } else if (event.payload.type === "leave" || event.payload.type === "drop") {
        setDragging(false);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [mode, onSelect]);

  const browse = useCallback(async () => {
    if (mode === "export") {
      const result = await save({
        defaultPath: "export.img",
        filters: [{ name: "Image", extensions: ["img", "raw", "dmg", "bin"] }],
      });
      if (typeof result === "string") {
        onSelect({
          path: result,
          name: result.split(/[\\/]/).pop() ?? result,
          size: 0,
        });
      }
      return;
    }
    // 专家模式：不限制扩展名
    const expert = localStorage.getItem("expertMode") === "1";
    const result = await open({
      multiple: false,
      directory: false,
      filters: expert
        ? undefined
        : [{ name: "Disk Image", extensions: ["img", "iso", "raw", "dmg", "bin"] }],
    });
    if (typeof result === "string") {
      onSelect({
        path: result,
        name: result.split(/[\\/]/).pop() ?? result,
        size: 0,
      });
      const meta = await invoke_get_size(result);
      onSelect({
        path: result,
        name: result.split(/[\\/]/).pop() ?? result,
        size: meta,
      });
    }
  }, [mode, onSelect]);

  const onDrop = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    setDragging(false);
  }, []);

  if (mode === "export") {
    return (
      <div className="flex items-center justify-between gap-4 rounded-2xl bg-white px-5 py-4 shadow-sm ring-1 ring-slate-200">
        <div className="min-w-0">
          <div className="text-sm font-medium text-slate-800">
            {image ? image.name : t("export.location")}
          </div>
          <div className="truncate text-xs text-slate-400">
            {image ? image.path : t("export.hint")}
          </div>
        </div>
        <button
          onClick={browse}
          disabled={disabled}
          className="shrink-0 rounded-xl bg-slate-100 px-4 py-2 text-sm font-medium text-slate-700 transition hover:bg-slate-200 disabled:opacity-50"
        >
          {image ? t("btn.change") : t("export.select")}
        </button>
      </div>
    );
  }

  return (
    <div
      onClick={browse}
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
      className={`cursor-pointer rounded-2xl border-2 border-dashed bg-white px-5 py-6 text-center transition ${
        dragging ? "border-blue-500 bg-blue-50/50" : "border-slate-200 hover:border-blue-300"
      } ${disabled ? "pointer-events-none opacity-50" : ""}`}
    >
      {image ? (
        <div>
          <div className="text-sm font-semibold text-slate-800">{image.name}</div>
          <div className="mt-1 text-xs text-slate-400">
            {formatSize(image.size)} · {t("image.change")}
          </div>
        </div>
      ) : (
        <div>
          <div className="text-sm font-medium text-slate-500">{t("image.drag")}</div>
          <div className="mt-1 text-xs text-slate-400">{t("image.supported")}</div>
        </div>
      )}
    </div>
  );
}

async function invoke_get_size(path: string): Promise<number> {
  try {
    const { invoke } = await import("@tauri-apps/api/core");
    return await invoke<number>("file_size", { path });
  } catch {
    return 0;
  }
}
