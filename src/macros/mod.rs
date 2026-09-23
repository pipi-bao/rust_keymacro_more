//! 键盘宏功能模块
//!
//! 从配置文件加载热键映射，支持多种操作类型

mod executor;
mod handler;

pub use executor::{execute_type_text, execute_sequence, execute_auto_repeat_once, execute_steps};
pub use handler::{keyboard_hook_proc, MacroEvent, MacroPhase, start_gamepad_forwarder};

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc::Sender};
use once_cell::sync::Lazy;
use windows::Win32::UI::WindowsAndMessaging::HHOOK;
use crate::config::Config;
use crate::gamepad::start_gamepad_thread;

// 全局变量
static TOGGLE_STATE: Lazy<Mutex<bool>> = Lazy::new(|| Mutex::new(false));
static MACRO_PHASE: Lazy<Mutex<MacroPhase>> = Lazy::new(|| Mutex::new(MacroPhase::Idle));
static MACRO_EVENT_SENDER: Lazy<Mutex<Option<Sender<MacroEvent>>>> = Lazy::new(|| Mutex::new(None));
static CONFIG: Lazy<Mutex<Option<Config>>> = Lazy::new(|| Mutex::new(None));
static SYSTEM_THREADS_STARTED: AtomicBool = AtomicBool::new(false);

/// 初始化键盘宏系统
///
/// # 参数
///
/// * `config` - 配置对象
///
/// # 返回
///
/// 成功返回钩子句柄，失败返回 None
///
/// # 说明
///
/// 设置低级键盘钩子监听全局键盘事件，启动宏处理线程和手柄监听线程
pub fn init_keyboard_macro_system(config: Config) -> Option<HHOOK> {
    set_config(config);

    // 保存/应用配置也会走到这里；线程只允许启动一次。
    // 多开监听会把虚拟手柄的输出再读成输入，RT 序列会自激循环。
    if !SYSTEM_THREADS_STARTED.swap(true, Ordering::SeqCst) {
        let macro_sender = handler::start_macro_thread();
        let gamepad_receiver = start_gamepad_thread();
        handler::start_gamepad_forwarder(gamepad_receiver, macro_sender);
    }

    match crate::winapi::keyboard::set_keyboard_hook(Some(handler::keyboard_hook_proc), 0) {
        Ok(hook) => Some(hook),
        Err(e) => {
            log::warn!("设置键盘钩子失败: {}", e);
            None
        }
    }
}

/// 设置配置（用于运行时重载）
pub fn set_config(config: Config) {
    handler::stop_all_auto_repeats();
    handler::stop_all_hold_loops();
    crate::gamepad::clear_extra_buttons();
    crate::gamepad::apply_output_config(&config);
    if let Ok(mut config_guard) = CONFIG.lock() {
        *config_guard = Some(config);
    }
}

/// 设置宏启用状态
///
/// # 参数
///
/// * `enabled` - true 启用，false 禁用
pub fn set_macro_enabled(enabled: bool) {
    if let Ok(mut state) = TOGGLE_STATE.lock() {
        *state = enabled;
    }
    if !enabled {
        handler::stop_all_auto_repeats();
        handler::stop_all_hold_loops();
        crate::gamepad::clear_extra_buttons();
    }
}

/// 切换宏总开关状态（取反）
///
/// 供全局快捷键回调使用，可在任意线程调用。
pub fn toggle_macro_state() {
    if let Ok(mut state) = TOGGLE_STATE.lock() {
        *state = !*state;
        let enabled = *state;
        let msg = if enabled { "已启用" } else { "已禁用" };
        log::info!("宏状态已通过全局快捷键切换为: {}", msg);
        if !enabled {
            drop(state);
            handler::stop_all_auto_repeats();
            handler::stop_all_hold_loops();
            crate::gamepad::clear_extra_buttons();
        }
    }
}

/// 清理键盘钩子
///
/// # 参数
///
/// * `hook` - 要卸载的钩子句柄
pub fn cleanup_keyboard_hook(hook: HHOOK) {
    if let Err(e) = crate::winapi::keyboard::unhook_keyboard_hook(hook) {
        log::debug!("卸载键盘钩子失败: {}", e);
    }
}

// 内部使用的全局访问函数
pub(crate) fn get_toggle_state() -> bool {
    TOGGLE_STATE.lock().map(|s| *s).unwrap_or(false)
}

pub(crate) fn get_macro_phase() -> MacroPhase {
    MACRO_PHASE.lock().map(|p| *p).unwrap_or(MacroPhase::Idle)
}

pub(crate) fn try_enter_executing() -> bool {
    if let Ok(mut p) = MACRO_PHASE.lock() {
        if *p == MacroPhase::Idle {
            *p = MacroPhase::Executing;
            return true;
        }
    }
    false
}

pub(crate) fn set_macro_phase(phase: MacroPhase) {
    if let Ok(mut p) = MACRO_PHASE.lock() {
        *p = phase;
    }
}

pub(crate) fn get_config() -> Option<Config> {
    CONFIG.lock().ok().and_then(|g| g.clone())
}

pub(crate) fn get_event_sender() -> Option<Sender<MacroEvent>> {
    MACRO_EVENT_SENDER.lock().ok().and_then(|g| g.clone())
}
