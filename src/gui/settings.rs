//! 程序设置页面
//!
//! 提供全局快捷键的查看、修改、冲突检测与保存功能。

use eframe::egui;
use crate::config::GlobalHotkeyConfig;
use crate::global_hotkey;

/// 是否正在监听按键（捕获新的快捷键组合）
pub struct HotkeyCapture {
    active: bool,
    /// 捕获冲突时弹出的提示信息（非空则显示对话框）
    conflict: Option<String>,
    /// 是否刚成功应用了新快捷键（避免重复应用同一组合）
    applied: Option<GlobalHotkeyConfig>,
}

impl Default for HotkeyCapture {
    fn default() -> Self {
        Self {
            active: false,
            conflict: None,
            applied: None,
        }
    }
}

impl HotkeyCapture {
    /// 是否正在捕获新的全局快捷键（此时不应拦截 Ctrl+S）
    pub fn is_active(&self) -> bool {
        self.active
    }
}

/// 渲染设置页面
///
/// # 参数
///
/// * `ui` - egui UI 句柄
/// * `ctx` - egui Context（用于读取按键输入与弹窗）
/// * `capture` - 快捷键捕获状态
/// * `current` - 当前生效的快捷键配置
/// * `status` - 状态栏消息
///
/// # 返回值
///
/// 返回是否点击了"保存"按钮（调用方需持久化配置）。
pub fn show(
    ui: &mut egui::Ui,
    ctx: &egui::Context,
    capture: &mut HotkeyCapture,
    current: &mut GlobalHotkeyConfig,
    status: &mut String,
) -> bool {
    let mut save_pressed = false;

    ui.add_space(8.0);
    ui.heading("⚙️ 程序设置");
    ui.add_space(8.0);
    ui.separator();

    // 全局快捷键设置
    ui.add_space(8.0);
    ui.strong("全局快捷键");
    ui.label("用于在任意程序前台快速启用/禁用所有宏（切换总开关）。");
    ui.add_space(6.0);

    ui.horizontal(|ui| {
        ui.label("当前快捷键：");
        ui.monospace(current.display());
    });

    ui.add_space(8.0);

    if capture.active {
        // 捕获模式：监听按键组合
        ui.colored_label(egui::Color32::GOLD, "▶ 请按下新的快捷键组合（如 Ctrl+Alt+Q）...");
        ui.label("提示：包含修饰键（Ctrl/Alt/Shift/Win）和至少一个普通键。按 Esc 取消。");
        ui.add_space(6.0);
        if ui.button("取消").clicked() {
            capture.active = false;
        }
    } else {
        ui.horizontal(|ui| {
            if ui.button("✏️ 修改快捷键").clicked() {
                capture.active = true;
                capture.applied = None;
            }
            if ui.button("↺ 恢复默认 (Ctrl+Alt+Q)").clicked() {
                *current = GlobalHotkeyConfig::default();
                match global_hotkey::set_hotkey(global_hotkey::default_hotkey()) {
                    Ok(desc) => {
                        *status = format!("✓ 已恢复默认快捷键 {}", desc);
                    }
                    Err(e) => {
                        capture.conflict = Some(e);
                    }
                }
            }
        });
    }

    ui.add_space(12.0);
    ui.separator();

    ui.horizontal(|ui| {
        if ui.button("💾 保存设置").clicked() {
            save_pressed = true;
        }
    });

    // 处理按键捕获
    if capture.active {
        handle_capture(ctx, capture, current, status);
    }

    // 渲染冲突提示对话框
    if let Some(msg) = capture.conflict.clone() {
        let mut close = false;
        egui::Window::new("⚠️ 快捷键冲突")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label("该快捷键已被其他程序占用，无法注册：");
                ui.add_space(6.0);
                ui.monospace(&msg);
                ui.add_space(12.0);
                if ui.button("知道了").clicked() {
                    close = true;
                }
            });
        if close {
            capture.conflict = None;
        }
    }

    save_pressed
}

/// 处理快捷键捕获：读取当前帧的组合键输入
fn handle_capture(
    ctx: &egui::Context,
    capture: &mut HotkeyCapture,
    current: &mut GlobalHotkeyConfig,
    status: &mut String,
) {
    // 提取当前按下的组合
    let (main_key, modifiers) = ctx.input(|i| {
        let mut mods: Vec<&str> = Vec::new();
        if i.modifiers.ctrl {
            mods.push("ctrl");
        }
        if i.modifiers.alt {
            mods.push("alt");
        }
        if i.modifiers.shift {
            mods.push("shift");
        }
        if i.modifiers.command {
            mods.push("win");
        }

        let mut main: Option<String> = None;
        for event in i.events.iter() {
            if let egui::Event::Key { key, pressed: true, .. } = event {
                if let Some(name) = key_to_name(*key) {
                    main = Some(name);
                }
            }
        }
        (main, mods.join("+"))
    });

    // Esc 取消
    let esc = ctx.input(|i| {
        i.events
            .iter()
            .any(|e| matches!(e, egui::Event::Key { key: egui::Key::Escape, pressed: true, .. }))
    });
    if esc {
        capture.active = false;
        return;
    }

    if let Some(main_key) = main_key {
        // 必须至少包含一个修饰键，避免误触
        if !modifiers.is_empty() {
            let new_cfg = GlobalHotkeyConfig {
                modifiers,
                key: main_key,
            };
            // 避免对同一组合重复应用
            if capture.applied.as_ref() != Some(&new_cfg) {
                let hotkey = global_hotkey::hotkey_from_config(&new_cfg);
                match global_hotkey::set_hotkey(hotkey) {
                    Ok(desc) => {
                        *current = new_cfg.clone();
                        capture.applied = Some(new_cfg);
                        *status = format!("✓ 快捷键已更新为 {}", desc);
                        capture.active = false;
                    }
                    Err(e) => {
                        capture.conflict = Some(e);
                        capture.applied = Some(new_cfg); // 冲突也记录，避免重复弹窗
                    }
                }
            }
        }
    }
    ctx.request_repaint();
}

/// 将 egui 按键映射为配置中的键名
fn key_to_name(key: egui::Key) -> Option<String> {
    use egui::Key;
    Some(match key {
        Key::A => "a".into(), Key::B => "b".into(), Key::C => "c".into(),
        Key::D => "d".into(), Key::E => "e".into(), Key::F => "f".into(),
        Key::G => "g".into(), Key::H => "h".into(), Key::I => "i".into(),
        Key::J => "j".into(), Key::K => "k".into(), Key::L => "l".into(),
        Key::M => "m".into(), Key::N => "n".into(), Key::O => "o".into(),
        Key::P => "p".into(), Key::Q => "q".into(), Key::R => "r".into(),
        Key::S => "s".into(), Key::T => "t".into(), Key::U => "u".into(),
        Key::V => "v".into(), Key::W => "w".into(), Key::X => "x".into(),
        Key::Y => "y".into(), Key::Z => "z".into(),
        Key::Num0 => "0".into(), Key::Num1 => "1".into(), Key::Num2 => "2".into(),
        Key::Num3 => "3".into(), Key::Num4 => "4".into(), Key::Num5 => "5".into(),
        Key::Num6 => "6".into(), Key::Num7 => "7".into(), Key::Num8 => "8".into(),
        Key::Num9 => "9".into(),
        Key::F1 => "f1".into(), Key::F2 => "f2".into(), Key::F3 => "f3".into(),
        Key::F4 => "f4".into(), Key::F5 => "f5".into(), Key::F6 => "f6".into(),
        Key::F7 => "f7".into(), Key::F8 => "f8".into(), Key::F9 => "f9".into(),
        Key::F10 => "f10".into(), Key::F11 => "f11".into(), Key::F12 => "f12".into(),
        Key::Space => "space".into(), Key::Enter => "enter".into(),
        Key::Escape => "esc".into(), Key::Tab => "tab".into(),
        Key::Backspace => "backspace".into(), Key::Delete => "delete".into(),
        Key::Insert => "insert".into(), Key::Home => "home".into(), Key::End => "end".into(),
        Key::PageUp => "pageup".into(), Key::PageDown => "pagedown".into(),
        Key::ArrowUp => "up".into(), Key::ArrowDown => "down".into(),
        Key::ArrowLeft => "left".into(), Key::ArrowRight => "right".into(),
        _ => return None, // 忽略修饰键本身和其他无法映射的键
    })
}
