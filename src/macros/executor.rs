//! 键盘宏执行模块
//!
//! 负责执行各种宏操作，包括输入文本、按键序列和鼠标操作

use rand::Rng;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use crate::config::{
    uses_gamepad_device, TypeTextParams, SequenceParams, Step, KeyAction,
    MouseAction as ConfigMouseAction, MouseButtonType, AutoRepeatParams, IntervalAction,
};
use crate::gamepad;
use crate::winapi::keyboard;
use crate::winapi::mouse;

/// 执行一轮按键连发（按下 + 释放）
///
/// 供连发线程循环调用。每次执行一轮完整的按下/释放。
pub fn execute_auto_repeat_once(params: &AutoRepeatParams) -> Result<(), Box<dyn std::error::Error>> {
    let vk = parse_key_string(&params.key)
        .ok_or_else(|| format!("无法解析连发按键: {}", params.key))?;

    keyboard::simulate_key_press(vk)?;
    thread::sleep(Duration::from_millis(params.press_ms));
    keyboard::simulate_key_release(vk)?;
    thread::sleep(Duration::from_millis(params.release_ms));

    Ok(())
}

/// 执行输入文本操作
pub fn execute_type_text(params: &TypeTextParams) -> Result<(), Box<dyn std::error::Error>> {
    // 输入每个字符
    for ch in params.text.chars() {
        // 获取当前字符的延迟
        let char_delay_ms = params.delay.as_ref().map_or(10, |d| d.get_delay());
        let char_delay = Duration::from_millis(char_delay_ms);
        
        if let Some(vk) = char_to_vk(ch) {
            keyboard::simulate_key_press(vk)?;
            thread::sleep(char_delay);
            keyboard::simulate_key_release(vk)?;
            thread::sleep(char_delay);
        } else {
            // 尝试发送 Unicode 字符
            simulate_unicode_char(ch)?;
        }
    }

    log::info!("序列执行完成");
    Ok(())
}

/// 可中断等待，松开触发键时尽快结束当前步骤
fn sleep_interruptible(ms: u64, stop: Option<&AtomicBool>) {
    if ms == 0 {
        return;
    }
    let Some(flag) = stop else {
        thread::sleep(Duration::from_millis(ms));
        return;
    };
    let deadline = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < deadline {
        if flag.load(Ordering::Relaxed) {
            return;
        }
        let remain = deadline.saturating_duration_since(Instant::now());
        thread::sleep(remain.min(Duration::from_millis(10)));
    }
}

fn should_stop(stop: Option<&AtomicBool>) -> bool {
    stop.map(|f| f.load(Ordering::Relaxed)).unwrap_or(false)
}

/// 执行一次按键（键盘或手柄）
pub fn execute_output_key(
    value: &str,
    device: Option<&str>,
    action: &KeyAction,
    delay_ms: u64,
    stop: Option<&AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    if uses_gamepad_device(device, value) {
        match action {
            KeyAction::Press => {
                gamepad::press_gamepad_button(value)?;
                sleep_interruptible(delay_ms, stop);
            }
            KeyAction::Release => {
                gamepad::release_gamepad_button(value)?;
                sleep_interruptible(delay_ms, stop);
            }
            KeyAction::Complete => {
                gamepad::press_gamepad_button(value)?;
                sleep_interruptible(delay_ms.max(1), stop);
                gamepad::release_gamepad_button(value)?;
            }
        }
        return Ok(());
    }

    let vk = parse_key_string(value).ok_or_else(|| format!("无法解析按键: {}", value))?;
    match action {
        KeyAction::Press => {
            keyboard::simulate_key_press(vk)?;
            sleep_interruptible(delay_ms, stop);
        }
        KeyAction::Release => {
            keyboard::simulate_key_release(vk)?;
            sleep_interruptible(delay_ms, stop);
        }
        KeyAction::Complete => {
            keyboard::simulate_key_press(vk)?;
            sleep_interruptible(delay_ms.max(1), stop);
            keyboard::simulate_key_release(vk)?;
        }
    }
    Ok(())
}

/// 执行定时附加按键（按下 + 释放）
pub fn execute_interval_action(
    action: &IntervalAction,
    stop: Option<&AtomicBool>,
) -> Result<(), Box<dyn std::error::Error>> {
    execute_output_key(
        &action.key,
        action.device.as_deref(),
        &KeyAction::Complete,
        action.press_ms,
        stop,
    )
}

/// 执行序列操作
pub fn execute_sequence(params: &SequenceParams) -> Result<(), Box<dyn std::error::Error>> {
    execute_steps(&params.steps, None)
}

/// 执行步骤列表，可选在停止标志置位时提前结束
pub fn execute_steps(steps: &[Step], stop: Option<&AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    log::debug!("开始执行序列，共 {} 个步骤", steps.len());
    for (idx, step) in steps.iter().enumerate() {
        if should_stop(stop) {
            log::debug!("序列被中断");
            return Ok(());
        }
        log::debug!("执行步骤 {}: {:?}", idx + 1, step);
        match step {
            Step::Key { value, delay, action, device } => {
                let key_action = action.as_ref().unwrap_or(&KeyAction::Complete);
                let delay_ms = delay.as_ref().map(|d| d.get_delay()).unwrap_or(0);
                log::debug!("按键: {}, 动作: {:?}, device: {:?}", value, key_action, device);
                if let Err(e) = execute_output_key(value, device.as_deref(), key_action, delay_ms, stop) {
                    log::warn!("按键执行失败 ({}): {}", value, e);
                }
            }
            Step::Wait { value } => {
                sleep_interruptible(*value, stop);
            }
            Step::WaitRandom { min, max } => {
                let actual_delay = rand::thread_rng().gen_range(*min..=*max);
                sleep_interruptible(actual_delay, stop);
            }
            Step::Text { value, delay } => {
                for ch in value.chars() {
                    if let Some(vk) = char_to_vk(ch) {
                        keyboard::simulate_key_press(vk)?;
                        if let Some(d) = delay {
                            thread::sleep(Duration::from_millis(d.get_delay()));
                        }
                        keyboard::simulate_key_release(vk)?;
                    } else {
                        simulate_unicode_char(ch)?;
                    }
                }
            }
            Step::MouseClick { button, delay } => {
                let mouse_button = match button {
                    MouseButtonType::Left => mouse::MouseButton::Left,
                    MouseButtonType::Right => mouse::MouseButton::Right,
                    MouseButtonType::Middle => mouse::MouseButton::Middle,
                };
                log::debug!("鼠标点击: {:?}", button);
                mouse::mouse_click(mouse_button)?;
                if let Some(d) = delay {
                    thread::sleep(Duration::from_millis(d.get_delay()));
                }
            }
            Step::MouseAction { button, action, delay } => {
                let mouse_button = match button {
                    MouseButtonType::Left => mouse::MouseButton::Left,
                    MouseButtonType::Right => mouse::MouseButton::Right,
                    MouseButtonType::Middle => mouse::MouseButton::Middle,
                };
                match action {
                    ConfigMouseAction::Click => {
                        log::debug!("鼠标点击: {:?}", button);
                        mouse::mouse_click(mouse_button)?;
                    }
                    ConfigMouseAction::Down => {
                        log::debug!("鼠标按下: {:?}", button);
                        mouse::mouse_down(&mouse_button)?;
                    }
                    ConfigMouseAction::Up => {
                        log::debug!("鼠标释放: {:?}", button);
                        mouse::mouse_up(&mouse_button)?;
                    }
                }
                if let Some(d) = delay {
                    thread::sleep(Duration::from_millis(d.get_delay()));
                }
            }
            Step::MouseMove { x, y, relative, delay } => {
                if relative == &Some(true) {
                    log::debug!("鼠标相对移动: ({}, {})", x, y);
                    mouse::move_mouse_relative(*x, *y)?;
                } else {
                    log::debug!("鼠标绝对移动到: ({}, {})", x, y);
                    mouse::move_mouse_to(*x, *y)?;
                }
                if let Some(d) = delay {
                    thread::sleep(Duration::from_millis(d.get_delay()));
                }
            }
            Step::MouseWheel { delta, delay } => {
                log::debug!("鼠标滚轮: {}", delta);
                mouse::mouse_wheel(*delta)?;
                if let Some(d) = delay {
                    thread::sleep(Duration::from_millis(d.get_delay()));
                }
            }
        }
    }

    log::debug!("序列执行完成");
    Ok(())
}

/// 将字符转换为虚拟键码
fn char_to_vk(ch: char) -> Option<u16> {
    match ch {
        'a'..='z' => Some(ch as u16 - 'a' as u16 + 0x41),
        'A'..='Z' => Some(ch as u16 - 'A' as u16 + 0x41),
        '0'..='9' => Some(ch as u16 - '0' as u16 + 0x30),
        ' ' => Some(windows::Win32::UI::Input::KeyboardAndMouse::VK_SPACE.0),
        '\r' | '\n' => Some(windows::Win32::UI::Input::KeyboardAndMouse::VK_RETURN.0),
        '\t' => Some(windows::Win32::UI::Input::KeyboardAndMouse::VK_TAB.0),
        _ => None,
    }
}

/// 模拟 Unicode 字符输入（备用方案）
fn simulate_unicode_char(_ch: char) -> Result<(), Box<dyn std::error::Error>> {
    // 这里可以实现 Unicode 字符输入，使用 SendInput 的 Unicode 模式
    // 为简化实现，这里暂时返回错误
    Err("Unicode 字符不支持".into())
}

/// 将键名字符串解析为虚拟键码
fn parse_key_string(key: &str) -> Option<u16> {
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    
    match key.to_uppercase().as_str() {
        "A" => Some(0x41),
        "B" => Some(0x42),
        "C" => Some(0x43),
        "D" => Some(0x44),
        "E" => Some(0x45),
        "F" => Some(0x46),
        "G" => Some(0x47),
        "H" => Some(0x48),
        "I" => Some(0x49),
        "J" => Some(0x4A),
        "K" => Some(0x4B),
        "L" => Some(0x4C),
        "M" => Some(0x4D),
        "N" => Some(0x4E),
        "O" => Some(0x4F),
        "P" => Some(0x50),
        "Q" => Some(0x51),
        "R" => Some(0x52),
        "S" => Some(0x53),
        "T" => Some(0x54),
        "U" => Some(0x55),
        "V" => Some(0x56),
        "W" => Some(0x57),
        "X" => Some(0x58),
        "Y" => Some(0x59),
        "Z" => Some(0x5A),
        s if s.len() == 1 && s.chars().next().unwrap().is_ascii_digit() => {
            s.chars().next().map(|c| c as u16 - '0' as u16 + 0x30)
        }
        "SPACE" | "Space" => Some(VK_SPACE.0),
        "ENTER" | "Enter" => Some(VK_RETURN.0),
        "TAB" | "Tab" => Some(VK_TAB.0),
        "BACKSPACE" | "Backspace" => Some(VK_BACK.0),
        "ESC" | "Escape" => Some(VK_ESCAPE.0),
        "SHIFT" | "Shift" => Some(VK_SHIFT.0),
        "CTRL" | "Ctrl" => Some(VK_CONTROL.0),
        "ALT" | "Alt" => Some(VK_MENU.0),
        // 方向键
        "UP" | "Up" => Some(VK_UP.0),
        "DOWN" | "Down" => Some(VK_DOWN.0),
        "LEFT" | "Left" => Some(VK_LEFT.0),
        "RIGHT" | "Right" => Some(VK_RIGHT.0),
        // 导航/编辑键
        "INSERT" | "Insert" => Some(VK_INSERT.0),
        "HOME" | "Home" => Some(VK_HOME.0),
        "PAGEUP" | "PageUp" => Some(VK_PRIOR.0),
        "DELETE" | "Delete" => Some(VK_DELETE.0),
        "END" | "End" => Some(VK_END.0),
        "PAGEDOWN" | "PageDown" => Some(VK_NEXT.0),
        "CAPSLOCK" | "CapsLock" => Some(VK_CAPITAL.0),
        "WIN" | "Win" => Some(VK_LWIN.0),
        "MENU" | "Menu" => Some(VK_APPS.0),
        // 数字小键盘
        "NUMPAD0" => Some(VK_NUMPAD0.0),
        "NUMPAD1" => Some(VK_NUMPAD1.0),
        "NUMPAD2" => Some(VK_NUMPAD2.0),
        "NUMPAD3" => Some(VK_NUMPAD3.0),
        "NUMPAD4" => Some(VK_NUMPAD4.0),
        "NUMPAD5" => Some(VK_NUMPAD5.0),
        "NUMPAD6" => Some(VK_NUMPAD6.0),
        "NUMPAD7" => Some(VK_NUMPAD7.0),
        "NUMPAD8" => Some(VK_NUMPAD8.0),
        "NUMPAD9" => Some(VK_NUMPAD9.0),
        // 功能键 F1-F24
        s if s.starts_with('F') && s.len() > 1
            && s[1..].parse::<u32>().map(|n| (1..=24).contains(&n)).unwrap_or(false) => {
            Some(s[1..].parse::<u32>().map(|n| (0x70 + n - 1) as u16).unwrap())
        }
        _ => None,
    }
}
