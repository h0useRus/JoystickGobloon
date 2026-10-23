language-name = 简体中文

# Navigation and toolbar tooltips
nav-devices = 设备与绑定
nav-hidhide = 设备隐藏
nav-apps = 应用程序
nav-vjoy = 虚拟摇杆
nav-viewer = 输入查看器
nav-calibration = 校准
nav-settings = 设置
action-open = 导入 Joystick Gremlin 配置文件
action-save = 保存
label-mode = 模式

# Settings
settings-appearance = 外观
settings-general = 常规
settings-about = 关于

settings-lighting = 灯光
settings-leds-hint = 当更改过摇杆 LED 的配置文件被停用时，将设置这些颜色。摇杆会保持更改后的颜色直到重新连接，因此无法恢复其自身的 LED 设置。
settings-leds-none = 未连接可控制 LED 的摇杆。
settings-leds-add = 添加 LED
settings-language = 语言
settings-reopen-last = 启动时打开上次的配置文件
theme-light = 浅色
theme-dark = 深色
theme-system = 跟随系统
theme-system-tip = 跟随系统的浅色或深色设置
about-text = 设备隐藏、虚拟摇杆和输入映射，集于一个应用。

# Empty states
no-profile = 未加载配置文件
device-linked = 配置文件中同名设备的绑定。使用“映射控件”保存关联。
device-disconnected = 未连接
no-devices-attached = 未找到游戏控制器

# Status bar
status-ready = 就绪
status-loaded = 已加载 { $file }
status-saved = 已保存 { $file }
import-error = 无法打开配置文件：{ $error }
save-error = 无法保存配置文件：{ $error }
file-filter-gremlin = Joystick Gremlin 配置文件

# Import report
alert-error = 出现问题
alert-import = { $file } 中的部分动作将无法工作
alert-open-log = 打开日志
ok = 确定
hidhide-whitelist-error = 无法将 Joystick Gobloon 添加到 HidHide 白名单：{ $error }
warn-not-executable = “{ $kind }” × { $count }：保留在配置文件中，但暂不执行
warn-scripts = 配置文件包含 Python 插件：会被保留，但不会运行

# Map controls (device substitution)
subst-title = 映射控件
subst-auto = 自动
subst-auto-tip = 将其余设备映射到名称相似的已连接设备；不确定的留给您选择
subst-banner = 配置文件中的设备 ID 与此电脑不匹配：{ $count }
access-banner = Joystick Gobloon 需要读取游戏控制器和创建虚拟设备的权限。
access-grant = 授予权限
access-granted = 已授予设备访问权限。
access-error = 无法授予设备访问权限：{ $error }
subst-keep = 保持不变
subst-apply = 应用
status-substituted = 控件已映射。保存配置文件以保留更改。

# vJoy
vjoy-missing = 未安装 vJoy 或无法加载。
vjoy-no-access = 没有创建 vJoy 设备的权限。
vjoy-enabled = 驱动已启用
vjoy-disabled = 驱动已禁用
vjoy-no-devices = 尚未配置 vJoy 设备。
vjoy-axes = 轴
vjoy-buttons = 按钮
vjoy-hats = 苦力帽
vjoy-test-hint = 测试输出：锁定设备后才能使用下方控件。
vjoy-test-pending = 应用更改后才能测试。
vjoy-lock = 独占控制此设备以测试其输出
vjoy-unlock = 释放设备
vjoy-free = 空闲
vjoy-own = 由 Joystick Gobloon 控制
vjoy-busy = 被其他程序或另一个 Joystick Gobloon 实例使用
vjoy-unknown = 未知状态
vjoy-error = 无法占用设备：{ $error }

# Device hiding
hidhide-missing = 未安装 HidHide。
hidhide-on = 设备隐藏已开启
hidhide-off = 设备隐藏已关闭
hidhide-hidden = 已隐藏：仅白名单中的应用可见
hidhide-visible = 所有应用可见
hidhide-filter-disconnected = 隐藏未连接的设备
hidhide-gaming-only = 仅游戏控制器
hidhide-enable = 启用设备隐藏
hidhide-hint = 点击眼睛图标可对其他程序隐藏设备（闭眼）。之后重新连接设备使更改生效。
hidhide-no-devices = 没有可显示的设备。
action-refresh = 刷新
settings-see-hidden = 查看隐藏的设备
settings-see-hidden-tip = 启动时 Joystick Gobloon 会将自己加入仍可看到隐藏设备的程序，使配置文件继续工作。
settings-group-startup = 启动
settings-group-devices = 设备
settings-group-editor = 编辑器
settings-focus-input = 跟随正在使用的输入
settings-focus-input-tip = 使用轴、按钮或苦力帽时，“设备与绑定”页面会选中其设备标签页并滚动到该输入。
settings-input-events = 在变化时读取控制器
settings-input-events-tip = 更低延迟和 CPU 占用：控制器报告变化时才读取，短按按钮也不会丢失。关闭时每 4 毫秒读取一次所有控制器。
settings-monotonic-curves = 曲线只升或只降
settings-monotonic-curves-tip = 响应曲线编辑器：点或控制柄会停在曲线即将折返的位置，因此曲线只穿过零一次。已经折返的曲线可自由编辑。
settings-warn-gremlin = 在配置文件将无法在 Joystick Gremlin 中打开之前提醒
gremlin-format-title = Joystick Gremlin 将无法打开
gremlin-format-text = { $file } 使用了只有 Joystick Gobloon 才有的功能（修饰键和层）。保存后 Joystick Gremlin 将无法再打开它。
gremlin-format-dont-ask = 不再询问
status-whitelisted = 已将 Joystick Gobloon 添加到 HidHide 白名单。

# Applications
apps-add = 添加应用程序
apps-empty = 暂无应用程序。添加游戏或程序以进行设置。
apps-missing = 未找到文件
apps-missing-text = 该程序已不存在。要从列表中移除吗？
apps-sees = 可见隐藏设备
apps-blind = 不可见隐藏设备
apps-inverse = 反转应用列表
apps-inverse-tip = 列表中的程序看不到隐藏设备；其他所有程序都能看到
apps-remove = 从列表中移除

# Engine
action-activate = 激活配置文件
action-deactivate = 停止配置文件
title-active = 已激活
title-inactive = 未激活
status-active = 配置文件已激活
status-stopped = 配置文件已停止
activate-error = 无法激活配置文件：{ $error }

# Editing
edit-add = 添加动作
edit-remove = 移除
edit-up = 上移
edit-down = 下移
edit-not-editable = 此动作保留在配置文件中，但暂不能编辑。
edit-on-press = 按下时
edit-on-release = 松开时
edit-on-both = 按下和松开时
edit-inverted = 反向
edit-threshold = 长按阈值
edit-short = 短按
edit-long = 长按
edit-deadzone = 死区
edit-invert = 反向
edit-inherited = 继承自模式“{ $mode }”
edit-unbind = 移除此输入的所有动作
edit-override = 在此模式中覆盖
input-axis-kind = 轴
input-button-kind = 按钮
input-hat-kind = 苦力帽
kind-map-to-vjoy = 映射到 vJoy
kind-response-curve = 响应曲线
kind-change-mode = 切换模式
kind-tempo = 短按 / 长按
kind-description = 描述

# Files and modes
action-new = 新建配置文件
action-save-as = 另存为…
status-new = 新配置文件
unsaved-title = 未保存的更改
unsaved-text = 保存对 { $file } 的更改吗？
unsaved-save = 保存
unsaved-discard = 不保存
cancel = 取消
modes-edit = 编辑模式
modifiers-edit = 修饰键
modifiers-none = 还没有修饰键。修饰键是一个按钮，按住时会把其他输入切换到另一层动作。
modifiers-add = 添加修饰键
modifiers-press = 请按下控制器上的按钮…
modifiers-assign = 点击后按下此修饰键要使用的按钮
modifiers-remove = 删除修饰键及其各层的动作
modifier-input = 修饰键 { $name }：按住时其他输入使用它的层；它本身不执行动作
layer-base = 基础
layer-set = 此层有动作
layer-empty = 此层没有动作
layer-additive = 叠加到基础层
layer-additive-tip = 开：这些动作与基础层一起运行。关：按住修饰键时替代基础层。
layers-tip = 层：{ $layers }
modes-title = 模式
modes-name = 名称
modes-parent = 继承自
modes-remove = 移除模式及其绑定
modes-new = 新模式
modes-add = 添加模式

# Undo
action-undo = 撤销 (Ctrl+Z)
action-redo = 重做 (Ctrl+Y)
status-undone = 已撤销
status-redone = 已重做
edit-curve-help = 拖动点或控制柄以移动 · 点击空白处反转 · 双击添加点 · 右键点击点以移除

# vJoy mapping
vjoy-device-n = vJoy #{ $n }
vjoy-axis-1 = X 轴
vjoy-axis-2 = Y 轴
vjoy-axis-3 = Z 轴
vjoy-axis-4 = X 旋转
vjoy-axis-5 = Y 旋转
vjoy-axis-6 = Z 旋转
vjoy-axis-7 = 滑块
vjoy-axis-8 = 旋钮
vjoy-button-n = 按钮 { $n }
vjoy-hat-n = 苦力帽 { $n }
vjoy-absolute = 绝对
vjoy-relative = 相对
vjoy-speed = 速度

# Mouse
kind-map-to-mouse = 映射到鼠标
mouse-mode-button = 鼠标按键
mouse-mode-motion = 光标移动
mouse-left = 左键
mouse-right = 右键
mouse-middle = 中键
mouse-forward = 前进键
mouse-back = 后退键
mouse-wheel-up = 滚轮向上
mouse-wheel-down = 滚轮向下
mouse-direction = 方向
mouse-min-speed = 最低速度
mouse-max-speed = 最高速度
mouse-time-to-max = 达到最高速度的时间

# Toggle, split, pause, run
kind-smart-toggle = 智能切换
kind-split-axis = 拆分轴
kind-pause-resume = 暂停 / 继续
kind-run-command = 运行命令
edit-delay = 瞬时模式的按住时间
edit-split = 拆分点
edit-lower = 下半部分
edit-upper = 上半部分
pause-op-pause = 暂停
pause-op-resume = 继续
pause-op-toggle = 切换
run-executable = 程序
run-arguments = 参数
run-browse = 选择程序
status-paused = 配置文件已暂停

# Double tap, chain
kind-double-tap = 双击
kind-chain = 链
edit-tap-threshold = 到第二次点击的时间
edit-tap-exclusive = 独占：单击会等待以排除双击
edit-tap-combined = 组合：双击也会执行单击的动作
edit-single = 单击
edit-double = 双击
edit-chain-timeout = 空闲这么久后从第一步重新开始（0 = 从不）
edit-chain-step = 第 { $n } 步

# Hat as buttons
kind-hat-buttons = 苦力帽作为按钮
edit-hat-four = 4 方向
edit-hat-eight = 8 方向（含对角线）
hat-north = 上
hat-north-east = 右上
hat-east = 右
hat-south-east = 右下
hat-south = 下
hat-south-west = 左下
hat-west = 左
hat-north-west = 左上
hat-center = 中心

# Merge axis
kind-merge-axis = 合并轴
edit-reuse = 在此处也添加这个现有动作（共享，与 Gremlin 绑定到两个轴的方式相同）
edit-merge-name = 名称
edit-merge-no-device = （无设备）
edit-merged = 合并轴
merge-average = 平均值
merge-minimum = 最小值
merge-maximum = 最大值
merge-sum = 求和
merge-bidirectional = 双向（轴 2 减轴 1）
merge-maximum-deflection = 最大偏移
merge-closest-to-center = 最接近中心

# Axis delta
kind-axis-delta = 轴变化量
edit-delta-threshold = 触发脉冲的轴变化量
edit-delta-positive = 轴增大
edit-delta-negative = 轴减小

# Dual axis deadzone
kind-dual-axis-deadzone = 双轴死区
edit-dual-inner = 内死区（中心周围的圆）
edit-dual-outer = 外限（从此处起完全偏移）
edit-dual-close = 内死区与外死区过于接近。
edit-dual-first = 第一轴输出
edit-dual-second = 第二轴输出

# Existing actions (Gremlin's reference action)
edit-link-existing = 链接现有动作
edit-copy-existing = 复制现有动作

# Condition
kind-condition = 条件
edit-cond-all = 所有条件都须满足
edit-cond-any = 满足任一条件即可
edit-cond-add = 条件
edit-cond-true = 如果成立
edit-cond-false = 否则
cond-current = 此输入
cond-joystick = 摇杆输入
cond-vjoy = vJoy 输出
cond-keyboard = 按住的键
cond-range = 轴位于
cond-pressed = 已按下
cond-released = 已松开

# Sound, speech, load profile
kind-play-sound = 播放声音
kind-text-to-speech = 文本转语音
kind-load-profile = 加载配置文件
file-filter-sound = 声音文件
edit-volume = 音量
edit-file-missing = 未找到文件
edit-tts-hint = 要朗读的文本；$current_mode 会替换为当前模式
edit-tts-rate = 语速
edit-tts-pitch = 音调
edit-tts-back = 排在所有内容之后
edit-tts-front = 排在下一个（系统语音：与排在所有内容之后相同）
edit-tts-interrupt = 打断正在朗读的内容
edit-preview = 试听

kind-light-led = LED 灯光
edit-led-device = 带 LED 的摇杆
edit-led-no-device = 此摇杆未连接，或无法控制其 LED
edit-led-id = LED
led-vkb-grip = 握把顶部 RGB
led-vkb-hat = 苦力帽旁小红灯
edit-led-color = 颜色

edit-led-color2 = 第二颜色
edit-led-blend = 颜色
led-blend-color1 = 单色
led-blend-color2 = 仅第二颜色
led-blend-alternate = 交替颜色
led-blend-alternate-reverse = 交替，第二颜色优先
led-blend-mix = 混合颜色
led-vkb-base-blue = 底座，蓝色（右）
led-vkb-base-red = 底座，红色（左）
edit-led-mode = 发光方式
led-mode-on = 常亮
led-mode-slow-blink = 慢闪
led-mode-fast-blink = 快闪
led-mode-fastest-blink = 极快闪
led-mode-off = 关闭
edit-led-test = 立即点亮
edit-led-on-activate = 配置文件或模式激活时
edit-tts-voice = 语音
edit-tts-default-voice = 默认语音
edit-tts-voice-missing = 未安装
kind-map-to-keyboard = 映射到键盘

# Key recording
keys-record = 录制按键：同时按下，全部松开时结束录制
keys-press = 请按键…
keys-none = 无按键
keys-cancel = 取消录制
edit-chain-add = 添加步骤
edit-chain-remove = 移除此步骤及其动作
kind-macro = 宏

# Macro editor
macro-repeat = 重复
macro-repeat-single = 一次
macro-repeat-count = 次数
macro-repeat-hold = 按住期间
macro-repeat-toggle = 直到再次按下
macro-repeat-count-tip = 重复几次
macro-repeat-delay = 重复间隔
macro-add-step = 步骤
macro-step-key = 按键
macro-step-pause = 暂停
macro-step-mouse-button = 鼠标按键
macro-step-mouse-motion = 鼠标移动
macro-step-vjoy-button = vJoy 按钮
macro-step-vjoy-axis = vJoy 轴
macro-step-vjoy-hat = vJoy 苦力帽
macro-press = 按下
macro-release = 松开

# Logical device
logical-device = 逻辑设备
logical-device-tip = 此配置文件的虚拟输入：将输入映射到它们，再像摇杆一样绑定
logical-add-axis = 添加轴
logical-add-button = 添加按钮
logical-add-hat = 添加苦力帽
logical-remove = 移除此输入及其绑定
logical-empty = 暂无逻辑输入：用上方按钮添加一个。
kind-map-to-logical-device = 映射到逻辑设备
cond-logical = 逻辑设备输入

# Tray and startup
settings-tray = 显示托盘图标
settings-close-to-tray = 关闭时最小化到托盘
settings-minimize-to-tray = 最小化到托盘
settings-tray-double-click = 双击激活配置文件
settings-tray-title = 托盘图标
settings-autostart = 登录时启动（最小化）
settings-activate-on-start = 启动时激活配置文件
tray-show = 显示 Joystick Gobloon
tray-hiding = 设备隐藏
tray-activate = 激活“{ $name }”
tray-activate-none = 激活配置文件
tray-quit = 退出
autostart-error = 无法更改登录时启动：{ $error }

# About
about-alpha = 测试版
about-created = 作者：{ $name }
about-support = 支持开发
about-support-tip = 通过 PayPal 捐助
about-source = 源代码
about-issues = 报告问题
about-releases = 版本发布
about-hiding = 设备隐藏：{ $backend }
about-virtual = 虚拟摇杆：{ $backend }
about-not-installed = 未安装
about-gremlin = 配置文件与 Joystick Gremlin 兼容。
about-license = 基于 MIT 许可证的自由开源软件。

# Updates
settings-check-updates = 启动时检查新版本并安装
update-check = 检查更新
update-checking = 正在查找新版本…
update-latest = 已是最新版本
update-available = 有新版本 { $version }
update-install = 立即更新
update-downloading = 正在下载版本 { $version }…
update-error = 无法检查更新
update-debug = 调试版本不会自动更新
update-install-error = 无法安装更新：{ $error }

# Input viewer
viewer-history = 轴随时间变化
viewer-current = 当前轴
viewer-buttons = 按钮和苦力帽
viewer-axis = 轴 { $n }

# vJoy configuration
vjoy-add = 添加 vJoy 设备
vjoy-add-max = vJoy 最多支持 16 个设备。
vjoy-new-device = 新 vJoy #{ $n }：尚未创建
vjoy-enable = 启用 vJoy 驱动
vjoy-disable = 禁用 vJoy 驱动
vjoy-hat-continuous = 连续
vjoy-hat-discrete = 4 方向
vjoy-ffb = 力反馈
vjoy-ffb-enable = 已启用
vjoy-ffb-const = 恒定
vjoy-ffb-ramp = 斜坡
vjoy-ffb-sq = 方波
vjoy-ffb-sine = 正弦
vjoy-ffb-tr = 三角波
vjoy-ffb-stup = 上升锯齿
vjoy-ffb-stdn = 下降锯齿
vjoy-ffb-spr = 弹簧
vjoy-ffb-dm = 阻尼
vjoy-ffb-inr = 惯性
vjoy-ffb-fric = 摩擦
vjoy-config-create = 创建
vjoy-config-apply = 应用
vjoy-config-revert = 还原
vjoy-config-remove = 移除设备
vjoy-config-admin = 更改 vJoy 时 Windows 会请求管理员权限
vjoy-config-locked = 停止配置文件后才能更改 vJoy 设备。
vjoy-config-reading = 正在读取 vJoy 配置...
vjoy-config-label = 配置 vJoy #{ $n }
vjoy-config-done = vJoy 配置已更改。
vjoy-config-error = 无法更改 vJoy 配置：{ $error }
vjoy-config-reboot = vJoy 配置已更改。重启 Windows 后生效。
vjoy-remove-title = 移除 vJoy #{ $n }？
vjoy-remove-text = 使用此设备的游戏和配置文件将失去它，直到重新创建为止。

# Response curve
curve-type = 曲线类型
curve-linear = 分段线性
curve-cubic = 三次样条
curve-bezier = 三次贝塞尔样条
curve-symmetric = 对称
curve-symmetric-tip = 编辑时将曲线关于中心镜像
curve-point = 点
curve-handle = 控制柄

# Keyboard device
keyboard-device = 键盘
keyboard-tip = 将键盘按键作为输入：像绑定摇杆按钮一样为按键绑定动作
keyboard-add = 绑定按键
keyboard-empty = 尚未绑定按键：用上方按钮录制一个。
edit-binding-description = 此输入的描述
edit-unbound = 暂无动作：添加一个。
viewer-detach = 在单独窗口中打开
viewer-attach = 返回主窗口
viewer-in-window = 输入查看器已在单独窗口中打开。

# Calibration
calibration-device = 要校准的设备
calibration-raw = 原始值
calibration-with-center = 含中心
calibration-reset = 重置
calibration-save = 保存此校准
calibration-center = 校准中心
calibration-center-tip = 让摇杆静止，在中心附近轻微晃动，然后再次点击
calibration-extrema = 校准极值
calibration-extrema-tip = 将轴移到两端，然后再次点击
calibration-in-window = 校准已在单独窗口中打开。

# Device info
device-info = 设备信息

device-leds = 可控制 LED（LED 灯光动作）
device-info-name = 设备名称
device-info-axes = 轴
device-info-buttons = 按钮
device-info-hats = 苦力帽
device-info-vid = VID
device-info-pid = PID
device-info-joystick-id = 摇杆 ID
device-info-guid = 设备 GUID
device-info-copy = 复制到剪贴板
