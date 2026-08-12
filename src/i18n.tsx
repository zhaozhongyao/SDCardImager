import { createContext, useContext, useEffect, useState, type ReactNode } from "react";

export type Lang = "zh" | "en";

const zh: Record<string, string> = {
  "app.subtitle": "跨平台 SD 卡 / U 盘镜像烧录工具",
  "mode.flash": "烧录镜像",
  "mode.export": "导出镜像",
  "btn.start.flash": "开始烧录",
  "btn.start.export": "开始导出",
  "btn.cancel.flash": "取消烧录",
  "btn.cancel.export": "取消导出",
  "header.target": "目标设备",
  "btn.refresh": "刷新",
  "btn.selectAll": "全选",
  "btn.clear": "清除",
  "no.devices": "未检测到可移动存储设备，请插入 SD 卡或 U 盘后刷新",
  "partitions": "{n} 个分区",
  "stage.queued": "排队中",
  "stage.preparing": "准备中",
  "stage.flashing": "烧录中",
  "stage.exporting": "导出中",
  "stage.verifying": "校验中",
  "stage.done": "✓ 完成",
  "stage.error": "✗ 失败",
  "stage.cancelled": "已取消",
  "duration": "总耗时 {d}",
  "speed.eta": "{s} MB/s · 剩 {e}",
  "warn.overwrite": "⚠ 警告：烧录将永久覆盖所选设备上的全部数据（共 {n} 台设备）",
  "confirm.title": "危险操作确认",
  "confirm.ok": "继续",
  "confirm.cancel": "取消",
  "confirm.flash.msg":
    "即将把镜像烧录到 {n} 个设备。\n\n{paths}\n\n目标设备上的所有数据将被覆盖，且无法恢复！是否继续？",
  "confirm.export.msg":
    "即将从设备导出镜像到：\n\n{paths}\n\n导出为只读操作，不会修改设备数据。是否继续？",
  "image.drag": "拖拽镜像文件到这里，或点击选择",
  "image.supported": "支持 img / iso / raw / dmg 等格式",
  "image.change": "点击更换镜像文件",
  "export.location": "导出位置",
  "export.hint": "导出模式：将所选设备完整读取为镜像文件",
  "btn.browse": "浏览",
  "btn.change": "更改",
  "export.select": "选择位置",
  "export.range.only": "仅导出有效分区",
  "export.full": "完全导出",
  "export.custom": "自定义大小",
  "export.custom.from": "从卡头部开始导出",
  "export.custom.info": "将导出前 {size}（自定义大小）",
  "export.range.info": "将导出分区范围 {size}（含分区表，共 {n} 个分区，跳过卡尾部空白）",
  "export.full.info": "将导出整卡 {size}",
  "export.full.noparts": "将导出整卡 {size}（未检测到分区表）",
  "about.title": "关于",
  "about.version": "版本",
  "about.build": "编译时间",
  "about.platform": "平台",
  "settings.title": "设置",
  "settings.language": "语言",
  "settings.expert": "专家模式",
  "settings.expert.hint": "开启后，烧录镜像的扩展名不受限制",
  "settings.concurrency": "并发数量",
  "settings.concurrency.hint": "允许同时执行的烧录任务数（1-8）",
};

const en: Record<string, string> = {
  "app.subtitle": "Cross-platform SD card / USB drive image flasher",
  "mode.flash": "Flash Image",
  "mode.export": "Export Image",
  "btn.start.flash": "Start Flashing",
  "btn.start.export": "Start Export",
  "btn.cancel.flash": "Cancel Flashing",
  "btn.cancel.export": "Cancel Export",
  "header.target": "Target Devices",
  "btn.refresh": "Refresh",
  "btn.selectAll": "Select All",
  "btn.clear": "Clear",
  "no.devices": "No removable storage detected. Insert an SD card or USB drive and refresh.",
  "partitions": "{n} partition(s)",
  "stage.queued": "Queued",
  "stage.preparing": "Preparing",
  "stage.flashing": "Flashing",
  "stage.exporting": "Exporting",
  "stage.verifying": "Verifying",
  "stage.done": "✓ Done",
  "stage.error": "✗ Failed",
  "stage.cancelled": "Cancelled",
  "duration": "Total {d}",
  "speed.eta": "{s} MB/s · {e} left",
  "warn.overwrite":
    "⚠ Warning: flashing will permanently overwrite ALL data on the selected device(s) ({n} device(s))",
  "confirm.title": "Dangerous Operation",
  "confirm.ok": "Continue",
  "confirm.cancel": "Cancel",
  "confirm.flash.msg":
    "The image will be flashed to {n} device(s).\n\n{paths}\n\nALL data on the target device(s) will be overwritten and cannot be recovered. Continue?",
  "confirm.export.msg":
    "The image will be exported from the device to:\n\n{paths}\n\nExport is a read-only operation and will not modify the device. Continue?",
  "image.drag": "Drag an image file here, or click to select",
  "image.supported": "Supports img / iso / raw / dmg and more",
  "image.change": "Click to change the image file",
  "export.location": "Export Location",
  "export.hint": "Export mode: read the selected device into an image file",
  "btn.browse": "Browse",
  "btn.change": "Change",
  "export.select": "Select Location",
  "export.range.only": "Partitions Only",
  "export.full": "Full Export",
  "export.custom": "Custom Size",
  "export.custom.from": "Export from the beginning of the card",
  "export.custom.info": "Will export the first {size} (custom size)",
  "export.range.info":
    "Will export partition range {size} (includes partition table, {n} partition(s), skips trailing free space)",
  "export.full.info": "Will export the full card {size}",
  "export.full.noparts": "Will export the full card {size} (no partition table detected)",
  "about.title": "About",
  "about.version": "Version",
  "about.build": "Build Time",
  "about.platform": "Platform",
  "settings.title": "Settings",
  "settings.language": "Language",
  "settings.expert": "Expert Mode",
  "settings.expert.hint": "When enabled, image file extensions are unrestricted",
  "settings.concurrency": "Concurrency",
  "settings.concurrency.hint": "Number of tasks allowed to run at once (1-8)",
};

export interface I18nContextValue {
  lang: Lang;
  setLang: (l: Lang) => void;
  t: (key: string, params?: Record<string, string | number>) => string;
}

const Ctx = createContext<I18nContextValue>({
  lang: "zh",
  setLang: () => {},
  t: (k) => k,
});

export function I18nProvider({ children }: { children: ReactNode }) {
  const [lang, setLang] = useState<Lang>(() => {
    const saved = localStorage.getItem("lang");
    return saved === "en" ? "en" : "zh";
  });

  const t = (key: string, params?: Record<string, string | number>): string => {
    const dict = lang === "zh" ? zh : en;
    let s = dict[key] ?? key;
    if (params) {
      for (const [k, v] of Object.entries(params)) {
        s = s.replaceAll(`{${k}}`, String(v));
      }
    }
    return s;
  };

  useEffect(() => {
    localStorage.setItem("lang", lang);
  }, [lang]);

  return <Ctx.Provider value={{ lang, setLang, t }}>{children}</Ctx.Provider>;
}

export const useI18n = () => useContext(Ctx);
