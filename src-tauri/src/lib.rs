mod manager;
mod qmp;

#[tauri::command]
async fn start_vm(
    manager: tauri::State<'_, manager::VmManager>,
    vm_id: String,
    config: qmp::QemuConfig,
) -> Result<(), qmp::QmpError> {
    manager.start_vm(vm_id, config).await
}

#[tauri::command]
async fn get_vm_status(
    manager: tauri::State<'_, manager::VmManager>,
    vm_id: String,
) -> Result<qmp::VmStatus, qmp::QmpError> {
    manager.get_vm_status(&vm_id).await
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(manager::VmManager::default())
        .invoke_handler(tauri::generate_handler![start_vm, get_vm_status])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
