    use std::path::PathBuf;
    use crate::instance::models::InstanceStatus;
    use crate::instance::mods::{mods_dir, is_mod_file};

    #[tauri::command]
    pub fn get_instance_status(instance_dir: String) -> InstanceStatus {
        let installed = PathBuf::from(&instance_dir).join("versions").read_dir().map(|mut d| d.next().is_some()).unwrap_or(false);
        let mods_count = mods_dir(&instance_dir).read_dir()
            .map(|d| d.filter_map(|e| e.ok()).filter(|e| is_mod_file(&e.file_name().to_string_lossy())).count() as u32)
            .unwrap_or(0);
        InstanceStatus { installed, mods_count }
    }