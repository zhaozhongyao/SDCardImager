import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useI18n } from "../i18n";

export function Modal({
  title,
  onClose,
  children,
}: {
  title: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/30"
      onClick={onClose}
    >
      <div
        className="w-[380px] rounded-2xl bg-white p-6 shadow-2xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-base font-bold text-slate-800">{title}</h3>
          <button
            onClick={onClose}
            className="flex h-7 w-7 items-center justify-center rounded-full text-slate-400 transition hover:bg-slate-100 hover:text-slate-600"
          >
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

interface AppInfo {
  version: string;
  build_unix: number | null;
  os: string;
  arch: string;
}

export function AboutDialog({ onClose }: { onClose: () => void }) {
  const { t } = useI18n();
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    invoke<AppInfo>("app_info").then(setInfo).catch(() => setInfo(null));
  }, []);

  const buildTime = info?.build_unix
    ? new Date(info.build_unix * 1000).toLocaleString()
    : "—";

  const Row = ({ label, value }: { label: string; value: string }) => (
    <div className="flex items-center justify-between border-b border-slate-100 py-2.5 text-sm last:border-0">
      <span className="text-slate-500">{label}</span>
      <span className="font-medium text-slate-800">{value}</span>
    </div>
  );

  return (
    <Modal title={t("about.title")} onClose={onClose}>
      <div className="mb-3 flex items-center gap-3">
        <div className="flex h-11 w-11 items-center justify-center rounded-xl bg-gradient-to-br from-blue-500 to-indigo-600 text-sm font-bold text-white">
          SD
        </div>
        <div>
          <div className="text-sm font-semibold text-slate-800">
            SDCardImager
          </div>
          <div className="text-xs text-slate-400">{t("app.subtitle")}</div>
        </div>
      </div>
      <div>
        <Row label={t("about.version")} value={info?.version ?? "—"} />
        <Row label={t("about.build")} value={buildTime} />
        <Row
          label={t("about.platform")}
          value={info ? `${info.os} / ${info.arch}` : "—"}
        />
      </div>
    </Modal>
  );
}

export function SettingsDialog({ onClose }: { onClose: () => void }) {
  const { t, lang, setLang } = useI18n();
  const [expert, setExpert] = useState(
    () => localStorage.getItem("expertMode") === "1"
  );
  const [concurrency, setConcurrency] = useState(() => {
    const v = Number(localStorage.getItem("concurrency"));
    return v >= 1 && v <= 8 ? v : 3;
  });

  const toggleExpert = (v: boolean) => {
    setExpert(v);
    localStorage.setItem("expertMode", v ? "1" : "0");
  };

  const changeConcurrency = (v: number) => {
    const c = Math.min(8, Math.max(1, v));
    setConcurrency(c);
    localStorage.setItem("concurrency", String(c));
  };

  return (
    <Modal title={t("settings.title")} onClose={onClose}>
      <div className="flex items-center justify-between py-2.5">
        <span className="text-sm text-slate-700">{t("settings.language")}</span>
        <div className="flex rounded-lg bg-slate-100 p-0.5">
          <button
            onClick={() => setLang("zh")}
            className={`rounded-md px-3 py-1 text-xs font-medium transition ${
              lang === "zh" ? "bg-white text-slate-800 shadow" : "text-slate-500"
            }`}
          >
            中文
          </button>
          <button
            onClick={() => setLang("en")}
            className={`rounded-md px-3 py-1 text-xs font-medium transition ${
              lang === "en" ? "bg-white text-slate-800 shadow" : "text-slate-500"
            }`}
          >
            English
          </button>
        </div>
      </div>
      <div className="flex items-center justify-between border-t border-slate-100 py-3">
        <div>
          <div className="text-sm text-slate-700">{t("settings.expert")}</div>
          <div className="text-xs text-slate-400">{t("settings.expert.hint")}</div>
        </div>
        <button
          onClick={() => toggleExpert(!expert)}
          className={`relative h-6 w-11 rounded-full transition ${
            expert ? "bg-blue-500" : "bg-slate-300"
          }`}
        >
          <span
            className={`absolute top-0.5 h-5 w-5 rounded-full bg-white shadow transition-all ${
              expert ? "left-[22px]" : "left-0.5"
            }`}
          />
        </button>
      </div>
      <div className="flex items-center justify-between border-t border-slate-100 py-3">
        <div>
          <div className="text-sm text-slate-700">
            {t("settings.concurrency")}
          </div>
          <div className="text-xs text-slate-400">{t("settings.concurrency.hint")}</div>
        </div>
        <div className="flex items-center gap-2">
          <button
            onClick={() => changeConcurrency(concurrency - 1)}
            disabled={concurrency <= 1}
            className="flex h-7 w-7 items-center justify-center rounded-lg bg-slate-100 text-slate-600 transition hover:bg-slate-200 disabled:opacity-40"
          >
            −
          </button>
          <span className="w-8 text-center text-sm font-bold tabular-nums text-slate-800">
            {concurrency}
          </span>
          <button
            onClick={() => changeConcurrency(concurrency + 1)}
            disabled={concurrency >= 8}
            className="flex h-7 w-7 items-center justify-center rounded-lg bg-slate-100 text-slate-600 transition hover:bg-slate-200 disabled:opacity-40"
          >
            +
          </button>
        </div>
      </div>
    </Modal>
  );
}
