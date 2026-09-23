//! 宏列表管理模块
//!
//! 提供宏列表的显示、添加、删除和复制功能

use crate::config::*;
use crate::visual_editor::VisualEditor;

/// 显示宏列表（左侧面板）
pub fn show_macro_list(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.heading("宏列表");
        ui.add_space(5.0);
        
        // 宏列表滚动区域
        egui::ScrollArea::vertical()
            .id_salt("macro_list")
            .auto_shrink([false, false])
            .min_scrolled_height(200.0)
            .max_height(400.0)
            .show(ui, |ui| {
                if editor.config.hotkeys.is_empty() {
                    ui.label("暂无宏配置，点击下方'+'添加");
                }
                
                for i in 0..editor.config.hotkeys.len() {
                    let is_selected = editor.selected_macro == Some(i);
                    
                    // 获取触发源信息
                    let hotkey = &editor.config.hotkeys[i];
                    let is_keyboard = matches!(&hotkey.trigger, TriggerSource::Keyboard { .. });
                    let current_key = match &hotkey.trigger {
                        TriggerSource::Keyboard { key } => key.clone(),
                        TriggerSource::Gamepad { key } => key.clone(),
                    };
                    
                    // 构建显示文本（根据动作类型显示不同标记）
                    let action_label = match hotkey.action.as_str() {
                        "auto_repeat" => "🔁 连发",
                        "hold_loop" => "🔄 循环",
                        _ => "📝 序列",
                    };
                    let display_text = format!(
                        "{} {} → {}",
                        if is_keyboard { "⌨" } else { "🎮" },
                        current_key,
                        action_label,
                    );
                    
                    // 使用带背景的按钮，点击选中宏
                    let mut button = egui::Button::new(display_text);
                    
                    if is_selected {
                        button = button.fill(ui.style().visuals.selection.bg_fill);
                    }
                    
                    if ui.add(button).clicked() {
                        editor.selected_macro = Some(i);
                        // 加载选中宏的编辑状态
                        editor.load_edit_state(i);
                    }
                }
            });
        
        ui.add_space(10.0);
        
        // 添加/删除/复制按钮
        show_macro_buttons(editor, ui, status_message, log_messages);
    });
}

/// 显示宏操作按钮
fn show_macro_buttons(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    ui.horizontal(|ui| {
        if ui.button("➕ 添加").clicked() {
            editor.config.hotkeys.push(HotkeyConfig {
                trigger: TriggerSource::Keyboard { key: "F1".to_string() },
                action: "sequence".to_string(),
                params: ActionParams::Sequence(SequenceParams {
                    steps: vec![],
                }),
            });
            *status_message = "已添加新宏".to_string();
            log_messages.push("[INFO] 添加新宏配置".to_string());
        }
        
        if let Some(idx) = editor.selected_macro {
            if idx < editor.config.hotkeys.len() {
                if ui.button("📋 复制").clicked() {
                    let new_macro = editor.config.hotkeys[idx].clone();
                    editor.config.hotkeys.insert(idx + 1, new_macro);
                    *status_message = "已复制宏".to_string();
                    log_messages.push("[INFO] 复制宏配置".to_string());
                }
                
                if ui.button("❌ 删除").clicked() {
                    editor.config.hotkeys.remove(idx);
                    editor.selected_macro = None;
                    *status_message = "已删除宏".to_string();
                    log_messages.push("[INFO] 删除宏配置".to_string());
                }
            }
        }
    });
}
