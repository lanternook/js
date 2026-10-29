# qinglong_env_editor

青龙面板环境变量更新工具（Win32 原生 C++，单文件 exe，无运行时依赖）。

通过青龙 OpenAPI 读取/更新环境变量，专为「粘贴新 cookie 后只替换 JSON 中 `sessionCookie` 字段」的场景设计，避免整段覆盖丢失其他键值。

## 功能

- **设置区**：青龙地址 / Client ID / Client Secret，保存到 exe 同目录 `settings.txt`
- **读取变量名**：`GET /open/envs` 拉取全部环境变量名，下拉框选择或手动输入
- **确认更新到青龙**：按名称查找变量 id 后 `PUT /open/envs`；若原值是含 `"sessionCookie":"..."` 的 JSON，则只替换该字段内容，其余键原样保留
- **JSON 压缩（Minify）**：把值框中的多行 JSON 压成单行（去除字符串外空白，字符串内部空白原样保留）。算法参考 simdjson `json_minifier` 与 rapidjson `condense` 示例：引号状态机 + 反斜杠转义跟踪。注意该功能只删空白、不校验 JSON 合法性

## 编译

依赖：MinGW-w64（windres + g++）、WinHTTP。

```bat
windres -O coff -o res.o resource.rc
g++ -Os -s -mwindows -D_GLIBCXX_USE_CXX11_ABI=0 -static-libgcc -static-libstdc++ -o qinglong_env_editor.exe qinglong_env_editor.cpp res.o -lwinhttp
```

## 目录结构

```
qinglong_env_editor.cpp   主源码（Win32 API，DPI 感知）
resource.rc               资源脚本（图标 + manifest）
app.manifest              DPI 感知清单（资源 ID 1 / 类型 24）
app.ico                   应用图标（未入库，自行放置同名文件即可）
NOTES.md                  开发备忘（踩坑记录）
```

## 使用流程

1. 填写青龙地址（如 `http://192.168.50.43:5700`）与 OpenAPI 凭证 → 保存设置
2. 点「读取变量名」→ 下拉选择目标变量
3. 粘贴新 cookie（可先点「JSON压缩(Minify)」压成单行）
4. 点「确认更新到青龙」，状态栏显示接口返回

## 备注

- OpenAPI 仅覆盖 auth/envs 等端点，脚本文件不在其范围内
- 客户端 SSL 校验已放宽（`SECURITY_FLAG_IGNORE_*`），仅建议内网使用
