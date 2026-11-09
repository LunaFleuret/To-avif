// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod tools;
mod scanner;
mod metadata;
mod context_menu;
mod engine;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use tauri::{Emitter, State};

use tools::{locate_tools, ToolStatus};
use scanner::{scan_multiple_paths_with_progress, ScanResult};
use engine::{
    get_default_presets, ConversionPreset, ConversionOptions,
    FileConvertResult, ProgressEventPayload, convert_single_file,
    move_files_to_trash,
};

struct AppState {
    cancel_flag: Arc<AtomicBool>,
    is_converting: Arc<AtomicBool>,
    tool_status: Mutex<ToolStatus>,
    initial_args: Mutex<Vec<String>>,
    scan_generation: Arc<AtomicU64>,
    active_scan_cancel: Mutex<Arc<AtomicBool>>,
}

#[tauri::command]
fn check_tools(state: State<'_, AppState>) -> ToolStatus {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let root_dir = current_exe.parent().unwrap_or(Path::new("."));
    let status = locate_tools(root_dir);
    *state.tool_status.lock().unwrap() = status.clone();
    status
}

#[tauri::command]
fn get_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
}

#[tauri::command]
fn get_presets() -> Vec<ConversionPreset> {
    get_default_presets()
}

#[tauri::command]
async fn scan_paths(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    paths: Vec<String>,
    recursive: bool,
) -> Result<Option<ScanResult>, String> {
    let my_gen = state.scan_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let new_cancel = Arc::new(AtomicBool::new(false));

    // 直前のスキャンが実行中であれば中断シグナルを送信
    {
        let mut old_cancel = state.active_scan_cancel.lock().unwrap();
        old_cancel.store(true, Ordering::SeqCst);
        *old_cancel = new_cancel.clone();
    }

    let scan_gen = state.scan_generation.clone();

    tokio::task::spawn_blocking(move || {
        let app_handle = app.clone();
        let res = scan_multiple_paths_with_progress(
            &paths,
            recursive,
            &new_cancel,
            |count, bytes| {
                if scan_gen.load(Ordering::Relaxed) == my_gen {
                    let _ = app_handle.emit(
                        "scan_progress",
                        serde_json::json!({
                            "count": count,
                            "bytes": bytes,
                        }),
                    );
                }
            },
        );

        if scan_gen.load(Ordering::Relaxed) != my_gen {
            return Ok(None);
        }

        Ok(res)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
fn get_initial_paths(state: State<'_, AppState>) -> Vec<String> {
    state.initial_args.lock().unwrap().clone()
}

#[tauri::command]
fn get_menu_status() -> bool {
    context_menu::is_context_menu_registered()
}

#[tauri::command]
fn set_menu_registered(register: bool) -> Result<bool, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if register {
        context_menu::register_context_menu(&exe)?;
    } else {
        context_menu::unregister_context_menu()?;
    }
    Ok(context_menu::is_context_menu_registered())
}

#[tauri::command]
fn trash_files(paths: Vec<String>) -> Result<usize, String> {
    move_files_to_trash(&paths)
}

#[tauri::command]
fn cancel_conversion(state: State<'_, AppState>) {
    state.cancel_flag.store(true, Ordering::SeqCst);
}

#[tauri::command]
async fn run_batch_conversion(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    files: Vec<String>,
    options: ConversionOptions,
) -> Result<Vec<FileConvertResult>, String> {
    if state.is_converting.load(Ordering::SeqCst) {
        return Err("A conversion task is already running".into());
    }

    state.is_converting.store(true, Ordering::SeqCst);
    state.cancel_flag.store(false, Ordering::SeqCst);

    let cancel_flag = state.cancel_flag.clone();
    let is_converting = state.is_converting.clone();

    let tool_status = state.tool_status.lock().unwrap().clone();
    let ffmpeg_cmd = tool_status.ffmpeg_path.unwrap_or_else(|| "ffmpeg".into());
    let exiftool_cmd = tool_status.exiftool_path;

    let total = files.len();
    let mut results: Vec<FileConvertResult> = Vec::with_capacity(total);
    let mut total_saved: i64 = 0;

    let thread_count = if options.thread_count > 0 {
        options.thread_count
    } else {
        get_cpu_count()
    };

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(thread_count)
        .build()
        .map_err(|e| format!("Failed to build thread pool: {}", e))?;

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<FileConvertResult>();

    // Spawn rayon worker thread
    let cancel_for_worker = cancel_flag.clone();
    let files_clone = files.clone();
    let options_clone = options.clone();

    std::thread::spawn(move || {
        pool.install(|| {
            use rayon::prelude::*;
            files_clone.par_iter().for_each(|file_str| {
                if cancel_for_worker.load(Ordering::SeqCst) {
                    return;
                }
                let src_path = Path::new(file_str);
                let dst_path = if let Some(ref out_dir) = options_clone.output_dir {
                    let mut d = PathBuf::from(out_dir);
                    if let Some(file_name) = src_path.file_stem() {
                        d.push(format!("{}.avif", file_name.to_string_lossy()));
                    }
                    d
                } else {
                    src_path.with_extension("avif")
                };

                let res = convert_single_file(
                    &ffmpeg_cmd,
                    src_path,
                    &dst_path,
                    &options_clone,
                );

                let _ = tx.send(res);
            });
        });
    });

    let mut pending_exif_pairs: Vec<(PathBuf, PathBuf)> = Vec::new();
    let mut current_idx = 0;

    while let Some(res) = rx.recv().await {
        current_idx += 1;
        if res.success {
            let diff = (res.original_bytes as i64) - (res.converted_bytes as i64);
            total_saved += diff;

            if options.preserve_metadata && exiftool_cmd.is_some() {
                pending_exif_pairs.push((PathBuf::from(&res.src_path), PathBuf::from(&res.dst_path)));
            }
        }

        let mut batch_msg: Option<String> = None;

        // Flush batch of 50 pairs
        if pending_exif_pairs.len() >= 50 {
            if let Some(ref exif_bin) = exiftool_cmd {
                let count = pending_exif_pairs.len();
                let pair_refs: Vec<(&Path, &Path)> = pending_exif_pairs
                    .iter()
                    .map(|(s, d)| (s.as_path(), d.as_path()))
                    .collect();
                if let Err(e) = crate::metadata::batch_copy_metadata_exiftool(exif_bin, &pair_refs) {
                    batch_msg = Some(format!("[Exif同期失敗] {}", e));
                } else {
                    batch_msg = Some(format!("[Exif同期] {}件のメタデータを一括高速同期しました", count));
                }
                pending_exif_pairs.clear();
            }
        }

        let payload = ProgressEventPayload {
            current_index: current_idx,
            total_files: total,
            percent: (current_idx as f32 / total as f32) * 100.0,
            current_file: res.src_path.clone(),
            saved_bytes: total_saved,
            last_result: Some(res.clone()),
            batch_message: batch_msg,
        };

        let _ = app.emit("conversion-progress", payload);
        results.push(res);

        if current_idx >= total || cancel_flag.load(Ordering::SeqCst) {
            break;
        }
    }

    // Flush any remaining pairs
    if !pending_exif_pairs.is_empty() {
        if let Some(ref exif_bin) = exiftool_cmd {
            let count = pending_exif_pairs.len();
            let pair_refs: Vec<(&Path, &Path)> = pending_exif_pairs
                .iter()
                .map(|(s, d)| (s.as_path(), d.as_path()))
                .collect();
            let msg = if let Err(e) = crate::metadata::batch_copy_metadata_exiftool(exif_bin, &pair_refs) {
                format!("[Exif同期失敗] {}", e)
            } else {
                format!("[Exif同期] 残り{}件のメタデータを一括高速同期しました", count)
            };
            let _ = app.emit("conversion-progress", ProgressEventPayload {
                current_index: current_idx,
                total_files: total,
                percent: (current_idx as f32 / total as f32) * 100.0,
                current_file: "".into(),
                saved_bytes: total_saved,
                last_result: None,
                batch_message: Some(msg),
            });
            pending_exif_pairs.clear();
        }
    }

    is_converting.store(false, Ordering::SeqCst);
    Ok(results)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // CLI helpers for setup and uninstallation without opening GUI
    if args.iter().any(|a| a == "--register-menu" || a == "-r") {
        let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("to-avif.exe"));
        let _ = context_menu::unregister_context_menu();
        match context_menu::register_context_menu(&exe) {
            Ok(_) => {
                println!("Successfully registered Windows context menu.");
            }
            Err(e) => {
                eprintln!("Failed to register context menu: {}", e);
            }
        }
        return;
    }

    if args.iter().any(|a| a == "--unregister-menu" || a == "-u") {
        match context_menu::unregister_context_menu() {
            Ok(_) => {
                println!("Successfully unregistered Windows context menu.");
            }
            Err(e) => {
                eprintln!("Failed to unregister context menu: {}", e);
            }
        }
        return;
    }

    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let root_dir = current_exe.parent().unwrap_or(Path::new("."));
    let initial_tools = locate_tools(root_dir);

    let state = AppState {
        cancel_flag: Arc::new(AtomicBool::new(false)),
        is_converting: Arc::new(AtomicBool::new(false)),
        tool_status: Mutex::new(initial_tools),
        initial_args: Mutex::new(args),
        scan_generation: Arc::new(AtomicU64::new(0)),
        active_scan_cancel: Mutex::new(Arc::new(AtomicBool::new(false))),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            check_tools,
            get_cpu_count,
            get_presets,
            scan_paths,
            get_initial_paths,
            get_menu_status,
            set_menu_registered,
            trash_files,
            cancel_conversion,
            run_batch_conversion
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
