use std::path::Path;
use std::process::Command;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
use filetime::{FileTime, set_file_times};

const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn copy_file_timestamps(src_path: &Path, dst_path: &Path) -> Result<(), std::io::Error> {
    let metadata = std::fs::metadata(src_path)?;
    let mtime = FileTime::from_last_modification_time(&metadata);
    let atime = FileTime::from_last_access_time(&metadata);

    set_file_times(dst_path, atime, mtime)?;
    Ok(())
}

#[allow(dead_code)]
pub fn copy_metadata_exiftool(
    exiftool_cmd: &str,
    src_path: &Path,
    dst_path: &Path,
) -> Result<bool, String> {
    let mut cmd = Command::new(exiftool_cmd);
    cmd.args([
        "-overwrite_original",
        "-TagsFromFile",
        src_path.to_str().ok_or("Invalid src path")?,
        "-all:all>all:all",
        "-icc_profile>icc_profile",
        dst_path.to_str().ok_or("Invalid dst path")?,
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let res = cmd.output();

    // 万が一 _original バックアップファイルが生成された場合は自動削除
    let orig_path_str = format!("{}_original", dst_path.to_string_lossy());
    let orig_path = Path::new(&orig_path_str);
    if orig_path.exists() {
        let _ = std::fs::remove_file(orig_path);
    }

    match res {
        Ok(output) => {
            if output.status.success() {
                Ok(true)
            } else {
                let err = String::from_utf8_lossy(&output.stderr);
                Err(format!("ExifTool error: {}", err))
            }
        }
        Err(e) => Err(format!("Failed to execute exiftool: {}", e)),
    }
}

#[allow(dead_code)]
pub fn batch_copy_metadata_exiftool(
    exiftool_cmd: &str,
    pairs: &[(&Path, &Path)],
) -> Result<(), String> {
    if pairs.is_empty() {
        return Ok(());
    }

    // ExifTool argfile execution for ultra-fast batch processing
    let mut temp_argfile = std::env::temp_dir();
    let unique_id = format!("to_avif_args_{}.txt", std::process::id());
    temp_argfile.push(unique_id);

    let mut args_content = String::new();
    for (src, dst) in pairs {
        args_content.push_str("-overwrite_original\n");
        args_content.push_str("-TagsFromFile\n");
        args_content.push_str(&format!("{}\n", src.to_string_lossy()));
        args_content.push_str("-all:all>all:all\n");
        args_content.push_str("-icc_profile>icc_profile\n");
        args_content.push_str(&format!("{}\n", dst.to_string_lossy()));
        args_content.push_str("-execute\n");
    }

    std::fs::write(&temp_argfile, &args_content)
        .map_err(|e| format!("Failed to create argfile: {}", e))?;

    let mut cmd = Command::new(exiftool_cmd);
    cmd.args([
        "-overwrite_original",
        "-charset", "filename=utf8",
        "-@",
        temp_argfile.to_str().unwrap_or_default(),
    ]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let res = cmd.output();
    let _ = std::fs::remove_file(&temp_argfile);

    // 万が一 _original バックアップファイルが生成された場合は確実に自動削除
    for (_, dst) in pairs {
        let orig_path_str = format!("{}_original", dst.to_string_lossy());
        let orig_path = Path::new(&orig_path_str);
        if orig_path.exists() {
            let _ = std::fs::remove_file(orig_path);
        }
    }

    match res {
        Ok(out) => {
            if out.status.success() {
                Ok(())
            } else {
                Err(String::from_utf8_lossy(&out.stderr).to_string())
            }
        }
        Err(e) => Err(format!("ExifTool batch execution failed: {}", e)),
    }
}
