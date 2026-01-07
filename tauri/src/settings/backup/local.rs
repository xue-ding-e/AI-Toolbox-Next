use chrono::Local;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use tauri::Manager;
use walkdir::WalkDir;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use super::utils::get_db_path;

/// Backup database to a zip file
#[tauri::command]
pub async fn backup_database(
    app_handle: tauri::AppHandle,
    backup_path: String,
) -> Result<String, String> {
    let db_path = get_db_path(&app_handle)?;

    // Ensure database directory exists
    if !db_path.exists() {
        fs::create_dir_all(&db_path)
            .map_err(|e| format!("Failed to create database dir: {}", e))?;
    }

    // Ensure backup directory exists
    let backup_dir = Path::new(&backup_path);
    if !backup_dir.exists() {
        fs::create_dir_all(backup_dir)
            .map_err(|e| format!("Failed to create backup dir: {}", e))?;
    }

    // Generate backup filename with timestamp
    let timestamp = Local::now().format("%Y%m%d-%H%M%S");
    let backup_filename = format!("ai-toolbox-backup-{}.zip", timestamp);
    let backup_file_path = backup_dir.join(&backup_filename);

    // Create zip file
    let file = File::create(&backup_file_path)
        .map_err(|e| format!("Failed to create backup file: {}", e))?;
    let mut zip = ZipWriter::new(file);
    let options =
        SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // Walk through the database directory and add files to zip
    let mut has_files = false;
    for entry in WalkDir::new(&db_path) {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let path = entry.path();
        let relative_path = path
            .strip_prefix(&db_path)
            .map_err(|e| format!("Failed to get relative path: {}", e))?;

        if path.is_file() {
            has_files = true;
            let name = relative_path.to_string_lossy();
            zip.start_file(name.to_string(), options)
                .map_err(|e| format!("Failed to start file in zip: {}", e))?;

            let mut file = File::open(path).map_err(|e| format!("Failed to open file: {}", e))?;
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read file: {}", e))?;
            zip.write_all(&buffer)
                .map_err(|e| format!("Failed to write to zip: {}", e))?;
        } else if path.is_dir() && !relative_path.as_os_str().is_empty() {
            let name = format!("{}/", relative_path.to_string_lossy());
            zip.add_directory(name, options)
                .map_err(|e| format!("Failed to add directory to zip: {}", e))?;
        }
    }

    // If no files, add a placeholder to ensure valid zip
    if !has_files {
        zip.start_file(".backup_marker", options)
            .map_err(|e| format!("Failed to create marker file: {}", e))?;
        zip.write_all(b"AI Toolbox Backup")
            .map_err(|e| format!("Failed to write marker: {}", e))?;
    }

    zip.finish()
        .map_err(|e| format!("Failed to finish zip: {}", e))?;

    Ok(backup_file_path.to_string_lossy().to_string())
}

/// Restore database from a zip file
#[tauri::command]
pub async fn restore_database(
    app_handle: tauri::AppHandle,
    zip_file_path: String,
) -> Result<(), String> {
    let db_path = get_db_path(&app_handle)?;
    let zip_path = Path::new(&zip_file_path);

    if !zip_path.exists() {
        return Err("Backup file does not exist".to_string());
    }

    // Open zip file
    let file = File::open(zip_path).map_err(|e| format!("Failed to open backup file: {}", e))?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read zip archive: {}", e))?;

    // Remove existing database directory
    if db_path.exists() {
        fs::remove_dir_all(&db_path)
            .map_err(|e| format!("Failed to remove existing database: {}", e))?;
    }

    // Create database directory
    fs::create_dir_all(&db_path)
        .map_err(|e| format!("Failed to create database directory: {}", e))?;

    // Extract zip contents
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read zip entry: {}", e))?;

        // Skip the backup marker file
        if file.name() == ".backup_marker" {
            continue;
        }

        let outpath = db_path.join(file.name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    fs::create_dir_all(parent)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
            }
            let mut outfile =
                File::create(&outpath).map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }

    Ok(())
}

/// Get database directory path for frontend
#[tauri::command]
pub fn get_database_path(app_handle: tauri::AppHandle) -> Result<String, String> {
    let db_path = get_db_path(&app_handle)?;
    Ok(db_path.to_string_lossy().to_string())
}

/// Open the app data directory in the file explorer
#[tauri::command]
pub fn open_app_data_dir(app_handle: tauri::AppHandle) -> Result<(), String> {
    let app_data_dir = app_handle
        .path()
        .app_data_dir()
        .map_err(|e| format!("Failed to get app data dir: {}", e))?;

    // Ensure directory exists
    if !app_data_dir.exists() {
        fs::create_dir_all(&app_data_dir)
            .map_err(|e| format!("Failed to create app data directory: {}", e))?;
    }

    // Open in file explorer (platform-specific)
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(&app_data_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(&app_data_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(&app_data_dir)
            .spawn()
            .map_err(|e| format!("Failed to open folder: {}", e))?;
    }

    Ok(())
}
