//! 可视化配置编辑器模块
//!
//! 提供表单式的GUI界面来编辑宏配置，替代YAML文本编辑

pub mod macro_list;
pub mod macro_editor;
pub mod step_editor;
pub mod key_selector;

use crate::config::*;
use crate::config::Step;

/// 可视化编辑器状态
pub struct VisualEditor {
    /// 当前配置
    pub config: Config,
    /// 当前选中的宏索引
    selected_macro: Option<usize>,
    /// 编辑状态：触发源类型 (true=键盘, false=手柄)
    edit_is_keyboard: bool,
    /// 编辑状态：触发键（键盘模式）
    edit_keyboard_key: String,
    /// 编辑状态：触发键（手柄模式）
    edit_gamepad_key: String,
    /// 配置是否已更改（需要重新初始化系统）
    pub config_changed: bool,
    /// 是否显示按键选择窗口
    pub show_key_selector_window: bool,
    /// 按键选择器是否用于步骤编辑
    pub key_selector_for_step: bool,
    /// 正在编辑步骤键的宏索引
    pub key_selector_macro_idx: usize,
    /// 正在编辑步骤键的步骤索引
    pub key_selector_step_idx: usize,
    /// 正在编辑的步骤键值
    pub step_editing_key: String,
    /// 步骤按键选择器是否显示手柄键位
    pub key_selector_for_gamepad: bool,
}

impl VisualEditor {
    /// 创建新的编辑器实例
    pub fn new(config: Config) -> Self {
        Self {
            config,
            selected_macro: None,
            edit_is_keyboard: true,
            edit_keyboard_key: "F1".to_string(),
            edit_gamepad_key: "A".to_string(),
            config_changed: false,
            show_key_selector_window: false,
            key_selector_for_step: false,
            key_selector_macro_idx: 0,
            key_selector_step_idx: 0,
            step_editing_key: String::new(),
            key_selector_for_gamepad: false,
        }
    }
    
    /// 当选择不同的宏时，从配置加载编辑状态
    fn load_edit_state(&mut self, idx: usize) {
        if idx < self.config.hotkeys.len() {
            let hotkey = &self.config.hotkeys[idx];
            self.edit_is_keyboard = matches!(&hotkey.trigger, TriggerSource::Keyboard { .. });
            match &hotkey.trigger {
                TriggerSource::Keyboard { key } => self.edit_keyboard_key = key.clone(),
                TriggerSource::Gamepad { key } => self.edit_gamepad_key = key.clone(),
            }
        }
    }
    
    /// 显示编辑器UI
    pub fn show(&mut self, ui: &mut egui::Ui, status_message: &mut String, log_messages: &mut Vec<String>) {
        // 先显示键盘/手柄选择窗口（在绘制主UI之前，确保窗口在最上层）
        let ctx = ui.ctx().clone();
        self.show_key_selector_popup(&ctx);

        ui.vertical(|ui| {
            ui.heading("🎯 宏配置可视化编辑器");
            ui.add_space(10.0);

            // 使用columns创建左右分栏
            ui.columns(2, |columns| {
                // 左侧：宏列表
                columns[0].vertical(|ui| {
                    self.show_macro_list(ui, status_message, log_messages);
                });

                // 右侧：宏详情编辑
                columns[1].vertical(|ui| {
                    self.show_macro_detail(ui, status_message, log_messages);
                });
            });
        });
    }

    /// 显示按键选择弹窗
    fn show_key_selector_popup(&mut self, ctx: &egui::Context) {
        use egui::*;

        if !self.show_key_selector_window {
            return;
        }

        let mut open = true;
        let is_keyboard = self.edit_is_keyboard;
        let is_for_step = self.key_selector_for_step;
        let step_uses_gamepad = is_for_step && self.key_selector_for_gamepad;
        let show_gamepad = if is_for_step { step_uses_gamepad } else { !is_keyboard };

        let title = if is_for_step && step_uses_gamepad {
            "选择步骤按键（手柄）"
        } else if is_for_step {
            "选择步骤按键（键盘）"
        } else if is_keyboard {
            "选择键盘按键"
        } else {
            "选择手柄按键"
        };

        Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut open)
            .show(ctx, |ui| {
                let mut key = if is_for_step {
                    self.step_editing_key.clone()
                } else if is_keyboard {
                    self.edit_keyboard_key.clone()
                } else {
                    self.edit_gamepad_key.clone()
                };
                let selected = if show_gamepad {
                    key_selector::show_gamepad_content(ui, &mut key)
                } else {
                    key_selector::show_keyboard_content(ui, &mut key)
                };
                if selected {
                    if is_for_step {
                        // 写回到步骤键值
                        self.step_editing_key = key;
                        if self.key_selector_step_idx == usize::MAX {
                            // 连发目标键
                            if let Some(hotkey) = self.config.hotkeys.get_mut(self.key_selector_macro_idx) {
                                if let ActionParams::AutoRepeat(params) = &mut hotkey.params {
                                    params.key = self.step_editing_key.clone();
                                    self.config_changed = true;
                                }
                            }
                        } else if let Some(steps) = self.config.hotkeys.get_mut(self.key_selector_macro_idx)
                            .and_then(|h| h.params.steps_mut())
                        {
                            if let Some(Step::Key { value, device, .. }) = steps.get_mut(self.key_selector_step_idx) {
                                *value = self.step_editing_key.clone();
                                if step_uses_gamepad {
                                    *device = Some("gamepad".to_string());
                                }
                                self.config_changed = true;
                            }
                        }
                    } else if is_keyboard {
                        self.edit_keyboard_key = key;
                    } else {
                        self.edit_gamepad_key = key;
                    }
                    self.show_key_selector_window = false;
                    self.key_selector_for_step = false;
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(4.0);

                if ui.button("关闭").clicked() {
                    self.show_key_selector_window = false;
                    self.key_selector_for_step = false;
                }
            });

        if !open {
            self.show_key_selector_window = false;
            self.key_selector_for_step = false;
        }
    }
    
    /// 显示宏列表（左侧面板）
    fn show_macro_list(&mut self, ui: &mut egui::Ui, status_message: &mut String, log_messages: &mut Vec<String>) {
        use macro_list::show_macro_list;
        show_macro_list(self, ui, status_message, log_messages);
    }
    
    /// 显示宏详情编辑（右侧面板）
    fn show_macro_detail(&mut self, ui: &mut egui::Ui, status_message: &mut String, log_messages: &mut Vec<String>) {
        use macro_editor::show_macro_detail;
        show_macro_detail(self, ui, status_message, log_messages);
    }
}
