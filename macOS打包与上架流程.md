# macOS 打包与上架流程

本文档整理本项目的两条 macOS 发布流程：

- DMG 分发包：用于官网下载、网盘分发等 App Store 之外的安装方式。
- Mac App Store 包：用于上传 App Store Connect / TestFlight / Mac App Store。

两条流程使用不同证书和不同 Tauri 配置文件，不要混用。

## 1. 通用准备

### 1.1 构建机器

macOS 打包和签名必须在 Mac 上执行。

建议准备：

```bash
node -v
npm -v
rustc --version
cargo --version
xcodebuild -version
```

如果没有 Rust macOS target，需要安装：

```bash
rustup target add aarch64-apple-darwin
```

当前项目脚本默认打 Apple Silicon：

```text
aarch64-apple-darwin
```

### 1.2 安装依赖

```bash
npm install
```

### 1.3 确认生产接口地址

打包时前端会读取 `.env.production`：

```env
VITE_API_BASE_URL=...
```

修改后必须重新打包才会生效。

### 1.4 查看本机证书

在打包 Mac 上执行：

```bash
security find-identity -v -p codesigning
```

DMG 流程需要 `Developer ID Application` 证书。

Mac App Store 的 `.app` 流程需要 `Apple Distribution` 证书。

Mac App Store 的 `.pkg` 流程还需要 Installer 证书。这个证书名称以你本机钥匙串实际显示为准，文档里的名称只是格式示例。

## 2. DMG 打包流程

### 2.1 使用的配置

DMG 使用：

```text
src-tauri/tauri.macos.conf.json
```

当前关键配置：

```json
{
  "bundle": {
    "targets": ["dmg"],
    "macOS": {
      "frameworks": ["libs/macos/libcomposer.dylib"],
      "signingIdentity": "Developer ID Application: Loogear Co., Ltd. (PMBZKYYRRP)",
      "hardenedRuntime": true
    }
  }
}
```

这里的 `signingIdentity` 必须能在本机钥匙串中找到。

### 2.2 打包命令

```bash
npm run build:dmg
```

等价于：

```bash
tauri build --config src-tauri/tauri.macos.conf.json --target aarch64-apple-darwin --bundles dmg
```

### 2.3 输出位置

一般在：

```text
src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/
```

### 2.4 验证签名

把 `$APP_PATH` 替换成实际 `.app` 路径：

```bash
codesign --verify --deep --strict --verbose=2 "$APP_PATH"
spctl --assess --type execute --verbose "$APP_PATH"
```

### 2.5 注意事项

- DMG 不是 App Store 上传包。
- DMG 使用 `Developer ID Application`，不是 `Apple Distribution`。
- 如果要面向公网分发，通常还需要 notarization。当前项目脚本只负责 build，不包含单独的上传公证步骤。

## 3. Mac App Store 打包流程

### 3.1 使用的配置

Mac App Store 使用：

```text
src-tauri/tauri.appstore.conf.json
```

当前关键配置：

```json
{
  "bundle": {
    "targets": ["app"],
    "category": "Video",
    "macOS": {
      "frameworks": ["libs/macos/libcomposer.dylib"],
      "minimumSystemVersion": "12.0",
      "signingIdentity": "Apple Distribution: Loogear Co., Ltd. (PMBZKYYRRP)",
      "entitlements": "./Entitlements.appstore.plist",
      "infoPlist": "./Info.appstore.plist",
      "files": {
        "embedded.provisionprofile": "profiles/aicut-mas.provisionprofile"
      }
    }
  }
}
```

### 3.2 必要文件

App Store entitlements：

```text
src-tauri/Entitlements.appstore.plist
```

里面包含：

- App Sandbox
- Team ID
- App ID
- 网络访问权限
- 用户选择文件读写权限

App Store Info.plist 补充：

```text
src-tauri/Info.appstore.plist
```

里面包含：

```text
ITSAppUsesNonExemptEncryption=false
```

Provisioning profile 放置位置：

```text
src-tauri/profiles/aicut-mas.provisionprofile
```

这个文件不要提交到 git。它必须是 Apple Developer 后台创建的 macOS `Mac App Store Connect` provisioning profile，Bundle ID 必须匹配：

```text
com.loogear.aicut
```

### 3.3 生成 .app

```bash
npm run build:mas:app
```

等价于：

```bash
tauri build --config src-tauri/tauri.appstore.conf.json --target aarch64-apple-darwin --bundles app
```

输出位置一般在：

```text
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/
```

### 3.4 生成上传用 .pkg

App Store Connect 上传 macOS App 需要 `.pkg`。

先确认本机有 Installer 证书。证书名称以本机实际输出为准：

```bash
security find-identity -v
```

然后执行：

```bash
APPLE_INSTALLER_SIGNING_IDENTITY="你的 Installer 证书名称" npm run build:mas:pkg
```

示例格式：

```bash
APPLE_INSTALLER_SIGNING_IDENTITY="3rd Party Mac Developer Installer: Loogear Co., Ltd. (PMBZKYYRRP)" npm run build:mas:pkg
```

注意：上面的示例不保证就是你本机的真实证书名，必须替换成钥匙串里的实际名称。

`build:mas:pkg` 会先执行 `build:mas:app`，然后调用：

```bash
xcrun productbuild
```

输出位置一般在：

```text
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/
```

文件名格式：

```text
应用名-mas.pkg
```

### 3.5 上传 App Store Connect

可以使用 Apple Transporter 图形工具上传，也可以使用命令行。

Transporter：

1. 打开 Transporter。
2. 登录 App Store Connect 账号。
3. 拖入生成的 `*-mas.pkg`。
4. 点击验证和交付。

命令行方式示例：

```bash
xcrun altool --upload-app \
  --type macos \
  --file "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/应用名-mas.pkg" \
  --apiKey "$APPLE_API_KEY_ID" \
  --apiIssuer "$APPLE_API_ISSUER"
```

也可以使用 `notarytool` / `xcrun iTMSTransporter`，以 Apple 当前推荐工具为准。

## 4. 常见问题

### 4.1 提示找不到 signing identity

检查配置里的证书名是否和钥匙串一致：

```bash
security find-identity -v -p codesigning
```

DMG 看 `Developer ID Application`。

Mac App Store `.app` 看 `Apple Distribution`。

`.pkg` 看 Installer 证书。

### 4.2 提示缺少 provisioning profile

确认文件存在：

```text
src-tauri/profiles/aicut-mas.provisionprofile
```

并确认它的 Bundle ID 是：

```text
com.loogear.aicut
```

### 4.3 App Store 审核或验证提示缺少沙盒

确认 `src-tauri/Entitlements.appstore.plist` 中有：

```xml
<key>com.apple.security.app-sandbox</key>
<true/>
```

### 4.4 App Store 包和 DMG 包证书能不能共用

不能按同一套理解。

- DMG：`Developer ID Application`
- Mac App Store `.app`：`Apple Distribution`
- Mac App Store `.pkg`：Installer 分发证书

### 4.5 如果要打 universal 包

当前脚本默认 Apple Silicon：

```text
aarch64-apple-darwin
```

如果后续要同时支持 Intel 和 Apple Silicon，可以增加 `universal-apple-darwin` 流程，但需要确保 Rust target、Tauri 配置、动态库 `libcomposer.dylib` 都支持 universal。
