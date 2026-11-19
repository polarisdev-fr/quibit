import { invoke } from "@tauri-apps/api/core";

export interface QemuConfig {
  machine: string;
  accelerator: string;
  cpu: string;
  memoryMb: number;
}

export interface VmStatus {
  status: string;
  running: boolean;
}

export type VmApiError = string;

export function startVm(vmId: string, config: QemuConfig): Promise<void> {
  return invoke<void>("start_vm", { vmId, config });
}

export function getVmStatus(vmId: string): Promise<VmStatus> {
  return invoke<VmStatus>("get_vm_status", { vmId });
}

export function vmApiErrorMessage(error: unknown): VmApiError {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}
