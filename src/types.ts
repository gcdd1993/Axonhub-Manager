export type Lifecycle = "not_installed" | "stopped" | "starting" | "running" | "installing" | "error";

export interface LogEntry {
  timestamp: string;
  level: "info" | "warn" | "error";
  message: string;
}

export interface Snapshot {
  platform: string;
  architecture: string;
  data_directory: string;
  config_path: string;
  installed_version: string | null;
  previous_version: string | null;
  latest_version: string | null;
  update_available: boolean;
  lifecycle: Lifecycle;
  healthy: boolean;
  pid: number | null;
  port: number;
  management_url: string;
  uptime: string | null;
  busy: boolean;
  progress_percent: number | null;
  progress_label: string | null;
  last_error: string | null;
  logs: LogEntry[];
  autostart_enabled: boolean;
}
