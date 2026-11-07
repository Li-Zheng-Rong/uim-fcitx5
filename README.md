# uim-fcitx5

通过 Rust 插件，让 `uim-fep` 连接 Fcitx5。UIM 负责终端交互和候选栏，Fcitx5 负责输入转换、词库和学习记录。

使用同一用户的 Fcitx5 进程，无需桌面环境。

## 构建与安装

依赖 Rust（支持 edition 2024）、C 编译工具、`pkg-config`、`libdbus` 开发文件和 Fcitx5。运行时需要 UTF-8 终端、用户 D-Bus，以及包含本项目补丁的 `uim-fep`。

Arch Linux 上先构建并安装终端依赖：

```sh
cd packaging
makepkg -si
cd ..
```

该包基于 UIM 1.9.8，增加 F13 识别和 D-Bus 事件监听所需的文件描述符回调，与完整 `uim` 包冲突。

然后构建并安装桥接：

```sh
make
sudo make install
```

安装会将插件和 Scheme 文件放入 UIM 目录、注册 `fcitx5` 引擎，并将启动器安装到 `/usr/local/bin`。可通过 `PREFIX` 更改启动器安装前缀；使用 `DESTDIR` 暂存安装时需另行注册引擎。

## 使用

```sh
uim-fcitx5
uim-fcitx5 zsh -l
uim-fcitx5 vim notes.txt
```

不传参数时启动 `$SHELL -l`，传入命令时运行该命令。

Fcitx5 未运行时，通过系统提供的 D-Bus 服务自动启动。启动器使用 `DBUS_SESSION_BUS_ADDRESS`；未设置时连接 `${XDG_RUNTIME_DIR:-/run/user/$(id -u)}/bus`。

按 F13 切换中英文。若使用 Win+Space，需要在终端的键盘配置中将它映射为 F13。输入状态共享遵循 Fcitx5 的 `ShareInputState` 设置。

如需在登录时自动启动，应限定目标终端，并检查 `UIM_FEP_PID`，避免重复嵌套启动。

## 限制

按键受终端协议限制，不支持读取或删除应用中光标周围的文字。候选栏显示 Fcitx5 提供的当前页，翻页由输入法处理。
