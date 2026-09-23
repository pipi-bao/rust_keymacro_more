//! GUI 应用模块
//!
//! 使用 egui + eframe 构建的主窗口界面

pub mod log_viewer;
pub mod settings;

use eframe::egui;
use crate::config::{Config, GlobalHotkeyConfig};
use crate::macros::{init_keyboard_macro_system, set_macro_enabled, cleanup_keyboard_hook};
use crate::visual_editor::VisualEditor;
use windows::Win32::UI::WindowsAndMessaging::HHOOK;

/// 主窗口应用
pub struct MacroApp {
    /// 宏配置
    config: Config,
    
    /// 可视化编辑器
    visual_editor: VisualEditor,
    
    /// 宏是否启用
    macro_enabled: bool,
    
    /// 键盘钩子句柄
    keyboard_hook: Option<HHOOK>,
    
    /// 日志消息
    log_messages: Vec<String>,
    
    /// 当前选中的标签页
    selected_tab: usize,
    
    /// 状态消息
    status_message: String,
    
    /// 是否有未保存的更改
    has_unsaved_changes: bool,
    
    /// 是否显示关闭确认对话框
    show_close_dialog: bool,
    
    /// 全局快捷键捕获状态（设置页）
    hotkey_capture: settings::HotkeyCapture,
    
    /// 当前全局快捷键配置
    global_hotkey_cfg: GlobalHotkeyConfig,
}

impl MacroApp {
    /// 创建新的应用实例
    pub fn new(cc: &eframe::CreationContext<'_>, config: Config) -> Self {
        // 设置支持中文的字体
        Self::setup_chinese_font(&cc.egui_ctx);
        
        // 设置浅色主题
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        
        // 创建可视化编辑器
        let visual_editor = VisualEditor::new(config.clone());
        
        // 初始化键盘宏系统
        let keyboard_hook = init_keyboard_macro_system(config.clone());

        // 全局快捷键配置：优先取配置文件中保存的，否则用默认
        let global_hotkey_cfg = config.global_hotkey.clone()
            .unwrap_or_else(GlobalHotkeyConfig::default);

        // 启动全局快捷键监听（程序后台也可用）
        let hotkey_desc = crate::global_hotkey::start_global_hotkeys(config.global_hotkey.as_ref());
        
        let mut log_messages = vec![
            "[INFO] 应用程序启动".to_string(),
            "[INFO] 键盘宏系统已初始化（默认禁用）".to_string(),
        ];
        match hotkey_desc {
            Ok(desc) => {
                log_messages.push(format!("[INFO] 全局快捷键 {} 已注册（切换宏总开关）", desc));
            }
            Err(e) => {
                log_messages.push(format!("[WARN] {}", e));
            }
        }
        
        Self {
            config,
            visual_editor,
            macro_enabled: false,
            keyboard_hook,
            log_messages,
            selected_tab: 0,
            status_message: "就绪".to_string(),
            has_unsaved_changes: false,
            show_close_dialog: false,
            hotkey_capture: settings::HotkeyCapture::default(),
            global_hotkey_cfg,
        }
    }
    
    /// 设置中文字体支持
    fn setup_chinese_font(ctx: &egui::Context) {
        use std::path::Path;
        
        let mut fonts = egui::FontDefinitions::default();
        
        // Windows 系统字体路径列表（按优先级）
        let font_paths = [
            "C:/Windows/Fonts/msyh.ttc",      // 微软雅黑
            "C:/Windows/Fonts/msyhbd.ttc",    // 微软雅黑粗体
            "C:/Windows/Fonts/simsun.ttc",    // 宋体
            "C:/Windows/Fonts/simhei.ttf",    // 黑体
            "C:/Windows/Fonts/mingliu.ttc",   // 细明体
        ];
        
        // 尝试加载第一个可用的中文字体
        for font_path in font_paths.iter() {
            if Path::new(font_path).exists() {
                if let Ok(font_bytes) = std::fs::read(font_path) {
                    fonts.font_data.insert(
                        "chinese_font".to_owned(),
                        egui::FontData::from_owned(font_bytes),
                    );
                    break;
                }
            }
        }
        
        // 设置字体优先级，中文字体排在前面
        fonts
            .families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .insert(0, "chinese_font".to_owned());
        
        fonts
            .families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .insert(0, "chinese_font".to_owned());
        
        ctx.set_fonts(fonts);
    }
    
    /// 保存配置到文件
    fn save_config(&mut self) {
        // 使用可视化编辑器的配置
        self.config = self.visual_editor.config.clone();
        // 写入当前全局快捷键配置
        self.config.global_hotkey = Some(self.global_hotkey_cfg.clone());
        
        let config_text = serde_yaml::to_string(&self.config)
            .unwrap_or_else(|_| "# 配置序列化失败".to_string());
        
        let config_path = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("config.yaml")))
            .unwrap_or_else(|| "config.yaml".into());
        
        match std::fs::write(&config_path, &config_text) {
            Ok(_) => {
                // 清理旧的钩子并重新初始化
                if let Some(hook) = self.keyboard_hook.take() {
                    cleanup_keyboard_hook(hook);
                }
                self.keyboard_hook = init_keyboard_macro_system(self.config.clone());
                
                self.has_unsaved_changes = false;
                self.status_message = format!("✓ 配置已保存到: {}", config_path.display());
                self.log_messages.push(format!("[INFO] 配置已保存到: {}", config_path.display()));
            }
            Err(e) => {
                self.status_message = format!("✗ 保存失败: {}", e);
                self.log_messages.push(format!("[ERROR] 保存配置失败: {}", e));
            }
        }
    }
    
    /// 切换宏启用状态
    fn toggle_macro(&mut self) {
        self.macro_enabled = !self.macro_enabled;
        set_macro_enabled(self.macro_enabled);
        
        let state = if self.macro_enabled { "已启用" } else { "已禁用" };
        self.status_message = format!("宏 {}", state);
        self.log_messages.push(format!("[INFO] 宏 {}", state));
    }
    
    /// 显示关闭确认对话框
    fn show_close_dialog(&mut self, ctx: &egui::Context) {
        if !self.show_close_dialog {
            return;
        }
        
        let mut should_save_and_close = false;
        let mut should_close_without_save = false;
        let mut cancelled = false;
        
        egui::Window::new("未保存的更改")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .open(&mut self.show_close_dialog)
            .show(ctx, |ui| {
                ui.label("当前有未保存的更改，是否在退出前保存？");
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if ui.button("保存并退出").clicked() {
                        should_save_and_close = true;
                    }
                    if ui.button("不保存").clicked() {
                        should_close_without_save = true;
                    }
                    if ui.button("取消").clicked() {
                        cancelled = true;
                    }
                });
            });
        
        if cancelled {
            // 取消关闭对话框，重置状态
            self.show_close_dialog = false;
            return;
        }
        
        if should_save_and_close {
            self.show_close_dialog = false;
            self.save_config();
            if let Some(hook) = self.keyboard_hook.take() {
                cleanup_keyboard_hook(hook);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else if should_close_without_save {
            self.has_unsaved_changes = false;
            self.show_close_dialog = false;
            if let Some(hook) = self.keyboard_hook.take() {
                cleanup_keyboard_hook(hook);
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl eframe::App for MacroApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 检查配置是否已更改，如果是则重新初始化系统
        if self.visual_editor.config_changed {
            // 同步配置
            self.config = self.visual_editor.config.clone();
            
            // 清理旧的钩子并重新初始化
            if let Some(hook) = self.keyboard_hook.take() {
                cleanup_keyboard_hook(hook);
            }
            self.keyboard_hook = init_keyboard_macro_system(self.config.clone());
            
            // 重置标志位
            self.visual_editor.config_changed = false;
            self.has_unsaved_changes = true;
            
            self.status_message = "配置已应用并重新初始化".to_string();
            self.log_messages.push("[INFO] 配置已应用，系统已重新初始化".to_string());
        }
        
        // 同步全局总开关状态（可能被全局快捷键在后台修改）
        self.macro_enabled = crate::macros::get_toggle_state();

        // Ctrl+S 保存（设置页正在捕获快捷键时不抢按键）
        if !self.hotkey_capture.is_active() {
            let save_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::S);
            if ctx.input_mut(|i| i.consume_shortcut(&save_shortcut)) {
                self.save_config();
            }
        }
        
        // 检查系统关闭请求（点击窗口右上角 X 按钮）
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.has_unsaved_changes {
                // 取消自动关闭，弹出确认对话框
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                self.show_close_dialog = true;
            }
            // 如果没有未保存更改，让窗口自动关闭
        }
        
        // 显示关闭确认对话框
        self.show_close_dialog(ctx);
        
        // 顶部面板
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            show_menu_bar(self, ui);
        });
        
        // 底部状态栏
        egui::TopBottomPanel::bottom("status_bar").show(ctx, |ui| {
            ui.separator();
            ui.horizontal(|ui| {
                ui.label(&self.status_message);
            });
        });
        
        // 中央内容区域
        egui::CentralPanel::default().show(ctx, |ui| {
            // 标签页选择
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.selected_tab, 0, "🎯 可视化编辑器");
                ui.selectable_value(&mut self.selected_tab, 1, "📊 运行日志");
                ui.selectable_value(&mut self.selected_tab, 2, "⚙️ 设置");
            });
            
            ui.separator();
            
            // 根据选中的标签页显示不同内容
            match self.selected_tab {
                0 => self.visual_editor.show(ui, &mut self.status_message, &mut self.log_messages),
                1 => log_viewer::show(self, ui),
                2 => {
                    if settings::show(
                        ui,
                        ctx,
                        &mut self.hotkey_capture,
                        &mut self.global_hotkey_cfg,
                        &mut self.status_message,
                    ) {
                        self.save_config();
                    }
                }
                _ => {}
            }
        });
    }
}

/// 显示菜单栏
fn show_menu_bar(app: &mut MacroApp, ui: &mut egui::Ui) {
    egui::menu::bar(ui, |ui| {
        ui.menu_button("文件", |ui| {
            if ui.button("保存配置    Ctrl+S").clicked() {
                app.save_config();
                ui.close_menu();
            }
            if ui.button("重新加载配置").clicked() {
                reload_config(app);
                ui.close_menu();
            }
            ui.separator();
            if ui.button("退出").clicked() {
                if app.has_unsaved_changes {
                    app.show_close_dialog = true;
                } else {
                    if let Some(hook) = app.keyboard_hook.take() {
                        cleanup_keyboard_hook(hook);
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                ui.close_menu();
            }
        });

        let save_label = if app.has_unsaved_changes {
            "💾 保存 *"
        } else {
            "💾 保存"
        };
        let save_btn = egui::Button::new(save_label);
        if ui.add(save_btn).on_hover_text("保存配置到 config.yaml（Ctrl+S）").clicked() {
            app.save_config();
        }
        if app.has_unsaved_changes {
            ui.colored_label(egui::Color32::from_rgb(180, 100, 0), "未保存");
        }
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let status_color = if app.macro_enabled {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            };
            
            ui.colored_label(status_color, if app.macro_enabled { "● 已启用" } else { "● 已禁用" });
            
            if ui.button(if app.macro_enabled { "禁用宏" } else { "启用宏" }).clicked() {
                app.toggle_macro();
            }
        });
    });
}

/// 重新加载配置
fn reload_config(app: &mut MacroApp) {
    let config_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("config.yaml")))
        .unwrap_or_else(|| "config.yaml".into());
    
    match Config::from_file(&config_path) {
        Ok(config) => {
            app.config = config.clone();
            app.visual_editor = VisualEditor::new(config.clone());
            app.global_hotkey_cfg = config.global_hotkey.clone()
                .unwrap_or_else(GlobalHotkeyConfig::default);
            
            // 清理旧的钩子并重新初始化
            if let Some(hook) = app.keyboard_hook.take() {
                cleanup_keyboard_hook(hook);
            }
            app.keyboard_hook = init_keyboard_macro_system(config.clone());

            // 重新注册全局快捷键（使用配置中的值，set_hotkey 会先注销旧热键）
            let target = match config.global_hotkey.as_ref() {
                Some(cfg) => crate::global_hotkey::hotkey_from_config(cfg),
                None => crate::global_hotkey::default_hotkey(),
            };
            if let Err(e) = crate::global_hotkey::set_hotkey(target) {
                app.status_message = format!("✓ 配置已重新加载（但全局快捷键：{}）", e);
            } else {
                app.status_message = "✓ 配置已重新加载".to_string();
            }
            app.has_unsaved_changes = false;
            app.log_messages.push("[INFO] 配置已重新加载".to_string());
        }
        Err(e) => {
            app.status_message = format!("✗ 加载失败: {}", e);
            app.log_messages.push(format!("[ERROR] 加载配置失败: {}", e));
        }
    }
}
