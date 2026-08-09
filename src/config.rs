use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;
use global_hotkey::hotkey::{Code, HotKey, Modifiers};
use seq_macro::seq;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub zoom: ZoomConfig,
    pub flashlight: FlashlightConfig,
    #[serde(default)]
    pub obs_output: ObsOutputConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FlashlightConfig {
    pub enabled: bool,
    pub radius: f32,
    pub shadow: f32,
}

impl Default for FlashlightConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            radius: 200.0,
            shadow: 0.8,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObsOutputConfig {
    pub enabled: bool,
}

impl Default for ObsOutputConfig {
    fn default() -> Self {
        Self { enabled: false }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HotkeyConfig {
    pub modifiers: Vec<String>,
    pub key: String,
}

impl HotkeyConfig {
    pub fn display(&self) -> String {
        let mut parts: Vec<String> = self
            .modifiers
            .iter()
            .map(|m| match m.to_ascii_uppercase().as_str() {
                "CONTROL" | "CTRL" => "Ctrl".into(),
                "ALT" => "Alt".into(),
                "SHIFT" => "Shift".into(),
                "SUPER" | "WIN" | "META" => "Win".into(),
                other => other.into(),
            })
            .collect();
        parts.push(if let Some(ch) = self.key.strip_prefix("Key") {
            ch.to_string()
        } else if let Some(d) = self.key.strip_prefix("Digit") {
            d.to_string()
        } else {
            self.key.clone()
        });
        parts.join(" + ")
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ZoomConfig {
    pub default_zoom: f32,
    #[serde(default)]
    pub free_roam: bool,
}

impl Default for ZoomConfig {
    fn default() -> Self {
        Self {
            default_zoom: 2.0,
            free_roam: false,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            hotkey: HotkeyConfig {
                modifiers: vec!["CONTROL".into(), "ALT".into()],
                key: "KeyQ".into(),
            },
            zoom: ZoomConfig::default(),
            flashlight: FlashlightConfig::default(),
            obs_output: ObsOutputConfig::default(),
        }
    }
}

impl Config {
    pub fn config_path() -> PathBuf {
        let dirs =
            ProjectDirs::from("", "", "fourlight").expect("could not resolve app data directory");
        dirs.config_dir()
            .parent()
            .expect("fourlight app root")
            .join("config.toml")
    }

    pub fn load_or_create() -> Result<(Self, PathBuf), String> {
        let path = Self::config_path();
        if !path.exists() {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            let config = Self::default();
            config.save(&path)?;
            return Ok((config, path));
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let config: Self = toml::from_str(&text).map_err(|e| {
            format!("{path:?}: {e}\ndelete this file and restart to recreate defaults")
        })?;
        Ok((config, path))
    }

    pub fn save(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = toml::to_string_pretty(self).map_err(|e| e.to_string())?;
        fs::write(path, text).map_err(|e| e.to_string())
    }

    pub fn to_hotkey(&self) -> Result<HotKey, String> {
        let modifiers = parse_modifiers(&self.hotkey.modifiers)?;
        let code = parse_code(&self.hotkey.key)?;
        Ok(HotKey::new(
            if modifiers.is_empty() {
                None
            } else {
                Some(modifiers)
            },
            code,
        ))
    }
}

fn parse_modifiers(names: &[String]) -> Result<Modifiers, String> {
    let mut mods = Modifiers::empty();
    for name in names {
        match name.to_ascii_uppercase().as_str() {
            "CONTROL" | "CTRL" => mods |= Modifiers::CONTROL,
            "ALT" => mods |= Modifiers::ALT,
            "SHIFT" => mods |= Modifiers::SHIFT,
            "SUPER" | "WIN" | "META" => mods |= Modifiers::SUPER,
            other => return Err(format!("unknown modifier `{other}`")),
        }
    }
    Ok(mods)
}

pub fn code_from_vk(vk: u32) -> Option<String> {
    match vk {
        0x41..=0x5A => Some(format!("Key{}", vk as u8 as char)),
        0x30..=0x39 => Some(format!("Digit{}", vk as u8 as char)),
        0x70..=0x7B => Some(format!("F{}", vk - 0x6F)),
        0x20 => Some("Space".into()),
        _ => None,
    }
}

pub fn parse_code(name: &str) -> Result<Code, String> {
    seq!(C in 'A'..='Z' {
        if name == stringify!(Key~C) {
            return Ok(Code::Key~C);
        }
    });
    seq!(C in '0'..='9' {
        if name == stringify!(Digit~C) {
            return Ok(Code::Digit~C);
        }
    });
    seq!(N in 1..=12 {
        if name == stringify!(F~N) {
            return Ok(Code::F~N);
        }
    });
    if name == "Space" {
        return Ok(Code::Space);
    }
    Err(format!("unknown key code `{name}`"))
}
