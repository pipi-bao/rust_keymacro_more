//! 配置文件加载和解析模块
//!
//! 支持从 YAML 文件加载键盘宏配置

use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::Path;

/// 延迟配置，支持固定值或随机范围
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DelayConfig {
    /// 固定延迟值（毫秒）
    Fixed(u64),
    /// 随机延迟范围（毫秒）
    Range { min: u64, max: u64 },
}

impl DelayConfig {
    /// 获取实际延迟值（如果是随机范围则生成随机值）
    pub fn get_delay(&self) -> u64 {
        match self {
            DelayConfig::Fixed(ms) => *ms,
            DelayConfig::Range { min, max } => {
                rand::thread_rng().gen_range(*min..=*max)
            }
        }
    }
}

/// 配置文件根结构
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// 全局快捷键配置（可选，用于控制宏总开关）
    #[serde(default)]
    pub global_hotkey: Option<GlobalHotkeyConfig>,
    pub hotkeys: Vec<HotkeyConfig>,
}

/// 全局快捷键配置
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GlobalHotkeyConfig {
    /// 修饰键组合，如 "ctrl+alt"，可为空字符串
    #[serde(default)]
    pub modifiers: String,
    /// 主键，如 "q"
    pub key: String,
}

impl Default for GlobalHotkeyConfig {
    fn default() -> Self {
        Self {
            modifiers: "ctrl+alt".to_string(),
            key: "q".to_string(),
        }
    }
}

impl GlobalHotkeyConfig {
    /// 格式化为可读文本，如 "Ctrl+Alt+Q"
    pub fn display(&self) -> String {
        let mods = if self.modifiers.is_empty() {
            String::new()
        } else {
            self.modifiers
                .split('+')
                .filter(|s| !s.is_empty())
                .map(|m| {
                    match m.to_lowercase().as_str() {
                        "ctrl" | "control" => "Ctrl".to_string(),
                        "alt" => "Alt".to_string(),
                        "shift" => "Shift".to_string(),
                        "win" | "meta" | "super" => "Win".to_string(),
                        _ => m.to_string(),
                    }
                })
                .collect::<Vec<_>>()
                .join("+")
        };
        let key = self.key.trim().to_string();
        if mods.is_empty() {
            key
        } else {
            format!("{}+{}", mods, key)
        }
    }
}

/// 触发源类型
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TriggerSource {
    /// 键盘按键，如 "F2", "'"
    Keyboard { key: String },
    /// 手柄按键，如 "A", "LT", "DUp"
    Gamepad { key: String },
}

impl TriggerSource {
    /// 获取触发键名称（用于查找）
    pub fn key_name(&self) -> String {
        match self {
            TriggerSource::Keyboard { key } => key.clone(),
            TriggerSource::Gamepad { key } => format!("GP:{}", canonicalize_gamepad_key(key)),
        }
    }

    /// 检查是否匹配给定的键名
    pub fn matches(&self, name: &str) -> bool {
        self.key_name().eq_ignore_ascii_case(name)
    }
}

/// 单个热键配置
#[derive(Debug, Clone, Serialize)]
pub struct HotkeyConfig {
    /// 触发源配置（新格式）
    #[serde(flatten)]
    pub trigger: TriggerSource,
    /// 是否启用此宏（默认 true）。全局总开关关闭时，即使这里为 true 也不会触发。
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// 操作类型："type_text" / "sequence" / "auto_repeat" / "hold_loop"
    pub action: String,
    /// 操作参数
    pub params: ActionParams,
}

impl<'de> Deserialize<'de> for HotkeyConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(flatten)]
            trigger: TriggerSource,
            #[serde(default = "default_enabled")]
            enabled: bool,
            action: String,
            params: serde_yaml::Value,
        }

        let raw = Raw::deserialize(deserializer)?;
        let params = match raw.action.as_str() {
            "type_text" => ActionParams::TypeText(
                serde_yaml::from_value(raw.params).map_err(serde::de::Error::custom)?,
            ),
            "auto_repeat" => ActionParams::AutoRepeat(
                serde_yaml::from_value(raw.params).map_err(serde::de::Error::custom)?,
            ),
            "hold_loop" => ActionParams::HoldLoop(
                serde_yaml::from_value(raw.params).map_err(serde::de::Error::custom)?,
            ),
            _ => ActionParams::Sequence(
                serde_yaml::from_value(raw.params).map_err(serde::de::Error::custom)?,
            ),
        };

        Ok(HotkeyConfig {
            trigger: raw.trigger,
            enabled: raw.enabled,
            action: raw.action,
            params,
        })
    }
}

impl HotkeyConfig {
    /// 兼容旧配置的 key 字段
    pub fn key(&self) -> String {
        self.trigger.key_name()
    }
}

/// 操作参数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionParams {
    TypeText(TypeTextParams),
    AutoRepeat(AutoRepeatParams),
    HoldLoop(HoldLoopParams),
    Sequence(SequenceParams),
}

impl ActionParams {
    /// 取出可编辑的步骤列表（sequence / hold_loop）
    pub fn steps(&self) -> Option<&Vec<Step>> {
        match self {
            ActionParams::Sequence(p) => Some(&p.steps),
            ActionParams::HoldLoop(p) => Some(&p.steps),
            _ => None,
        }
    }

    /// 取出可编辑的步骤列表（sequence / hold_loop）
    pub fn steps_mut(&mut self) -> Option<&mut Vec<Step>> {
        match self {
            ActionParams::Sequence(p) => Some(&mut p.steps),
            ActionParams::HoldLoop(p) => Some(&mut p.steps),
            _ => None,
        }
    }
}

fn default_enabled() -> bool { true }
fn default_press_ms() -> u64 { 20 }
fn default_release_ms() -> u64 { 30 }

/// 按键连发参数（DNF 风格：按住触发键，持续重复触发目标按键）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoRepeatParams {
    /// 连发时模拟的目标按键（虚拟键码对应的键名，如 "A"、"Space"）
    pub key: String,
    /// 目标按键按下持续时间（毫秒）
    #[serde(default = "default_press_ms")]
    pub press_ms: u64,
    /// 释放后到下一次按下的间隔（毫秒）
    #[serde(default = "default_release_ms")]
    pub release_ms: u64,
    /// "keyboard" 或 "gamepad"；缺省时按键名判断
    #[serde(default)]
    pub device: Option<String>,
}

impl Default for AutoRepeatParams {
    fn default() -> Self {
        Self {
            key: "A".to_string(),
            press_ms: default_press_ms(),
            release_ms: default_release_ms(),
            device: None,
        }
    }
}

/// 输入文本参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TypeTextParams {
    pub text: String,
    #[serde(default)]
    pub delay: Option<DelayConfig>,
}

/// 序列参数
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SequenceParams {
    pub steps: Vec<Step>,
}

/// 按住触发键期间循环执行序列，并可选按固定间隔插入额外按键
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldLoopParams {
    pub steps: Vec<Step>,
    /// 循环期间按固定间隔触发的附加按键（如每 5 秒按一次 X）
    #[serde(default)]
    pub every: Vec<IntervalAction>,
}

impl Default for HoldLoopParams {
    fn default() -> Self {
        Self {
            steps: Vec::new(),
            every: Vec::new(),
        }
    }
}

/// 循环期间的定时按键
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntervalAction {
    pub key: String,
    pub interval_ms: u64,
    #[serde(default = "default_press_ms")]
    pub press_ms: u64,
    /// "keyboard" 或 "gamepad"；缺省时按键名判断
    #[serde(default)]
    pub device: Option<String>,
}

/// 按键动作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum KeyAction {
    Press,    // 只按下
    Release,  // 只释放
    Complete, // 按下+释放（默认）
}

impl Default for KeyAction {
    fn default() -> Self {
        KeyAction::Complete
    }
}

/// 鼠标按钮类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MouseButtonType {
    Left,
    Right,
    Middle,
}

/// 鼠标动作类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MouseAction {
    Click,    // 点击（按下+释放）
    Down,     // 按下
    Up,       // 释放
}

impl Default for MouseAction {
    fn default() -> Self {
        MouseAction::Click
    }
}

/// 序列中的单个步骤
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum Step {
    Key { 
        value: String, 
        #[serde(default)] 
        delay: Option<DelayConfig>,
        #[serde(default)]
        action: Option<KeyAction>,
        /// "keyboard" 或 "gamepad"；缺省时 LB/RB 等仅手柄键名按手柄处理
        #[serde(default)]
        device: Option<String>,
    },
    Wait { 
        value: u64,
    },
    /// 随机等待，范围 [min, max]
    #[serde(rename = "wait_random")]
    WaitRandom { 
        min: u64,
        max: u64,
    },
    Text { value: String, #[serde(default)] delay: Option<DelayConfig> },
    /// 鼠标点击操作
    MouseClick { 
        button: MouseButtonType,
        #[serde(default)]
        delay: Option<DelayConfig>,
    },
    /// 鼠标按下/释放操作
    MouseAction { 
        button: MouseButtonType,
        action: MouseAction,
        #[serde(default)]
        delay: Option<DelayConfig>,
    },
    /// 鼠标移动操作
    MouseMove { 
        x: i32,
        y: i32,
        #[serde(default)]
        relative: Option<bool>, // true=相对移动，false=绝对移动
        #[serde(default)]
        delay: Option<DelayConfig>,
    },
    /// 鼠标滚轮操作
    MouseWheel { 
        delta: i32, // 正数向上，负数向下
        #[serde(default)]
        delay: Option<DelayConfig>,
    },
}

impl Config {
    /// 从文件加载配置（严格模式，用于测试）
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// 从文件加载配置（容错模式）
    ///
    /// 遇到无效的 hotkey 条目会跳过并记录日志，不会导致整体加载失败。
    /// 仅在所有 hotkey 都无效时才返回错误。
    pub fn from_file_lenient<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let content = fs::read_to_string(path)?;
        Self::from_str_lenient(&content)
    }

    /// 从字符串加载配置（严格模式，用于测试）
    #[allow(dead_code)]
    pub fn from_str(yaml_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let config: Config = serde_yaml::from_str(yaml_str)?;
        Ok(config)
    }

    /// 从字符串加载配置（容错模式）
    ///
    /// 逐个解析 hotkey，跳过无效条目，记录警告日志。
    pub fn from_str_lenient(yaml_str: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let raw: serde_yaml::Value = serde_yaml::from_str(yaml_str)?;

        let hotkeys_raw = raw
            .get("hotkeys")
            .and_then(|v| v.as_sequence())
            .ok_or("配置文件中缺少 hotkeys 字段或格式不正确")?;

        let mut hotkeys = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        for (i, item) in hotkeys_raw.iter().enumerate() {
            match serde_yaml::from_value::<HotkeyConfig>(item.clone()) {
                Ok(hk) => hotkeys.push(hk),
                Err(e) => {
                    let msg = format!("跳过无效的热键配置 [{}]: {}", i, e);
                    log::warn!("{}", msg);
                    errors.push(msg);
                }
            }
        }

        if hotkeys.is_empty() && !errors.is_empty() {
            return Err(format!(
                "所有热键配置均无效，共 {} 个错误:\n{}",
                errors.len(),
                errors.join("\n")
            )
            .into());
        }

        if !errors.is_empty() {
            log::warn!(
                "配置加载完成，跳过了 {} 个无效热键，有效热键: {}",
                errors.len(),
                hotkeys.len()
            );
        }

        let global_hotkey = raw
            .get("global_hotkey")
            .and_then(|v| serde_yaml::from_value::<GlobalHotkeyConfig>(v.clone()).ok());

        Ok(Config {
            global_hotkey,
            hotkeys,
        })
    }

    /// 查找指定键的配置
    pub fn find_hotkey(&self, key: &str) -> Option<&HotkeyConfig> {
        self.hotkeys.iter().find(|h| h.trigger.matches(key))
    }

    /// 是否需要虚拟手柄输出（任一已启用宏的输出含手柄按键）
    pub fn needs_virtual_gamepad(&self) -> bool {
        self.hotkeys.iter().filter(|h| h.enabled).any(hotkey_outputs_gamepad)
    }

}

/// 统一手柄键名，避免 YAML 写 Up 而事件是 DUp 对不上
pub fn canonicalize_gamepad_key(name: &str) -> String {
    match name.to_ascii_uppercase().as_str() {
        "UP" | "DUP" => "DUp".to_string(),
        "DOWN" | "DDOWN" => "DDown".to_string(),
        "LEFT" | "DLEFT" => "DLeft".to_string(),
        "RIGHT" | "DRIGHT" => "DRight".to_string(),
        "VIEW" | "SELECT" | "BACK" => "Back".to_string(),
        "MENU" | "START" => "Start".to_string(),
        "GUIDE" | "XBOX" => "Guide".to_string(),
        "LT" | "LEFTTRIGGER" => "LT".to_string(),
        "RT" | "RIGHTTRIGGER" => "RT".to_string(),
        "LB" | "LEFTSHOULDER" => "LB".to_string(),
        "RB" | "RIGHTSHOULDER" => "RB".to_string(),
        "LS" | "LEFTTHUMB" | "L3" => "LS".to_string(),
        "RS" | "RIGHTTHUMB" | "R3" => "RS".to_string(),
        other => {
            // 保留 A/B/X/Y 等常见单字母的常见大小写
            match other {
                "A" | "B" | "X" | "Y" => other.to_string(),
                _ => name.to_string(),
            }
        }
    }
}

/// 仅手柄存在、键盘没有同名键的按钮
pub fn is_exclusive_gamepad_button(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "LB" | "RB" | "LT" | "RT" | "START" | "BACK" | "GUIDE" | "LS" | "RS"
            | "DUP" | "DDOWN" | "DLEFT" | "DRIGHT"
            | "LEFTTRIGGER" | "RIGHTTRIGGER" | "LEFTSHOULDER" | "RIGHTSHOULDER"
            | "LEFTTHUMB" | "RIGHTTHUMB" | "L3" | "R3"
            | "VIEW" | "XBOX"
    )
}

/// 根据 device 字段和键名判断是否走手柄输出
pub fn uses_gamepad_device(device: Option<&str>, key: &str) -> bool {
    match device {
        Some(d) if d.eq_ignore_ascii_case("gamepad") => true,
        Some(d) if d.eq_ignore_ascii_case("keyboard") => false,
        _ => is_exclusive_gamepad_button(key),
    }
}

fn step_uses_gamepad(step: &Step) -> bool {
    match step {
        Step::Key { value, device, .. } => uses_gamepad_device(device.as_deref(), value),
        _ => false,
    }
}

/// 该宏执行时是否会向虚拟手柄输出按键
fn hotkey_outputs_gamepad(hotkey: &HotkeyConfig) -> bool {
    match &hotkey.params {
        ActionParams::HoldLoop(p) => {
            p.steps.iter().any(step_uses_gamepad)
                || p.every.iter().any(|a| uses_gamepad_device(a.device.as_deref(), &a.key))
        }
        ActionParams::Sequence(p) => p.steps.iter().any(step_uses_gamepad),
        ActionParams::AutoRepeat(p) => uses_gamepad_device(p.device.as_deref(), &p.key),
        ActionParams::TypeText(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_type_text_config() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "F2"
    action: "type_text"
    params:
      text: "hello"
      delay: 5
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        let hotkey = &config.hotkeys[0];
        assert_eq!(hotkey.key(), "F2");
        assert_eq!(hotkey.action, "type_text");

        if let ActionParams::TypeText(params) = &hotkey.params {
            assert_eq!(params.text, "hello");
            assert!(matches!(params.delay, Some(DelayConfig::Fixed(5))));
        } else {
            panic!("Expected TypeText params");
        }
    }

    #[test]
    fn test_parse_gamepad_config() {
        let yaml = r#"
hotkeys:
  - type: gamepad
    key: "A"
    action: "sequence"
    params:
      steps:
        - { type: "key", value: "Space", action: "press" }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        let hotkey = &config.hotkeys[0];
        assert_eq!(hotkey.key(), "GP:A");
        assert_eq!(hotkey.action, "sequence");

        match &hotkey.trigger {
            TriggerSource::Gamepad { key } => assert_eq!(key, "A"),
            _ => panic!("Expected Gamepad trigger"),
        }
    }

    #[test]
    fn test_parse_sequence_config() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "Ctrl+Shift+A"
    action: "sequence"
    params:
      steps:
        - { type: "key", value: "a", delay: 50 }
        - { type: "wait", value: 100 }
        - { type: "text", value: "done" }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        let hotkey = &config.hotkeys[0];
        assert_eq!(hotkey.key(), "Ctrl+Shift+A");
        assert_eq!(hotkey.action, "sequence");

        if let ActionParams::Sequence(params) = &hotkey.params {
            assert_eq!(params.steps.len(), 3);
            match &params.steps[0] {
                Step::Key { value, delay, action, .. } => {
                    assert_eq!(value, "a");
                    assert!(matches!(delay, Some(DelayConfig::Fixed(50))));
                    assert_eq!(*action, None); // 默认值为 None，会使用 KeyAction::Complete
                }
                _ => panic!("Expected Key step"),
            }
        } else {
            panic!("Expected Sequence params");
        }
    }

    #[test]
    fn test_parse_key_action_config() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "F1"
    action: "sequence"
    params:
      steps:
        - { type: "key", value: "Shift", action: "press" }
        - { type: "key", value: "a", action: "press" }
        - { type: "wait", value: 100 }
        - { type: "key", value: "a", action: "release" }
        - { type: "key", value: "Shift", action: "release" }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        if let ActionParams::Sequence(params) = &config.hotkeys[0].params {
            assert_eq!(params.steps.len(), 5);

            // 测试 press 动作
            match &params.steps[0] {
                Step::Key { value, action, .. } => {
                    assert_eq!(value, "Shift");
                    assert!(matches!(action, Some(KeyAction::Press)));
                }
                _ => panic!("Expected Key step"),
            }

            // 测试 release 动作
            match &params.steps[3] {
                Step::Key { value, action, .. } => {
                    assert_eq!(value, "a");
                    assert!(matches!(action, Some(KeyAction::Release)));
                }
                _ => panic!("Expected Key step"),
            }
        } else {
            panic!("Expected Sequence params");
        }
    }

    #[test]
    fn test_parse_auto_repeat_config() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "X"
    action: "auto_repeat"
    params:
      key: "Space"
      press_ms: 30
      release_ms: 20
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        let hotkey = &config.hotkeys[0];
        assert_eq!(hotkey.key(), "X");
        assert_eq!(hotkey.action, "auto_repeat");

        if let ActionParams::AutoRepeat(params) = &hotkey.params {
            assert_eq!(params.key, "Space");
            assert_eq!(params.press_ms, 30);
            assert_eq!(params.release_ms, 20);
        } else {
            panic!("Expected AutoRepeat params");
        }
    }

    #[test]
    fn test_parse_auto_repeat_defaults() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "Y"
    action: "auto_repeat"
    params:
      key: "A"
"#;
        let config = Config::from_str(yaml).unwrap();

        if let ActionParams::AutoRepeat(params) = &config.hotkeys[0].params {
            // 未指定时使用默认值
            assert_eq!(params.key, "A");
            assert_eq!(params.press_ms, 20);
            assert_eq!(params.release_ms, 30);
        } else {
            panic!("Expected AutoRepeat params");
        }
    }

    #[test]
    fn test_parse_hold_loop_config() {
        let yaml = r#"
hotkeys:
  - type: gamepad
    key: Y
    action: hold_loop
    params:
      steps:
        - { type: key, value: LB, device: gamepad, delay: 50 }
        - { type: key, value: RB, device: gamepad, delay: 50 }
      every:
        - key: X
          interval_ms: 5000
          device: gamepad
          press_ms: 40
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);
        assert_eq!(config.hotkeys[0].action, "hold_loop");
        match &config.hotkeys[0].trigger {
            TriggerSource::Gamepad { key } => assert_eq!(key, "Y"),
            _ => panic!("Expected Gamepad trigger"),
        }
        if let ActionParams::HoldLoop(params) = &config.hotkeys[0].params {
            assert_eq!(params.steps.len(), 2);
            assert_eq!(params.every.len(), 1);
            assert_eq!(params.every[0].key, "X");
            assert_eq!(params.every[0].interval_ms, 5000);
            assert_eq!(params.every[0].press_ms, 40);
            match &params.steps[0] {
                Step::Key { value, device, .. } => {
                    assert_eq!(value, "LB");
                    assert_eq!(device.as_deref(), Some("gamepad"));
                }
                _ => panic!("Expected Key step"),
            }
        } else {
            panic!("Expected HoldLoop params");
        }
        assert!(config.needs_virtual_gamepad());
        assert!(config.hotkeys[0].enabled);
    }

    #[test]
    fn test_macro_enabled_flag() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: F1
    action: sequence
    params:
      steps: []
  - type: gamepad
    key: X
    enabled: false
    action: hold_loop
    params:
      steps:
        - { type: key, value: LB, device: gamepad, delay: 50 }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 2);
        assert!(config.hotkeys[0].enabled);
        assert!(!config.hotkeys[1].enabled);
        assert!(!config.needs_virtual_gamepad());
    }

    #[test]
    fn test_sequence_gamepad_needs_virtual_pad() {
        let yaml = r#"
hotkeys:
  - type: gamepad
    key: RT
    enabled: true
    action: sequence
    params:
      steps:
        - { type: key, value: RT, device: gamepad, delay: 50 }
        - { type: key, value: RB, device: gamepad, delay: 50 }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert!(config.needs_virtual_gamepad());
    }

    #[test]
    fn test_auto_repeat_gamepad_needs_virtual_pad() {
        let yaml = r#"
hotkeys:
  - type: gamepad
    key: LT
    enabled: true
    action: auto_repeat
    params:
      key: RB
      device: gamepad
      press_ms: 20
      release_ms: 30
"#;
        let config = Config::from_str(yaml).unwrap();
        assert!(config.needs_virtual_gamepad());
    }

    #[test]
    fn test_gamepad_key_aliases_match_events() {
        let yaml = r#"
hotkeys:
  - type: gamepad
    key: Up
    action: sequence
    params:
      steps: []
"#;
        let config = Config::from_str(yaml).unwrap();
        assert!(config.find_hotkey("GP:DUp").is_some());
        assert_eq!(config.hotkeys[0].key(), "GP:DUp");
        assert_eq!(canonicalize_gamepad_key("View"), "Back");
        assert_eq!(canonicalize_gamepad_key("Xbox"), "Guide");
        assert_eq!(canonicalize_gamepad_key("L3"), "LS");
        assert_eq!(canonicalize_gamepad_key("RightTrigger"), "RT");
    }

    #[test]
    fn test_parse_random_delay_config() {
        let yaml = r#"
hotkeys:
  - type: keyboard
    key: "F3"
    action: "sequence"
    params:
      steps:
        - { type: "key", value: "a", delay: { min: 10, max: 30 } }
        - { type: "wait_random", min: 0, max: 100 }
        - { type: "text", value: "done", delay: { min: 5, max: 15 } }
"#;
        let config = Config::from_str(yaml).unwrap();
        assert_eq!(config.hotkeys.len(), 1);

        if let ActionParams::Sequence(params) = &config.hotkeys[0].params {
            assert_eq!(params.steps.len(), 3);

            // 测试随机延迟范围
            match &params.steps[0] {
                Step::Key { value, delay, .. } => {
                    assert_eq!(value, "a");
                    assert!(matches!(delay, Some(DelayConfig::Range { min: 10, max: 30 })));
                }
                _ => panic!("Expected Key step"),
            }

            // 测试随机等待
            match &params.steps[1] {
                Step::WaitRandom { min, max } => {
                    assert_eq!(*min, 0);
                    assert_eq!(*max, 100);
                }
                _ => panic!("Expected WaitRandom step"),
            }

            // 测试文本随机延迟
            match &params.steps[2] {
                Step::Text { value, delay } => {
                    assert_eq!(value, "done");
                    assert!(matches!(delay, Some(DelayConfig::Range { min: 5, max: 15 })));
                }
                _ => panic!("Expected Text step"),
            }
        } else {
            panic!("Expected Sequence params");
        }
    }
}
