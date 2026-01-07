use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

/// Get the database directory path
pub fn get_db_path(app_handle: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    use tauri::Manager;
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;
    Ok(app_data_dir.join("database"))
}

/// Create a temporary backup zip file and return its contents as bytes
pub fn create_backup_zip(db_path: &Path) -> Result<Vec<u8>, String> {
    use std::io::Cursor;

    let mut buffer = Cursor::new(Vec::new());

    {
        let mut zip = ZipWriter::new(&mut buffer);
        let options =
            SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        let mut has_files = false;
        for entry in WalkDir::new(db_path) {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            let relative_path = path
                .strip_prefix(db_path)
                .map_err(|e| format!("Failed to get relative path: {}", e))?;

            if path.is_file() {
                has_files = true;
                let name = relative_path.to_string_lossy();
                zip.start_file(name.to_string(), options)
                    .map_err(|e| format!("Failed to start file in zip: {}", e))?;

                let mut file =
                    File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
                let mut file_buffer = Vec::new();
                file.read_to_end(&mut file_buffer)
                    .map_err(|e| format!("Failed to read file: {}", e))?;
                zip.write_all(&file_buffer)
                    .map_err(|e| format!("Failed to write to zip: {}", e))?;
            } else if path.is_dir() && !relative_path.as_os_str().is_empty() {
                let name = format!("{}/", relative_path.to_string_lossy());
                zip.add_directory(name, options)
                    .map_err(|e| format!("Failed to add directory to zip: {}", e))?;
            }
        }

        if !has_files {
            zip.start_file(".backup_marker", options)
                .map_err(|e| format!("Failed to create marker file: {}", e))?;
            zip.write_all(b"AI Toolbox Backup")
                .map_err(|e| format!("Failed to write marker: {}", e))?;
        }

        zip.finish()
            .map_err(|e| format!("Failed to finish zip: {}", e))?;
    }

    Ok(buffer.into_inner())
}
