# 青龙 env 编辑器（C++ Win32 原生）

> 2026-10-29 从 `E:\workbuddy\fiddler\cmcc\` 整体迁入本目录。

## 文件
- `qinglong_env_editor.cpp` + `resource.rc` + `app.manifest` + `app.ico` → `qinglong_env_editor.exe`（~1.13MB 静态链接）
- 运行时在 exe 同目录读写 `settings.txt`

## 编译（`C:\mingw64`）
```
windres -O coff -o res.o resource.rc
g++ -Os -s -mwindows -D_GLIBCXX_USE_CXX11_ABI=0 -static-libgcc -static-libstdc++ -o qinglong_env_editor.exe qinglong_env_editor.cpp res.o -lwinhttp
```
不加 ABI 宏缺 move ctor。

## 功能
- 设置区（青龙地址/client_id/client_secret）存同目录 `settings.txt` →「读取变量名」拉 `/open/envs` → 下拉选择或手填名称 → 填值 →「确认更新到青龙」PUT `/open/envs`
- 对 `CMCC_ACCOUNT` 识别到 `sessionCookie` 时只替换该字段，保留 JSON 其余键
- 「JSON压缩(Minify)」按钮（2026-10-29 加，IDC_MINIFY）：对值框内容去字符串外空白压成单行；算法参考 simdjson json_minifier / rapidjson condense（引号状态机+转义跟踪），不校验合法性

## 坑
- manifest 必须资源 ID 1/类型 24
- `WNDCLASSW` 无 `hIconSm`（用 `WM_SETICON`）
- 字体 `SPI_GETNONCLIENTMETRICS.lfMessageFont` + `S(v)=MulDiv(v,dpi,96)` DPI 缩放
- 编译时若 exe 被占用报 Permission denied，确认进程退出后重试
