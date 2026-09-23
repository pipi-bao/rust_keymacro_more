//! 步骤编辑模块
//!
//! 提供按键序列的步骤编辑和详情编辑功能

use crate::config::*;
use crate::visual_editor::VisualEditor;
use egui::Id;

/// 编辑按键序列操作
pub fn edit_sequence_action(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    ui.label("执行序列:");
    ui.separator();
    
    // 显示步骤数量
    if let Some(steps) = editor.config.hotkeys[idx].params.steps() {
        ui.label(format!("📊 步骤数量: {}", steps.len()));
    }
    
    // 步骤列表
    show_step_list(editor, ui, idx);
    
    ui.add_space(10.0);
    
    // 添加步骤按钮和删除按钮
    let mut added_idx: Option<usize> = None;
    ui.horizontal(|ui| {
        ui.menu_button("➕ 添加步骤", |ui| {
            if ui.button("⌨ 按键").clicked() {
                added_idx = add_key_step(editor, idx, status_message, log_messages);
                ui.close_menu();
            }
            
            // ⏱ 等待：可点击按钮触发添加，后跟可调的等待时间输入
            let wait_id = Id::new(("pending_wait_ms", idx));
            let mut wait_val = ui.data(|d| d.get_temp::<u64>(wait_id).unwrap_or(100));
            ui.horizontal(|ui| {
                if ui.button("⏱ 等待:").clicked() {
                    added_idx = add_wait_step_with_value(editor, idx, wait_val, status_message, log_messages);
                    ui.close_menu();
                }
                ui.add(egui::DragValue::new(&mut wait_val).range(1..=60000).speed(10.0));
                ui.label("ms");
                ui.data_mut(|d| d.insert_temp(wait_id, wait_val));
            });
            
            // 🎲 随机延迟：可点击按钮触发添加，后跟可调的 min/max 范围输入
            let mut r_min = ui.data(|d| d.get_temp::<u64>(Id::new(("r_min", idx))).unwrap_or(50));
            let mut r_max = ui.data(|d| d.get_temp::<u64>(Id::new(("r_max", idx))).unwrap_or(200));
            ui.horizontal(|ui| {
                if ui.button("🎲 随机延迟:").clicked() {
                    added_idx = add_wait_random_step(editor, idx, r_min, r_max, status_message, log_messages);
                    ui.close_menu();
                }
                ui.label("min:");
                ui.add(egui::DragValue::new(&mut r_min).range(1..=60000).speed(10.0));
                ui.label("max:");
                ui.add(egui::DragValue::new(&mut r_max).range(1..=60000).speed(10.0));
                if r_min > r_max { r_max = r_min; }
                ui.data_mut(|d| d.insert_temp(Id::new(("r_min", idx)), r_min));
                ui.data_mut(|d| d.insert_temp(Id::new(("r_max", idx)), r_max));
            });
            
            if ui.button("✍ 文本").clicked() {
                added_idx = add_text_step(editor, idx, status_message, log_messages);
                ui.close_menu();
            }
            ui.menu_button("🖱 鼠标点击", |ui| {
                if ui.button("左键").clicked() {
                    added_idx = add_mouse_click_step(editor, idx, MouseButtonType::Left, status_message, log_messages);
                    ui.close_menu();
                }
                if ui.button("右键").clicked() {
                    added_idx = add_mouse_click_step(editor, idx, MouseButtonType::Right, status_message, log_messages);
                    ui.close_menu();
                }
                if ui.button("中键").clicked() {
                    added_idx = add_mouse_click_step(editor, idx, MouseButtonType::Middle, status_message, log_messages);
                    ui.close_menu();
                }
            });
        });
        
    });
    
    // 自动选中新添加的步骤
    if let Some(step_idx) = added_idx {
        ui.data_mut(|data| {
            data.insert_temp(Id::new("selected_step"), step_idx);
        });
    }
    
    // 编辑选中的步骤
    if let Some(step_idx) = ui.data(|data| data.get_temp::<usize>(Id::new("selected_step"))) {
        ui.separator();
        ui.label("✏ 编辑步骤详情:");

        // 获取总步骤数，用于启用/禁用按钮和键盘
        let total_steps = editor.config.hotkeys[idx].params.steps().map(|s| s.len()).unwrap_or(0);

        // 上移/下移按钮
        let mut new_selected: Option<usize> = None;

        ui.horizontal(|ui| {
            let up_enabled = step_idx > 0;
            let down_enabled = step_idx + 1 < total_steps;

            if ui.add_enabled(up_enabled, egui::Button::new("⬆ 上移")).clicked() {
                new_selected = Some(step_idx - 1);
            }
            if ui.add_enabled(down_enabled, egui::Button::new("⬇ 下移")).clicked() {
                new_selected = Some(step_idx + 1);
            }
        });

        // 键盘快捷键（上下箭头调整步骤顺序）
        let key_up = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
        let key_down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));

        if step_idx > 0 && key_up {
            new_selected = Some(step_idx - 1);
        } else if step_idx + 1 < total_steps && key_down {
            new_selected = Some(step_idx + 1);
        }

        // 执行移动操作
        if let Some(new_idx) = new_selected {
            if let Some(steps) = editor.config.hotkeys[idx].params.steps_mut() {
                steps.swap(step_idx, new_idx);
                editor.config_changed = true;
                ui.data_mut(|data| {
                    data.insert_temp(Id::new("selected_step"), new_idx);
                });
                *status_message = format!("步骤 {} ↔ {}", step_idx + 1, new_idx + 1);
                log_messages.push(format!("[INFO] 步骤 {} 与 {} 交换位置", step_idx + 1, new_idx + 1));
            }
            edit_step_detail(editor, ui, idx, new_idx, status_message, log_messages);
        } else {
            edit_step_detail(editor, ui, idx, step_idx, status_message, log_messages);
        }
    }
}

/// 显示步骤列表
fn show_step_list(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    idx: usize,
) {
    let mut delete_idx: Option<usize> = None;
    // 待处理的按键选择面板打开请求（在可变借用循环完成后使用）
    let mut pending_key_selector: Option<usize> = None;
    egui::ScrollArea::vertical()
        .id_salt("step_list")
        .auto_shrink([false, false])
        .min_scrolled_height(100.0)
        .max_height(ui.available_height() - 150.0)
        .show(ui, |ui| {
            if let Some(steps) = editor.config.hotkeys[idx].params.steps_mut() {
                for (i, step) in steps.iter_mut().enumerate() {
                    let is_selected = ui.data(|data| data.get_temp::<usize>(Id::new("selected_step")) == Some(i));

                    let frame = egui::Frame::none()
                        .fill(if is_selected {
                            egui::Color32::from_rgba_premultiplied(50, 100, 200, 80)
                        } else {
                            egui::Color32::TRANSPARENT
                        });

                    frame.show(ui, |ui| {
                        // 注册背景点击（在子控件之前注册，子控件优先响应）
                        let bg_id = ui.id().with("step_row_bg").with(i);
                        let bg_clicked = ui.interact(
                            ui.max_rect(), bg_id, egui::Sense::click()
                        ).clicked();

                        ui.horizontal(|ui| {
                            match step {
                                Step::Key { value, device, .. } => {
                                    let icon = if device.as_deref() == Some("gamepad") { "🎮 按键:" } else { "⌨ 按键:" };
                                    let label = ui.add(egui::Label::new(icon).sense(egui::Sense::click()));
                                    if label.clicked() {
                                        ui.data_mut(|data| {
                                            if is_selected {
                                                data.remove::<usize>(Id::new("selected_step"));
                                            } else {
                                                data.insert_temp(Id::new("selected_step"), i);
                                            }
                                        });
                                    }
                                    // 使用按钮打开可视化按键选择面板
                                    let btn_text = format!("🔑 {}", value);
                                    if ui.button(btn_text).clicked() {
                                        pending_key_selector = Some(i);
                                    }
                                }
                                Step::Wait { value } => {
                                    let label = ui.add(egui::Label::new("⏱ 等待:").sense(egui::Sense::click()));
                                    if label.clicked() {
                                        ui.data_mut(|data| {
                                            if is_selected {
                                                data.remove::<usize>(Id::new("selected_step"));
                                            } else {
                                                data.insert_temp(Id::new("selected_step"), i);
                                            }
                                        });
                                    }
                                    let resp = ui.add(egui::DragValue::new(value).range(1..=60000).speed(10.0));
                                    if resp.drag_stopped() || resp.lost_focus() {
                                        editor.config_changed = true;
                                    }
                                    ui.label("ms");
                                }
                                Step::WaitRandom { min, max } => {
                                    let label = ui.add(egui::Label::new("🎲 随机延迟:").sense(egui::Sense::click()));
                                    if label.clicked() {
                                        ui.data_mut(|data| {
                                            if is_selected {
                                                data.remove::<usize>(Id::new("selected_step"));
                                            } else {
                                                data.insert_temp(Id::new("selected_step"), i);
                                            }
                                        });
                                    }
                                    ui.label("min:");
                                    let resp = ui.add(egui::DragValue::new(min).range(1..=60000).speed(10.0));
                                    if resp.drag_stopped() || resp.lost_focus() {
                                        if *min > *max { *max = *min; }
                                        editor.config_changed = true;
                                    }
                                    ui.label("max:");
                                    let resp = ui.add(egui::DragValue::new(max).range(1..=60000).speed(10.0));
                                    if resp.drag_stopped() || resp.lost_focus() {
                                        if *max < *min { *min = *max; }
                                        editor.config_changed = true;
                                    }
                                    ui.label("ms");
                                }
                                _ => {
                                    let text = match step {
                                        Step::Text { value, .. } => format!("✍ 文本: \"{}\"", value),
                                        Step::MouseClick { button, .. } => format!("🖱 鼠标{}键点击",
                                            match button {
                                                MouseButtonType::Left => "左",
                                                MouseButtonType::Right => "右",
                                                MouseButtonType::Middle => "中",
                                            }),
                                        Step::MouseAction { button, action, .. } => format!("🖱 鼠标{}键 {:?}",
                                            match button {
                                                MouseButtonType::Left => "左",
                                                MouseButtonType::Right => "右",
                                                MouseButtonType::Middle => "中",
                                            }, action),
                                        Step::MouseMove { x, y, relative, .. } => {
                                            if *relative == Some(true) {
                                                format!("🖱 相对移动: ({}, {})", x, y)
                                            } else {
                                                format!("🖱 移动到: ({}, {})", x, y)
                                            }
                                        }
                                        Step::MouseWheel { delta, .. } => {
                                            if *delta > 0 {
                                                format!("🖱 滚轮向上: {}", delta)
                                            } else {
                                                format!("🖱 滚轮向下: {}", delta.abs())
                                            }
                                        }
                                        _ => String::new(),
                                    };

                                    let label = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if label.clicked() {
                                        ui.data_mut(|data| {
                                            if data.get_temp::<usize>(Id::new("selected_step")) == Some(i) {
                                                data.remove::<usize>(Id::new("selected_step"));
                                            } else {
                                                data.insert_temp(Id::new("selected_step"), i);
                                            }
                                        });
                                    }
                                }
                            }

                            // 每个步骤行上的删除按钮，右对齐
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if ui.small_button("🗑").clicked() {
                                    delete_idx = Some(i);
                                }
                            });
                        });

                        // 点击行空白区域也能选中/取消选中
                        // 背景点击在子控件之前注册，子控件（Label、Button）注册更晚，
                        // 所以子控件优先响应，只有行内空白区域才触发背景选中
                        if bg_clicked {
                            ui.data_mut(|data| {
                                if is_selected {
                                    data.remove::<usize>(Id::new("selected_step"));
                                } else {
                                    data.insert_temp(Id::new("selected_step"), i);
                                }
                            });
                        }
                    });

                }
            }
        });

    // 处理步骤删除（在循环外执行可变操作）
    if let Some(step_idx) = delete_idx {
        remove_step(editor, idx, step_idx, &mut String::new(), &mut Vec::new());
        // 清除选中状态
        ui.data_mut(|data| {
            data.remove::<usize>(Id::new("selected_step"));
        });
    }

    // 处理按键选择面板的打开请求（在循环外，避免借用冲突）
    if let Some(step_idx) = pending_key_selector {
        // 获取当前键值
        let mut for_gamepad = false;
        if let Some(steps) = editor.config.hotkeys[idx].params.steps() {
            if let Some(Step::Key { value, device, .. }) = steps.get(step_idx) {
                editor.step_editing_key = value.clone();
                for_gamepad = device.as_deref() == Some("gamepad")
                    || crate::config::is_exclusive_gamepad_button(value);
            }
        }
        editor.key_selector_for_step = true;
        editor.key_selector_for_gamepad = for_gamepad;
        editor.key_selector_macro_idx = idx;
        editor.key_selector_step_idx = step_idx;
        editor.show_key_selector_window = true;
    }
}

/// 添加按键步骤
fn add_key_step(
    editor: &mut VisualEditor,
    idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) -> Option<usize> {
    let device = editor.config.hotkeys.get(idx).and_then(|hk| {
        if hk.action == "hold_loop" && matches!(hk.trigger, TriggerSource::Gamepad { .. }) {
            Some("gamepad".to_string())
        } else {
            None
        }
    });
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            steps.push(Step::Key {
                value: if device.is_some() { "LB".to_string() } else { "A".to_string() },
                delay: Some(DelayConfig::Fixed(50)),
                action: Some(KeyAction::Complete),
                device,
            });
            let new_idx = steps.len() - 1;
            *status_message = "已添加按键步骤".to_string();
            log_messages.push("[INFO] 添加按键步骤".to_string());
            return Some(new_idx);
        }
    }
    None
}

/// 添加等待步骤
fn add_wait_step_with_value(
    editor: &mut VisualEditor,
    idx: usize,
    wait_ms: u64,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) -> Option<usize> {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            steps.push(Step::Wait {
                value: wait_ms,
            });
            let new_idx = steps.len() - 1;
            *status_message = format!("已添加等待步骤 ({}ms)", wait_ms);
            log_messages.push(format!("[INFO] 添加等待步骤 ({}ms)", wait_ms));
            return Some(new_idx);
        }
    }
    None
}

/// 添加随机等待步骤
fn add_wait_random_step(
    editor: &mut VisualEditor,
    idx: usize,
    min_ms: u64,
    max_ms: u64,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) -> Option<usize> {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            steps.push(Step::WaitRandom {
                min: min_ms,
                max: max_ms,
            });
            let new_idx = steps.len() - 1;
            *status_message = format!("已添加随机延迟步骤 ({}~{}ms)", min_ms, max_ms);
            log_messages.push(format!("[INFO] 添加随机延迟步骤 ({}~{}ms)", min_ms, max_ms));
            return Some(new_idx);
        }
    }
    None
}

/// 添加文本步骤
fn add_text_step(
    editor: &mut VisualEditor,
    idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) -> Option<usize> {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            steps.push(Step::Text {
                value: "hello".to_string(),
                delay: Some(DelayConfig::Fixed(10)),
            });
            let new_idx = steps.len() - 1;
            *status_message = "已添加文本步骤".to_string();
            log_messages.push("[INFO] 添加文本步骤".to_string());
            return Some(new_idx);
        }
    }
    None
}

/// 添加鼠标点击步骤
fn add_mouse_click_step(
    editor: &mut VisualEditor,
    idx: usize,
    button: MouseButtonType,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) -> Option<usize> {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            let btn_label = match button {
                MouseButtonType::Left => "左键",
                MouseButtonType::Right => "右键",
                MouseButtonType::Middle => "中键",
            };
            steps.push(Step::MouseClick {
                button,
                delay: Some(DelayConfig::Fixed(50)),
            });
            let new_idx = steps.len() - 1;
            *status_message = format!("已添加鼠标{}点击步骤", btn_label);
            log_messages.push(format!("[INFO] 添加鼠标{}点击步骤", btn_label));
            return Some(new_idx);
        }
    }
    None
}

/// 删除步骤
fn remove_step(
    editor: &mut VisualEditor,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(hotkey) = editor.config.hotkeys.get_mut(macro_idx) {
        if let Some(steps) = hotkey.params.steps_mut() {
            if step_idx < steps.len() {
                steps.remove(step_idx);
                *status_message = "已删除步骤".to_string();
                log_messages.push("[INFO] 删除步骤".to_string());
            }
        }
    }
}

/// 编辑步骤详情
fn edit_step_detail(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    // 使用索引访问而不是直接借用 step，避免借用冲突
    let step_type = if let Some(params) = editor.config.hotkeys[macro_idx].params.steps() {
        if step_idx < params.len() {
            match &params[step_idx] {
                Step::Key { .. } => 0,
                Step::Wait { .. } => 1,
                Step::Text { .. } => 2,
                Step::MouseClick { .. } => 3,
                Step::MouseAction { .. } => 4,
                Step::MouseMove { .. } => 5,
                Step::MouseWheel { .. } => 6,
                Step::WaitRandom { .. } => 7,
            }
        } else {
            return;
        }
    } else {
        return;
    };
    
    match step_type {
        0 => edit_key_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        1 => edit_wait_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        2 => edit_text_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        3 => edit_mouse_click_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        4 => edit_mouse_action_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        5 => edit_mouse_move_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        6 => edit_mouse_wheel_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        7 => edit_wait_random_step(editor, ui, macro_idx, step_idx, status_message, log_messages),
        _ => {}
    }
}

/// 编辑按键步骤
fn edit_key_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::Key { delay, action, device, .. } = &mut params[step_idx] {
            ui.label("按键配置:");

            ui.horizontal(|ui| {
                ui.label("输出:");
                let mut is_gamepad = device.as_deref() == Some("gamepad");
                let old = is_gamepad;
                ui.radio_value(&mut is_gamepad, false, "⌨ 键盘");
                ui.radio_value(&mut is_gamepad, true, "🎮 手柄");
                if is_gamepad != old {
                    *device = if is_gamepad { Some("gamepad".to_string()) } else { None };
                    editor.config_changed = true;
                }
            });
            
            let mut action_str = match action {
                Some(KeyAction::Press) => "press".to_string(),
                Some(KeyAction::Release) => "release".to_string(),
                _ => "complete".to_string(),
            };
            let old_action = action_str.clone();
            ui.horizontal(|ui| {
                ui.radio_value(&mut action_str, "press".to_string(), "按下");
                ui.radio_value(&mut action_str, "release".to_string(), "释放");
                ui.radio_value(&mut action_str, "complete".to_string(), "完整(按下+释放)");
            });
            if action_str != old_action {
                let new_action = match action_str.as_str() {
                    "press" => KeyAction::Press,
                    "release" => KeyAction::Release,
                    _ => KeyAction::Complete,
                };
                // 切换到非 Complete 模式时清除延迟，切换到 Complete 时设置默认值
                if !matches!(new_action, KeyAction::Complete) {
                    *delay = None;
                } else if delay.is_none() {
                    *delay = Some(DelayConfig::Fixed(50));
                }
                *action = Some(new_action);
                editor.config_changed = true;
                *status_message = "按键动作已更新".to_string();
                log_messages.push("[INFO] 更新按键动作".to_string());
            }
            
            // 只在"完成(按下+释放)"模式下显示延迟输入
            if matches!(action, Some(KeyAction::Complete)) {
                ui.horizontal(|ui| {
                    ui.label("持续时间 (ms):");
                    let mut val = match delay {
                        Some(DelayConfig::Fixed(ms)) => *ms,
                        _ => 50,
                    };
                    let resp = ui.add(egui::DragValue::new(&mut val).range(1..=10000).speed(10.0));
                    *delay = Some(DelayConfig::Fixed(val));
                    if resp.drag_stopped() || resp.lost_focus() {
                        editor.config_changed = true;
                    }
                    ui.label("ms");
                });
            }
        }
    }
}

/// 编辑等待步骤
fn edit_wait_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::Wait { value } = &mut params[step_idx] {
            ui.label("⏱ 等待配置:");
            ui.horizontal(|ui| {
                ui.label("等待时间:");
                let resp = ui.add(egui::DragValue::new(value).range(1..=60000).speed(10.0));
                if resp.drag_stopped() || resp.lost_focus() {
                    editor.config_changed = true;
                }
                ui.label("ms");
            });
            if ui.button("✓ 应用等待配置").clicked() {
                editor.config_changed = true;
                *status_message = "等待配置已更新".to_string();
                log_messages.push("[INFO] 更新等待配置".to_string());
            }
        }
    }
}

/// 编辑随机延迟步骤
fn edit_wait_random_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::WaitRandom { min, max } = &mut params[step_idx] {
            ui.label("🎲 随机延迟配置:");
            ui.horizontal(|ui| {
                ui.label("最小:");
                let resp = ui.add(egui::DragValue::new(min).range(1..=60000).speed(10.0));
                if resp.drag_stopped() || resp.lost_focus() {
                    if *min > *max { *max = *min; }
                    editor.config_changed = true;
                }
            });
            ui.horizontal(|ui| {
                ui.label("最大:");
                let resp = ui.add(egui::DragValue::new(max).range(1..=60000).speed(10.0));
                if resp.drag_stopped() || resp.lost_focus() {
                    if *max < *min { *min = *max; }
                    editor.config_changed = true;
                }
            });
            if ui.button("✓ 应用随机延迟配置").clicked() {
                editor.config_changed = true;
                *status_message = "随机延迟配置已更新".to_string();
                log_messages.push("[INFO] 更新随机延迟配置".to_string());
            }
        }
    }
}

/// 编辑文本步骤
fn edit_text_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::Text { value, delay: _ } = &mut params[step_idx] {
            ui.label("✍ 文本配置:");
            
            ui.label("输入文本:");
            ui.add(egui::TextEdit::multiline(value)
                .desired_rows(2)
                .desired_width(f32::INFINITY));
            
            if ui.button("✓ 应用文本配置").clicked() {
                editor.config_changed = true;
                *status_message = "文本配置已更新".to_string();
                log_messages.push("[INFO] 更新文本步骤".to_string());
            }
        }
    }
}

/// 编辑鼠标点击步骤
fn edit_mouse_click_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::MouseClick { button, delay } = &mut params[step_idx] {
            ui.label("🖱 鼠标点击配置:");
            
            ui.label("按钮:");
            let mut button_str = match button {
                MouseButtonType::Left => "left",
                MouseButtonType::Right => "right",
                MouseButtonType::Middle => "middle",
            }.to_string();
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut button_str, "left".to_string(), "左键");
                ui.radio_value(&mut button_str, "right".to_string(), "右键");
                ui.radio_value(&mut button_str, "middle".to_string(), "中键");
            });
            
            *button = match button_str.as_str() {
                "left" => MouseButtonType::Left,
                "right" => MouseButtonType::Right,
                "middle" => MouseButtonType::Middle,
                _ => MouseButtonType::Left,
            };
            
            ui.label("延迟 (ms):");
            edit_delay_config(ui, delay);
            
            if ui.button("✓ 应用鼠标点击配置").clicked() {
                editor.config_changed = true;
                *status_message = "鼠标点击配置已更新".to_string();
                log_messages.push("[INFO] 更新鼠标点击配置".to_string());
            }
        }
    }
}

/// 编辑鼠标动作步骤
fn edit_mouse_action_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::MouseAction { button, action, delay } = &mut params[step_idx] {
            ui.label("🖱 鼠标动作配置:");
            
            ui.label("按钮:");
            let mut button_str = match button {
                MouseButtonType::Left => "left",
                MouseButtonType::Right => "right",
                MouseButtonType::Middle => "middle",
            }.to_string();
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut button_str, "left".to_string(), "左键");
                ui.radio_value(&mut button_str, "right".to_string(), "右键");
                ui.radio_value(&mut button_str, "middle".to_string(), "中键");
            });
            
            *button = match button_str.as_str() {
                "left" => MouseButtonType::Left,
                "right" => MouseButtonType::Right,
                "middle" => MouseButtonType::Middle,
                _ => MouseButtonType::Left,
            };
            
            ui.label("动作:");
            let mut action_str = match action {
                MouseAction::Click => "click",
                MouseAction::Down => "down",
                MouseAction::Up => "up",
            }.to_string();
            
            ui.horizontal(|ui| {
                ui.radio_value(&mut action_str, "click".to_string(), "点击");
                ui.radio_value(&mut action_str, "down".to_string(), "按下");
                ui.radio_value(&mut action_str, "up".to_string(), "释放");
            });
            
            *action = match action_str.as_str() {
                "click" => MouseAction::Click,
                "down" => MouseAction::Down,
                "up" => MouseAction::Up,
                _ => MouseAction::Click,
            };
            
            ui.label("延迟 (ms):");
            edit_delay_config(ui, delay);
            
            if ui.button("✓ 应用鼠标动作配置").clicked() {
                editor.config_changed = true;
                *status_message = "鼠标动作配置已更新".to_string();
                log_messages.push("[INFO] 更新鼠标动作配置".to_string());
            }
        }
    }
}

/// 编辑鼠标移动步骤
fn edit_mouse_move_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::MouseMove { x, y, relative, delay } = &mut params[step_idx] {
            ui.label("🖱 鼠标移动配置:");
            
            ui.label("移动类型:");
            let mut is_relative = relative.unwrap_or(false);
            ui.horizontal(|ui| {
                ui.radio_value(&mut is_relative, false, "绝对位置");
                ui.radio_value(&mut is_relative, true, "相对移动");
            });
            *relative = Some(is_relative);
            
            ui.label(if is_relative { "X偏移:" } else { "X坐标:" });
            let mut x_str = x.to_string();
            ui.text_edit_singleline(&mut x_str);
            if let Ok(new_x) = x_str.parse::<i32>() {
                *x = new_x;
            }
            
            ui.label(if is_relative { "Y偏移:" } else { "Y坐标:" });
            let mut y_str = y.to_string();
            ui.text_edit_singleline(&mut y_str);
            if let Ok(new_y) = y_str.parse::<i32>() {
                *y = new_y;
            }
            
            ui.label("延迟 (ms):");
            edit_delay_config(ui, delay);
            
            if ui.button("✓ 应用鼠标移动配置").clicked() {
                editor.config_changed = true;
                *status_message = "鼠标移动配置已更新".to_string();
                log_messages.push("[INFO] 更新鼠标移动配置".to_string());
            }
        }
    }
}

/// 编辑鼠标滚轮步骤
fn edit_mouse_wheel_step(
    editor: &mut VisualEditor,
    ui: &mut egui::Ui,
    macro_idx: usize,
    step_idx: usize,
    status_message: &mut String,
    log_messages: &mut Vec<String>,
) {
    if let Some(params) = editor.config.hotkeys[macro_idx].params.steps_mut() {
        if let Step::MouseWheel { delta, delay } = &mut params[step_idx] {
            ui.label("🖱 鼠标滚轮配置:");
            
            ui.label("滚轮:");
            let mut delta_str = delta.to_string();
            ui.horizontal(|ui| {
                ui.label("数值 (正=向上, 负=向下):");
                ui.text_edit_singleline(&mut delta_str);
            });
            
            if let Ok(new_delta) = delta_str.parse::<i32>() {
                *delta = new_delta;
            }
            
            ui.label("延迟 (ms):");
            edit_delay_config(ui, delay);
            
            if ui.button("✓ 应用鼠标滚轮配置").clicked() {
                editor.config_changed = true;
                *status_message = "鼠标滚轮配置已更新".to_string();
                log_messages.push("[INFO] 更新鼠标滚轮配置".to_string());
            }
        }
    }
}

/// 编辑延迟配置（通用函数）
fn edit_delay_config(ui: &mut egui::Ui, delay: &mut Option<DelayConfig>) {
    let mut is_range = matches!(delay, Some(DelayConfig::Range { .. }));
    let mut fixed_val = match delay {
        Some(DelayConfig::Fixed(ms)) => *ms,
        _ => 50,
    };
    let mut min_val = match delay {
        Some(DelayConfig::Range { min, .. }) => *min,
        _ => 10,
    };
    let mut max_val = match delay {
        Some(DelayConfig::Range { max, .. }) => *max,
        _ => 100,
    };

    ui.horizontal(|ui| {
        ui.radio_value(&mut is_range, false, "固定延迟");
        ui.radio_value(&mut is_range, true, "随机范围");
    });

    if is_range {
        ui.horizontal(|ui| {
            ui.label("最小:");
            ui.add(egui::DragValue::new(&mut min_val).range(1..=10000));
            ui.label("最大:");
            ui.add(egui::DragValue::new(&mut max_val).range(1..=10000));
        });
        // 确保 min <= max
        if min_val > max_val {
            max_val = min_val;
        }
        *delay = Some(DelayConfig::Range { min: min_val, max: max_val });
    } else {
        ui.add(egui::DragValue::new(&mut fixed_val).range(1..=10000));
        *delay = Some(DelayConfig::Fixed(fixed_val));
    }
}
