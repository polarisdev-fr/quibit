use crate::qmp::{self, QemuConfig, QmpError, VmStatus};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::io::BufReader;
use tokio::net::UnixStream;
use tokio::process::Child;
use tokio::sync::Mutex;

static SOCKET_COUNTER: AtomicU64 = AtomicU64::new(0);

struct RunningVm {
    child: Child,
    qmp: BufReader<UnixStream>,
}

#[derive(Default)]
pub struct VmManager {
    vms: Mutex<HashMap<String, RunningVm>>,
}

impl VmManager {
    pub async fn start_vm(&self, vm_id: String, config: QemuConfig) -> Result<(), QmpError> {
        let mut vms = self.vms.lock().await;
        if vms.contains_key(&vm_id) {
            return Err(QmpError::VmAlreadyRunning(vm_id));
        }

        let socket_path = new_socket_path();
        let (child, qmp) = qmp::spawn_and_connect(&socket_path, &config).await?;
        vms.insert(vm_id, RunningVm { child, qmp });
        Ok(())
    }

    pub async fn get_vm_status(&self, vm_id: &str) -> Result<VmStatus, QmpError> {
        let mut vms = self.vms.lock().await;
        let vm = vms
            .get_mut(vm_id)
            .ok_or_else(|| QmpError::VmNotFound(vm_id.to_owned()))?;

        if vm.child.try_wait().map_err(QmpError::Io)?.is_some() {
            vms.remove(vm_id);
            return Err(QmpError::VmExited(vm_id.to_owned()));
        }

        let status = qmp::query_status(&mut vm.qmp).await;
        if status.is_err() && vm.child.try_wait().map_err(QmpError::Io)?.is_some() {
            vms.remove(vm_id);
            return Err(QmpError::VmExited(vm_id.to_owned()));
        }
        status
    }
}

fn new_socket_path() -> PathBuf {
    let sequence = SOCKET_COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("qubit-qmp-{}-{sequence}.sock", std::process::id()))
}
