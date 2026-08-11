import { invoke } from "@tauri-apps/api/core";
import type { Snapshot } from "./types";

export const isTauri = () => "__TAURI_INTERNALS__" in window;

const demo: Snapshot = {
  platform: "Windows", architecture: "x86_64", data_directory: "~/.axonhub-manager",
  config_path: "~/.axonhub-manager/config.yml", installed_version: null,
  previous_version: null, latest_version: "v1.0.0-beta5", update_available: false,
  lifecycle: "not_installed", healthy: false, pid: null, port: 8090,
  management_url: "http://127.0.0.1:8090", uptime: null, busy: false,
  progress_percent: null, progress_label: null, last_error: null,
  logs: [{ timestamp: new Date().toISOString(), level: "info", message: "前端预览模式：启动 Tauri 后可管理真实 AxonHub 实例" }],
};

export async function getSnapshot(): Promise<Snapshot> {
  return isTauri() ? invoke("get_snapshot") : demo;
}

export async function run(command: string): Promise<Snapshot> {
  if (!isTauri()) return demo;
  return invoke(command);
}

export async function readConfig(): Promise<string> {
  if (!isTauri()) return "server:\n  host: \"127.0.0.1\"\n  port: 8090\n\ndb:\n  dialect: \"sqlite3\"";
  return invoke("read_config");
}

export async function saveConfig(content: string): Promise<Snapshot> {
  return invoke("save_config", { content });
}
