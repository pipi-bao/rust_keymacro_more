//! 宏详情编辑模块
//!
//! 提供宏的触发源配置和操作类型选择

use crate::config::*;
use crate::visual_editor::VisualEditor;

/// 显示宏详情编辑（右侧面板）
pub fn show_macro_detail(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("宏详情编辑");
        ui.add_space(5.0);
        
        if let Some(idx) = editor.selected_macro {
            if idx < editor.config.hotkeys.len() {
                edit_macro_detail(editor, ui, idx, status_message, log_messages);
            }
        } else {
            ui.label("💡 提示：");
            ui.label("• 点击左侧宏列表项选择要编辑的宏");
            ui.label("• 在右侧编辑宏的详细配置");
        }
    });
}

/// 编辑宏详情
fn edit_macro_detail(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    ui.vertical(|ui| {
        ui.label(format!("编辑宏 #{}", idx + 1));
        ui.separator();
        
        // 触发源类型
        ui.label("触发源类型:");
        ui.horizontal(|ui| {
            ui.radio_value(&mut editor.edit_is_keyboard, true, "⌨ 键盘");
            ui.radio_value(&mut editor.edit_is_keyboard, false, "🎮 手柄");
        });
        
        ui.add_space(5.0);
        
        // 触发键 - 按钮打开键盘图
        ui.label("触发键:");
        let current_key = if editor.edit_is_keyboard {
            &editor.edit_keyboard_key
        } else {
            &editor.edit_gamepad_key
        };
        let key_label = if current_key.is_empty() {
            "点击选择...".to_string()
        } else {
            current_key.clone()
        };
        let btn_text = format!("🔑 {}", key_label);
        if ui.button(btn_text).clicked() {
            editor.show_key_selector_window = true;
        }
        
        ui.add_space(5.0);
        
        // 自动应用触发配置
        apply_trigger_config(editor, idx);
            
        ui.add_space(10.0);
        ui.separator();
        
        // 动作类型选择：序列 / 连发 / 按住循环
        ui.label("动作类型:");
        let current_action = editor.config.hotkeys.get(idx)
            .map(|h| h.action.as_str())
            .unwrap_or("sequence")
            .to_string();
        let mut new_action = current_action.clone();
        ui.horizontal(|ui| {
            ui.radio_value(&mut new_action, "sequence".to_string(), "📝 按键序列");
            ui.radio_value(&mut new_action, "auto_repeat".to_string(), "🔁 按键连发");
            ui.radio_value(&mut new_action, "hold_loop".to_string(), "🔄 按住循环");
        });
        if new_action != current_action {
            if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
                hotkey.action = new_action.clone();
                match new_action.as_str() {
                    "auto_repeat" => {
                        if !matches!(hotkey.params, ActionParams::AutoRepeat(_)) {
                            hotkey.params = ActionParams::AutoRepeat(AutoRepeatParams::default());
                        }
                    }
                    "hold_loop" => {
                        if !matches!(hotkey.params, ActionParams::HoldLoop(_)) {
                            hotkey.params = ActionParams::HoldLoop(HoldLoopParams::default());
                        }
                    }
                    _ => {
                        if !matches!(hotkey.params, ActionParams::Sequence(_)) {
                            hotkey.params = ActionParams::Sequence(SequenceParams { steps: vec![] });
                        }
                    }
                }
                editor.config_changed = true;
            }
        }

        ui.add_space(5.0);

        match new_action.as_str() {
            "auto_repeat" => {
                edit_auto_repeat_action(editor, ui, idx, status_message, log_messages);
            }
            "hold_loop" => {
                ui.label("🔄 按住触发键期间循环执行下列步骤，松开即停");
                super::step_editor::edit_sequence_action(editor, ui, idx, status_message, log_messages);
                edit_hold_loop_every(editor, ui, idx);
            }
            _ => {
                super::step_editor::edit_sequence_action(editor, ui, idx, status_message, log_messages);
            }
        }
    });
}

/// 编辑连发参数
fn edit_auto_repeat_action(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let ActionParams::AutoRepeat(params) = &mut editor.config.hotkeys[idx].params {
        ui.label("🔁 按键连发配置（按住触发键持续重复目标按键）");
        ui.separator();

        // 目标按键
        ui.label("连发目标按键:");
        let btn_text = format!("🔑 {}", if params.key.is_empty() { "点击选择...".to_string() } else { params.key.clone() });
        if ui.button(btn_text).clicked() {
            editor.step_editing_key = params.key.clone();
            editor.key_selector_for_step = true;
            editor.key_selector_macro_idx = idx;
            editor.key_selector_step_idx = usize::MAX; // 标记为连发目标键，非步骤
            editor.show_key_selector_window = true;
        }

        ui.add_space(5.0);

        // 按下持续时间
        ui.horizontal(|ui| {
            ui.label("按下时长 (ms):");
            let resp = ui.add(egui::DragValue::new(&mut params.press_ms).range(1..=1000).speed(1.0));
            if resp.drag_stopped() || resp.lost_focus() {
                editor.config_changed = true;
            }
        });

        // 释放间隔
        ui.horizontal(|ui| {
            ui.label("释放间隔 (ms):");
            let resp = ui.add(egui::DragValue::new(&mut params.release_ms).range(0..=1000).speed(1.0));
            if resp.drag_stopped() || resp.lost_focus() {
                editor.config_changed = true;
            }
        });

        ui.add_space(5.0);
        ui.label(format!("⏱ 实际触发频率约: 每 {} ms 触发一次", params.press_ms + params.release_ms));

        if ui.button("✓ 应用连发配置").clicked() {
            editor.config_changed = true;
            *status_message = "连发配置已更新".to_string();
            log_messages.push("[INFO] 更新连发配置".to_string());
        }
    }
}

/// 编辑按住循环的定时附加按键
fn edit_hold_loop_every(editor: &mut VisualEditor, ui: &mut egui::Ui, idx: usize) {
    ui.add_space(8.0);
    ui.separator();
    ui.label("⏱ 定时附加按键（循环期间按间隔触发）");

    let mut remove_at: Option<usize> = None;
    if let ActionParams::HoldLoop(params) = &mut editor.config.hotkeys[idx].params {
        for (i, action) in params.every.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", i + 1));
                let resp_key = ui.add(egui::TextEdit::singleline(&mut action.key).desired_width(50.0));
                ui.label("每");
                let resp_int = ui.add(egui::DragValue::new(&mut action.interval_ms).range(100..=600000).speed(100.0));
                ui.label("ms");
                ui.label("按下");
                let resp_press = ui.add(egui::DragValue::new(&mut action.press_ms).range(1..=1000).speed(1.0));
                ui.label("ms");
                let mut is_gamepad = action.device.as_deref() == Some("gamepad");
                let old = is_gamepad;
                ui.checkbox(&mut is_gamepad, "🎮 手柄");
                if is_gamepad != old {
                    action.device = if is_gamepad { Some("gamepad".to_string()) } else { None };
                    editor.config_changed = true;
                }
                if resp_key.changed() || resp_int.changed() || resp_press.changed() {
                    editor.config_changed = true;
                }
                if ui.small_button("🗑").clicked() {
                    remove_at = Some(i);
                }
            });
        }

        if ui.button("➕ 添加定时按键").clicked() {
            params.every.push(IntervalAction {
                key: "X".to_string(),
                interval_ms: 5000,
                press_ms: 50,
                device: Some("gamepad".to_string()),
            });
            editor.config_changed = true;
        }
    }

    if let Some(i) = remove_at {
        if let ActionParams::HoldLoop(params) = &mut editor.config.hotkeys[idx].params {
            if i < params.every.len() {
                params.every.remove(i);
                editor.config_changed = true;
            }
        }
    }
}

/// 自动应用触发配置（仅在值真正变化时触发更新）
fn apply_trigger_config(editor: &mut VisualEditor, idx: usize) {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        // 计算新的触发值
        let key = if editor.edit_is_keyboard {
            editor.edit_keyboard_key.clone()
        } else {
            editor.edit_gamepad_key.clone()
        };
        let new_trigger = if editor.edit_is_keyboard {
            TriggerSource::Keyboard { key }
        } else {
            TriggerSource::Gamepad { key }
        };
        
        // 检查触发源是否变化（TriggerSource 未实现 PartialEq，手动比较）
        let trigger_changed = match (&hotkey.trigger, &new_trigger) {
            (TriggerSource::Keyboard { key: a }, TriggerSource::Keyboard { key: b }) => a != b,
            (TriggerSource::Gamepad { key: a }, TriggerSource::Gamepad { key: b }) => a != b,
            _ => true, // 类型不同，肯定变了
        };
        
        if trigger_changed {
            hotkey.trigger = new_trigger;
            // 保持当前动作类型（sequence 或 auto_repeat），仅确保 params 类型与 action 匹配
            match hotkey.action.as_str() {
                "auto_repeat" => {
                    if !matches!(hotkey.params, ActionParams::AutoRepeat(_)) {
                        hotkey.params = ActionParams::AutoRepeat(AutoRepeatParams::default());
                    }
                }
                "hold_loop" => {
                    if !matches!(hotkey.params, ActionParams::HoldLoop(_)) {
                        hotkey.params = ActionParams::HoldLoop(HoldLoopParams::default());
                    }
                }
                _ => {
                    hotkey.action = "sequence".to_string();
                    if !matches!(hotkey.params, ActionParams::Sequence(_)) {
                        hotkey.params = ActionParams::Sequence(SequenceParams {
                            steps: vec![],
                        });
                    }
                }
            }
            editor.config_changed = true;
        }
    }
}
