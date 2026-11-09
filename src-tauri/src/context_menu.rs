use std::path::Path;
use winreg::enums::*;
use winreg::RegKey;

const MENU_KEY_NAME: &str = "ConvertAVIF";
const MENU_LABEL: &str = "AVIFに一括変換 (GUI)";

const TARGET_KEYS: &[&str] = &[
    r"Software\Classes\Directory\shell",
    r"Software\Classes\Directory\Background\shell",
    r"Software\Classes\Drive\shell",
];

pub fn is_context_menu_registered() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let check_path = format!(r"{}\{}", TARGET_KEYS[0], MENU_KEY_NAME);
    hkcu.open_subkey(&check_path).is_ok()
}

pub fn register_context_menu(exe_path: &Path) -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let exe_str = exe_path.to_string_lossy().to_string();
    let command_str = format!("\"{}\" \"%V\"", exe_str);
    let icon_str = format!("\"{}\",0", exe_str);

    for base_path in TARGET_KEYS {
        let menu_path = format!(r"{}\{}", base_path, MENU_KEY_NAME);
        let (key, _) = hkcu
            .create_subkey(&menu_path)
            .map_err(|e| format!("Failed to create subkey {}: {}", menu_path, e))?;

        key.set_value("", &MENU_LABEL)
            .map_err(|e| format!("Failed to set label: {}", e))?;
        key.set_value("Icon", &icon_str)
            .map_err(|e| format!("Failed to set icon: {}", e))?;

        let (cmd_key, _) = key
            .create_subkey("command")
            .map_err(|e| format!("Failed to create command key: {}", e))?;

        cmd_key
            .set_value("", &command_str)
            .map_err(|e| format!("Failed to set command value: {}", e))?;
    }

    Ok(())
}

pub fn unregister_context_menu() -> Result<(), String> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    for base_path in TARGET_KEYS {
        if let Ok(base_key) = hkcu.open_subkey_with_flags(base_path, KEY_ALL_ACCESS) {
            let menu_path = format!(r"{}\{}", base_path, MENU_KEY_NAME);
            // Delete command subkey first
            let _ = hkcu.delete_subkey(format!(r"{}\command", menu_path));
            // Delete main menu key
            let _ = base_key.delete_subkey(MENU_KEY_NAME);
        }
    }

    Ok(())
}
