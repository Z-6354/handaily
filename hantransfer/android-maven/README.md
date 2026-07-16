# hantransfer-android（Maven 构建）

**无需 Android Studio / Gradle**。使用 Maven + Android SDK 命令行工具构建 WebView APK。

## 方式 A：手机浏览器（零构建）

1. 电脑运行 `npm run hantransfer`
2. 手机浏览器打开 `http://<电脑IP>:7822/m/`
3. 手动输入电脑 IP，选择文件发送

**限制：** 浏览器只能发送相册、下载等普通文件，**无法**读取碧蓝航线等 App 的私有目录（`Android/data/...`）。游戏资源请用方式 B 安装 APK，在「碧蓝」页授权 `AssetBundles` 后批量发送。

## 方式 B：Maven 构建 APK

### 环境（一次性）

1. 安装 [Android SDK Command-line Tools](https://developer.android.com/studio#command-tools)（不必装 Android Studio）
2. 安装 [Apache Maven](https://maven.apache.org/download.cgi)
3. 设置 `ANDROID_HOME` 为 SDK 根目录
4. 安装平台：

```bash
sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0"
```

### 构建

依赖下载使用 **阿里云 Maven 镜像**（`pom.xml` 仓库地址 + `maven-settings.xml` 镜像）。

```powershell
# 仓库根目录
npm run hantransfer:apk
```

输出：`hantransfer/release/hantransfer-<version>-debug.apk`（同时更新 `latest-debug.apk`）

```powershell
adb install -r hantransfer/release/hantransfer-latest-debug.apk
```

覆盖安装要求：

1. APK 内必须带递增的 `versionCode`（构建脚本会写入；旧包曾出现空 versionCode，导致只能先卸载）
2. 签名证书一致（使用 `android-maven/signing/hantransfer-debug.keystore`；首次构建会从本机 `~/.android/debug.keystore` 复制）

若仍提示签名不一致，卸载一次后再装即可，之后可正常 `-r` 覆盖。

### 原理

- UI：`hantransfer/mobile-web/`（静态 HTML/JS，打包进 APK assets）
- 原生壳：WebView + `HantransferBridge`（mDNS、文件访问、上传）
- 构建：`android-maven-plugin` 4.6.0

## 模块

```
android-maven/src/main/kotlin/com/handaily/hantransfer/
├── MainActivity.kt       # WebView
├── HantransferBridge.kt  # JS 桥接
├── NsdDiscovery.kt
├── TransferClient.kt
└── AzurlaneModule.kt
```

## 历史说明

早期曾用 Gradle + Compose 脚手架（`hantransfer/android/`），已删除。请使用本目录或浏览器模式。
