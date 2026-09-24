# 开发日志

## 来源

本仓库基于 [rchangl/rust_keymacro](https://github.com/rchangl/rust_keymacro.git)（键盘/手柄触发的配置文件驱动宏，GUI 版 v2.0.0）。

上游已具备：YAML 配置、键盘热键、Xbox 手柄**触发**、`type_text` / `sequence` / `auto_repeat`、鼠标模拟、可视化编辑器。手柄在上游中只作为触发源，输出仍是键盘/鼠标（`SendInput`），无法向游戏写入手柄按键。

本分支在此之上引入 **ViGEmBus 虚拟手柄**，补上手柄按键输出和按住循环。

## 2026-09-22 — 引入 ViGEmBus，实现手柄输出与按住循环

### 动机

需要用实体 Xbox 手柄做宏：按住 `X` 循环 `LB`/`RB`，并定时按 `Y`。Windows XInput 不能把按键写回同一只实体手柄，游戏读到的仍是真实状态，因此必须另挂一只虚拟手柄。

### 做法

- 依赖 [ViGEmBus](https://github.com/nefarius/ViGEmBus/releases) 驱动，以及 Rust 客户端 `vigem-client`。
- 配置里出现手柄输出（`device: gamepad`，或 `LB`/`RB` 等仅手柄键名）时，创建虚拟 **Xbox 360 Controller**。
- 轮询实体手柄：检测开宏键；把摇杆、扳机、其它按键抄到虚拟手柄（透传）。
- 在虚拟手柄上叠加宏按键，并屏蔽开宏触发键（避免游戏也收到该键）。

这不是拦截实体手柄。实体和虚拟同时存在；游戏必须读虚拟那只，宏按键才会生效。Windows「首选设备」只决定游戏读谁，不会自动绑定实体到虚拟，透传由本程序完成。

### 新增能力

1. **`hold_loop`（按住循环）**  
   按住触发键循环执行 `steps`，松开即停。`every` 为定时附加按键。  
   时序：按下后先立刻执行全部 `every`，再开始 `steps`；之后每隔 `interval_ms` 再发一次。

2. **步骤 `device` 字段**  
   `keyboard` / `gamepad`。`A`/`B`/`X`/`Y` 默认仍是键盘，模拟手柄时需写 `device: gamepad`。

3. **可视化编辑器**  
   增加「按住循环」、步骤输出设备、定时附加按键编辑。

### 当前示例配置

按住手柄 `X`：立刻先发 `Y`，再循环 `LB`/`RB`，之后每 5 秒再发一次 `Y`。松开 `X` 停止。见 `config.yaml`。

### 使用注意

- 必须安装 ViGEmBus，并保持本程序运行、宏总开关开启。
- 多数游戏（含《暗黑破坏神 4》）不能在设置里指定手柄，通常只认 0 号 XInput 设备。可在 `joy.cpl` → 高级里把虚拟手柄设为首选；仍认实体时，用 [HidHide](https://github.com/nefarius/HidHide) 对游戏隐藏实体手柄。
- 未安装驱动时循环仍会跑，但游戏收不到模拟按键。

### 主要改动文件

| 位置 | 内容 |
|------|------|
| `Cargo.toml` | 增加 `vigem-client` |
| `src/gamepad/mod.rs` | 虚拟手柄创建、透传、宏按键叠加、触发键屏蔽 |
| `src/config.rs` | `hold_loop`、`IntervalAction`、`device`、按 action 解析 params |
| `src/macros/handler.rs` | 按住启动/松开停止；`every` 先发再循环 |
| `src/macros/executor.rs` | 按 `device` 走键盘或虚拟手柄 |
| `src/visual_editor/` | 按住循环与手柄步骤编辑 |
| `config.yaml` | 手柄 X 按住循环示例 |
| `README.md` | ViGEmBus、透传说明、故障排查 |

---

后续改动按日期追加在本文件下方即可。
