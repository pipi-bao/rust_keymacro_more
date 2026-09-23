//! 游戏手柄输入处理模块
//!
//! 使用 Windows XInput API 支持 Xbox 协议手柄；
//! 通过 ViGEmBus 创建虚拟手柄，把物理输入与宏按键合并后输出。

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU8, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::OnceLock;
use std::thread;
use std::time::Duration;
use vigem_client::{Client, TargetId, XButtons, XGamepad, Xbox360Wired};
use windows::core::{w, PCSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::UI::Input::XboxController::*;

/// 标准 XInputGetState 不回报 Xbox 键；ordinal 100 的 GetStateEx 可以
const XINPUT_GAMEPAD_GUIDE: u16 = 0x0400;

type XInputGetStateFn = unsafe extern "system" fn(u32, *mut XINPUT_STATE) -> u32;

fn xinput_get_state(index: u32, state: &mut XINPUT_STATE) -> u32 {
    static FN: OnceLock<Option<XInputGetStateFn>> = OnceLock::new();
    let f = *FN.get_or_init(|| unsafe {
        for dll in [w!("xinput1_4.dll"), w!("xinput1_3.dll")] {
            if let Ok(lib) = LoadLibraryW(dll) {
                // ordinal 100 = XInputGetStateEx，才能读到 Xbox/Guide 键
                if let Some(p) = GetProcAddress(lib, PCSTR(100usize as *const u8)) {
                    log::info!("使用 XInputGetStateEx 读取 Xbox/Guide 键");
                    return Some(std::mem::transmute(p));
                }
            }
        }
        log::info!("未找到 XInputGetStateEx，Xbox/Guide 键可能无法识别");
        None
    });
    unsafe {
        match f {
            Some(ex) => ex(index, state),
            None => XInputGetState(index, state),
        }
    }
}

use crate::config::{canonicalize_gamepad_key, Config, TriggerSource};

/// 手柄事件类型
#[derive(Debug, Clone)]
pub enum GamepadEvent {
    ButtonPressed { button: String },
    ButtonReleased { button: String },
}

static EXTRA_BUTTONS: AtomicU16 = AtomicU16::new(0);
static EXTRA_LT: AtomicU8 = AtomicU8::new(0);
static EXTRA_RT: AtomicU8 = AtomicU8::new(0);
static SUPPRESS_BUTTONS: AtomicU16 = AtomicU16::new(0);
static SUPPRESS_LT: AtomicBool = AtomicBool::new(false);
static SUPPRESS_RT: AtomicBool = AtomicBool::new(false);

/// 扳机按下判定阈值（0–255）
const TRIGGER_THRESHOLD: u8 = 30;
static VIGEM_WANTED: AtomicBool = AtomicBool::new(false);
static VIGEM_WARNED: AtomicBool = AtomicBool::new(false);
static VIGEM_OWNER: AtomicBool = AtomicBool::new(false);
/// 虚拟手柄占用的 XInput 槽，-1 表示未知。所有监听线程都必须跳过，否则会把宏输出再读成输入。
static VIRTUAL_INDEX: AtomicI32 = AtomicI32::new(-1);

fn skip_virtual_slot(index: u32, local: Option<u32>) -> bool {
    if local == Some(index) {
        return true;
    }
    let stored = VIRTUAL_INDEX.load(Ordering::Relaxed);
    stored >= 0 && stored as u32 == index
}

/// 根据配置决定是否启用虚拟手柄，并屏蔽 hold_loop 的物理触发键
pub fn apply_output_config(config: &Config) {
    VIGEM_WANTED.store(config.needs_virtual_gamepad(), Ordering::Relaxed);
    let mut suppress_lt = false;
    let mut suppress_rt = false;
    let suppress = config
        .hotkeys
        .iter()
        .filter(|h| h.enabled && h.action == "hold_loop")
        .filter_map(|h| match &h.trigger {
            TriggerSource::Gamepad { key } => match canonicalize_gamepad_key(key).as_str() {
                "LT" => {
                    suppress_lt = true;
                    None
                }
                "RT" => {
                    suppress_rt = true;
                    None
                }
                canonical => button_name_to_mask(canonical),
            },
            _ => None,
        })
        .fold(0u16, |acc, mask| acc | mask);
    SUPPRESS_BUTTONS.store(suppress, Ordering::Relaxed);
    SUPPRESS_LT.store(suppress_lt, Ordering::Relaxed);
    SUPPRESS_RT.store(suppress_rt, Ordering::Relaxed);
}

/// 手柄按键名 → XInput 按钮掩码（扳机 LT/RT 除外）
pub fn button_name_to_mask(name: &str) -> Option<u16> {
    Some(match name.to_ascii_uppercase().as_str() {
        "DUP" | "UP" => XINPUT_GAMEPAD_DPAD_UP.0,
        "DDOWN" | "DOWN" => XINPUT_GAMEPAD_DPAD_DOWN.0,
        "DLEFT" | "LEFT" => XINPUT_GAMEPAD_DPAD_LEFT.0,
        "DRIGHT" | "RIGHT" => XINPUT_GAMEPAD_DPAD_RIGHT.0,
        "START" | "MENU" => XINPUT_GAMEPAD_START.0,
        "BACK" | "VIEW" | "SELECT" => XINPUT_GAMEPAD_BACK.0,
        "LS" | "L3" | "LEFTTHUMB" => XINPUT_GAMEPAD_LEFT_THUMB.0,
        "RS" | "R3" | "RIGHTTHUMB" => XINPUT_GAMEPAD_RIGHT_THUMB.0,
        "LB" | "LEFTSHOULDER" => XINPUT_GAMEPAD_LEFT_SHOULDER.0,
        "RB" | "RIGHTSHOULDER" => XINPUT_GAMEPAD_RIGHT_SHOULDER.0,
        "A" => XINPUT_GAMEPAD_A.0,
        "B" => XINPUT_GAMEPAD_B.0,
        "X" => XINPUT_GAMEPAD_X.0,
        "Y" => XINPUT_GAMEPAD_Y.0,
        "GUIDE" | "XBOX" => XINPUT_GAMEPAD_GUIDE,
        _ => return None,
    })
}

/// 按下虚拟手柄按键（叠加到透传状态上）
pub fn press_gamepad_button(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    match canonicalize_gamepad_key(name).as_str() {
        "LT" => EXTRA_LT.store(255, Ordering::Relaxed),
        "RT" => EXTRA_RT.store(255, Ordering::Relaxed),
        canonical => {
            let mask = button_name_to_mask(canonical)
                .ok_or_else(|| format!("未知手柄按键: {}", name))?;
            EXTRA_BUTTONS.fetch_or(mask, Ordering::Relaxed);
        }
    }
    Ok(())
}

/// 释放虚拟手柄按键
pub fn release_gamepad_button(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    match canonicalize_gamepad_key(name).as_str() {
        "LT" => EXTRA_LT.store(0, Ordering::Relaxed),
        "RT" => EXTRA_RT.store(0, Ordering::Relaxed),
        canonical => {
            let mask = button_name_to_mask(canonical)
                .ok_or_else(|| format!("未知手柄按键: {}", name))?;
            EXTRA_BUTTONS.fetch_and(!mask, Ordering::Relaxed);
        }
    }
    Ok(())
}

/// 循环结束后清掉残留的宏按键
pub fn clear_extra_buttons() {
    EXTRA_BUTTONS.store(0, Ordering::Relaxed);
    EXTRA_LT.store(0, Ordering::Relaxed);
    EXTRA_RT.store(0, Ordering::Relaxed);
}

/// 启动手柄监听线程
///
/// 返回一个 Receiver，用于接收手柄事件
pub fn start_gamepad_thread() -> Receiver<GamepadEvent> {
    let (sender, receiver) = mpsc::channel::<GamepadEvent>();

    thread::spawn(move || {
        log::info!("手柄监听线程启动 (XInput)");

        let mut found_controller = false;
        for i in 0..4u32 {
            let mut state = XINPUT_STATE::default();
            let result = xinput_get_state(i, &mut state);
            if result == ERROR_SUCCESS.0 {
                log::info!("检测到手柄 [{}] 已连接", i);
                found_controller = true;
            }
        }

        if !found_controller {
            log::warn!("未检测到手柄，等待手柄连接...");
        }

        let mut prev_states: [u16; 4] = [0; 4];
        let mut prev_lt: [u8; 4] = [0; 4];
        let mut prev_rt: [u8; 4] = [0; 4];
        let mut controller_connected: [bool; 4] = [false; 4];
        let mut virtual_pad: Option<Xbox360Wired<Client>> = None;

        loop {
            if VIGEM_WANTED.load(Ordering::Relaxed)
                && virtual_pad.is_none()
                && VIGEM_OWNER
                    .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                    .is_ok()
            {
                match connect_virtual_pad() {
                    Ok(mut pad) => {
                        if let Ok(idx) = pad.get_user_index() {
                            VIRTUAL_INDEX.store(idx as i32, Ordering::Relaxed);
                            log::info!("虚拟手柄已就绪（XInput 槽 {}），请在游戏中选择该 Xbox 360 控制器", idx);
                        } else {
                            log::info!("虚拟手柄已就绪，请在游戏中选择该 Xbox 360 控制器");
                        }
                        virtual_pad = Some(pad);
                    }
                    Err(e) => {
                        VIGEM_OWNER.store(false, Ordering::SeqCst);
                        if !VIGEM_WARNED.swap(true, Ordering::Relaxed) {
                            log::warn!(
                                "无法创建虚拟手柄: {}。模拟 LB/RB/X 需要安装 ViGEmBus: https://github.com/nefarius/ViGEmBus/releases",
                                e
                            );
                        }
                    }
                }
            }

            let virtual_index = virtual_pad
                .as_mut()
                .and_then(|pad| pad.get_user_index().ok());
            if let Some(idx) = virtual_index {
                VIRTUAL_INDEX.store(idx as i32, Ordering::Relaxed);
            }

            let mut passthrough: Option<XINPUT_GAMEPAD> = None;

            for i in 0..4usize {
                if skip_virtual_slot(i as u32, virtual_index) {
                    continue;
                }

                let mut state = XINPUT_STATE::default();
                let result = xinput_get_state(i as u32, &mut state);

                if result == ERROR_SUCCESS.0 {
                    if !controller_connected[i] {
                        log::info!("手柄 [{}] 已连接", i);
                        controller_connected[i] = true;
                    }

                    let current_buttons = state.Gamepad.wButtons.0;
                    let changed = current_buttons ^ prev_states[i];

                    if changed != 0 {
                        check_button_changes(
                            i as u32,
                            prev_states[i],
                            current_buttons,
                            changed,
                            &sender,
                        );
                        prev_states[i] = current_buttons;
                    }

                    let lt = state.Gamepad.bLeftTrigger;
                    let rt = state.Gamepad.bRightTrigger;
                    check_trigger_edge(i as u32, "LT", prev_lt[i], lt, &sender);
                    check_trigger_edge(i as u32, "RT", prev_rt[i], rt, &sender);
                    prev_lt[i] = lt;
                    prev_rt[i] = rt;

                    if passthrough.is_none() {
                        passthrough = Some(state.Gamepad);
                    }
                } else if controller_connected[i] {
                    log::info!("手柄 [{}] 已断开", i);
                    controller_connected[i] = false;
                    prev_states[i] = 0;
                    prev_lt[i] = 0;
                    prev_rt[i] = 0;
                }
            }

            if let Some(pad) = virtual_pad.as_mut() {
                let extra = EXTRA_BUTTONS.load(Ordering::Relaxed);
                let suppress = SUPPRESS_BUTTONS.load(Ordering::Relaxed);
                let extra_lt = EXTRA_LT.load(Ordering::Relaxed);
                let extra_rt = EXTRA_RT.load(Ordering::Relaxed);
                let suppress_lt = SUPPRESS_LT.load(Ordering::Relaxed);
                let suppress_rt = SUPPRESS_RT.load(Ordering::Relaxed);

                let report = if let Some(gp) = passthrough {
                    XGamepad {
                        buttons: XButtons::from((gp.wButtons.0 & !suppress) | extra),
                        left_trigger: if suppress_lt { extra_lt } else { gp.bLeftTrigger.max(extra_lt) },
                        right_trigger: if suppress_rt { extra_rt } else { gp.bRightTrigger.max(extra_rt) },
                        thumb_lx: gp.sThumbLX,
                        thumb_ly: gp.sThumbLY,
                        thumb_rx: gp.sThumbRX,
                        thumb_ry: gp.sThumbRY,
                    }
                } else {
                    XGamepad {
                        buttons: XButtons::from(extra),
                        left_trigger: extra_lt,
                        right_trigger: extra_rt,
                        ..Default::default()
                    }
                };

                if let Err(e) = pad.update(&report) {
                    log::debug!("更新虚拟手柄失败: {:?}", e);
                }
            }

            thread::sleep(Duration::from_millis(16));
        }
    });

    receiver
}

fn connect_virtual_pad() -> Result<Xbox360Wired<Client>, String> {
    let client = Client::connect().map_err(|e| format!("{:?}", e))?;
    let mut target = Xbox360Wired::new(client, TargetId::XBOX360_WIRED);
    target.plugin().map_err(|e| format!("{:?}", e))?;
    target.wait_ready().map_err(|e| format!("{:?}", e))?;
    Ok(target)
}

/// 扳机模拟量过阈值时视为按下/松开
fn check_trigger_edge(
    controller_id: u32,
    name: &str,
    prev: u8,
    current: u8,
    sender: &mpsc::Sender<GamepadEvent>,
) {
    let was_down = prev >= TRIGGER_THRESHOLD;
    let is_down = current >= TRIGGER_THRESHOLD;
    if was_down == is_down {
        return;
    }
    if is_down {
        log::info!("手柄 [{}] 扳机按下: {} ({})", controller_id, name, current);
        if let Err(e) = sender.send(GamepadEvent::ButtonPressed {
            button: name.to_string(),
        }) {
            log::error!("发送扳机按下事件失败: {}", e);
        }
    } else {
        log::info!("手柄 [{}] 扳机释放: {} ({})", controller_id, name, current);
        if let Err(e) = sender.send(GamepadEvent::ButtonReleased {
            button: name.to_string(),
        }) {
            log::error!("发送扳机释放事件失败: {}", e);
        }
    }
}

/// 检查按钮变化并发送事件
fn check_button_changes(
    controller_id: u32,
    _prev: u16,
    current: u16,
    changed: u16,
    sender: &mpsc::Sender<GamepadEvent>,
) {
    let buttons: [(u16, &str); 15] = [
        (XINPUT_GAMEPAD_DPAD_UP.0, "DUp"),
        (XINPUT_GAMEPAD_DPAD_DOWN.0, "DDown"),
        (XINPUT_GAMEPAD_DPAD_LEFT.0, "DLeft"),
        (XINPUT_GAMEPAD_DPAD_RIGHT.0, "DRight"),
        (XINPUT_GAMEPAD_START.0, "Start"),
        (XINPUT_GAMEPAD_BACK.0, "Back"),
        (XINPUT_GAMEPAD_LEFT_THUMB.0, "LS"),
        (XINPUT_GAMEPAD_RIGHT_THUMB.0, "RS"),
        (XINPUT_GAMEPAD_LEFT_SHOULDER.0, "LB"),
        (XINPUT_GAMEPAD_RIGHT_SHOULDER.0, "RB"),
        (XINPUT_GAMEPAD_A.0, "A"),
        (XINPUT_GAMEPAD_B.0, "B"),
        (XINPUT_GAMEPAD_X.0, "X"),
        (XINPUT_GAMEPAD_Y.0, "Y"),
        (XINPUT_GAMEPAD_GUIDE, "Guide"),
    ];

    for (mask, name) in &buttons {
        if changed & mask != 0 {
            if current & mask != 0 {
                log::info!("手柄 [{}] 按钮按下: {}", controller_id, name);
                if let Err(e) = sender.send(GamepadEvent::ButtonPressed {
                    button: name.to_string(),
                }) {
                    log::error!("发送按钮按下事件失败: {}", e);
                }
            } else {
                log::info!("手柄 [{}] 按钮释放: {}", controller_id, name);
                if let Err(e) = sender.send(GamepadEvent::ButtonReleased {
                    button: name.to_string(),
                }) {
                    log::error!("发送按钮释放事件失败: {}", e);
                }
            }
        }
    }
}

/// 将 gilrs Button 映射为配置键名（保留此函数以兼容现有代码）
pub fn button_to_key_name(button: &str) -> String {
    match button {
        "South" => "A".to_string(),
        "East" => "B".to_string(),
        "West" => "X".to_string(),
        "North" => "Y".to_string(),
        "LeftTrigger" => "LT".to_string(),
        "RightTrigger" => "RT".to_string(),
        "LeftTrigger2" => "LB".to_string(),
        "RightTrigger2" => "RB".to_string(),
        "Select" => "Back".to_string(),
        "Start" => "Start".to_string(),
        "Mode" => "Guide".to_string(),
        "LeftThumb" => "LS".to_string(),
        "RightThumb" => "RS".to_string(),
        "DPadUp" => "DUp".to_string(),
        "DPadDown" => "DDown".to_string(),
        "DPadLeft" => "DLeft".to_string(),
        "DPadRight" => "DRight".to_string(),
        _ => button.to_string(),
    }
}
