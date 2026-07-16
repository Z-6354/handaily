#Requires -Version 5.1
param(
    [switch]$FullClean
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$MavenDir = Join-Path $Root "hantransfer\android-maven"
$ToolsDir = Join-Path $Root ".tools"
$MavenVersion = "3.9.9"
$MavenHome = Join-Path $ToolsDir "apache-maven-$MavenVersion"
$DefaultAndroidHome = Join-Path $ToolsDir "android-sdk"
$MavenSettings = Join-Path $MavenDir "maven-settings.xml"
$script:LastProgress = 0

function Write-BuildProgress {
    param(
        [int]$Percent,
        [string]$Message
    )
    if ($Percent -lt $script:LastProgress) { return }
    $script:LastProgress = $Percent
    Write-Host ("[{0,3}%] {1}" -f $Percent, $Message)
}

function Stop-StaleBuildProcesses {
    $patterns = @(
        "hantransfer\android-maven",
        "build-hantransfer-apk.ps1",
        "hantransfer:apk"
    )
    $killed = 0
    Get-CimInstance Win32_Process -Filter "Name='java.exe'" -ErrorAction SilentlyContinue | ForEach-Object {
        $cmd = $_.CommandLine
        if (-not $cmd) { return }
        foreach ($pattern in $patterns) {
            if ($cmd -like "*$pattern*") {
                Stop-Process -Id $_.ProcessId -Force -ErrorAction SilentlyContinue
                $killed += 1
                break
            }
        }
    }
    if ($killed -gt 0) {
        Write-BuildProgress 8 "已终止 $killed 个旧 Maven 构建进程"
        Start-Sleep -Seconds 1
    }
}

function Ensure-Java {
    $java = Get-Command java -ErrorAction SilentlyContinue
    if (-not $java) {
        throw "Java not found. Install JDK 17+ and add it to PATH."
    }
    $javaHome = Split-Path (Split-Path $java.Source)
    if (-not $env:JAVA_HOME -or -not (Test-Path (Join-Path $env:JAVA_HOME "bin\java.exe"))) {
        $env:JAVA_HOME = $javaHome
    }
    Write-BuildProgress 10 "Java OK: $($env:JAVA_HOME)"
}

function Download-ArchiveWithProgress {
    param(
        [string]$Url,
        [string]$DestZip,
        [int]$ProgressStart,
        [int]$ProgressEnd,
        [string]$Label
    )
    if (Test-Path $DestZip) {
        Write-BuildProgress $ProgressEnd "$Label (cached)"
        return
    }
    New-Item -ItemType Directory -Force -Path (Split-Path $DestZip) | Out-Null
    Write-BuildProgress $ProgressStart "$Label downloading..."
    $request = [System.Net.HttpWebRequest]::Create($Url)
    $request.Method = "GET"
    $response = $request.GetResponse()
    $total = [int64]$response.ContentLength
    $stream = $response.GetResponseStream()
    $fileStream = [System.IO.File]::Create($DestZip)
    try {
        $buffer = New-Object byte[] 81920
        $read = 0L
        while (($n = $stream.Read($buffer, 0, $buffer.Length)) -gt 0) {
            $fileStream.Write($buffer, 0, $n)
            $read += $n
            if ($total -gt 0) {
                $ratio = [math]::Min(1.0, $read / $total)
                $pct = $ProgressStart + [int](($ProgressEnd - $ProgressStart) * $ratio)
                Write-BuildProgress $pct ("$Label " + ([math]::Round($ratio * 100)) + "%")
            }
        }
    } finally {
        $fileStream.Close()
        $stream.Close()
        $response.Close()
    }
    Write-BuildProgress $ProgressEnd "$Label done"
}

function Ensure-Maven {
    $mvnCmd = Get-Command mvn -ErrorAction SilentlyContinue
    if ($mvnCmd) {
        Write-BuildProgress 20 "Maven OK: $($mvnCmd.Source)"
        return $mvnCmd.Source
    }

    $zip = Join-Path $ToolsDir "apache-maven-$MavenVersion-bin.zip"
    if (-not (Test-Path (Join-Path $MavenHome "bin\mvn.cmd"))) {
        $mavenUrl = "https://mirrors.tuna.tsinghua.edu.cn/apache/maven/maven-3/$MavenVersion/binaries/apache-maven-$MavenVersion-bin.zip"
        if (-not (Test-Path $zip)) {
            try {
                Download-ArchiveWithProgress $mavenUrl $zip 12 20 "Maven"
            } catch {
                $mavenUrl = "https://archive.apache.org/dist/maven/maven-3/$MavenVersion/binaries/apache-maven-$MavenVersion-bin.zip"
                Download-ArchiveWithProgress $mavenUrl $zip 12 20 "Maven"
            }
        } else {
            Write-BuildProgress 20 "Maven (cached)"
        }
        Expand-Archive -Path $zip -DestinationPath $ToolsDir -Force
    }
    $path = Join-Path $MavenHome "bin\mvn.cmd"
    Write-BuildProgress 20 "Maven OK: $path"
    return $path
}

function Get-SdkManagerPath {
    param([string]$AndroidHome)
    @(
        (Join-Path $AndroidHome "cmdline-tools\latest\bin\sdkmanager.bat"),
        (Join-Path $AndroidHome "tools\bin\sdkmanager.bat")
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}

function Accept-AndroidLicenses {
    param([string]$SdkManager)
    $marker = Join-Path $ToolsDir "android-licenses.accepted"
    if (Test-Path $marker) { return }
    Write-BuildProgress 28 "Accepting Android SDK licenses..."
    1..40 | ForEach-Object { "y" } | & $SdkManager --licenses | Out-Null
    Set-Content -Path $marker -Value (Get-Date).ToString("o") -Encoding ascii
}

function Ensure-AndroidSdk {
    if ($env:ANDROID_HOME -and (Test-Path $env:ANDROID_HOME)) {
        Write-BuildProgress 25 "ANDROID_HOME=$($env:ANDROID_HOME)"
        Ensure-SdkToolsMarker $env:ANDROID_HOME
        Write-BuildProgress 35 "Android SDK ready"
        return $env:ANDROID_HOME
    }
    $guess = Join-Path $env:LOCALAPPDATA "Android\Sdk"
    if (Test-Path $guess) {
        $env:ANDROID_HOME = $guess
        Write-BuildProgress 25 "ANDROID_HOME=$guess"
        Ensure-SdkToolsMarker $guess
        Write-BuildProgress 35 "Android SDK ready"
        return $guess
    }

    $androidHome = $DefaultAndroidHome
    $sdkmanager = Get-SdkManagerPath $androidHome
    if (-not $sdkmanager) {
        $zip = Join-Path $ToolsDir "commandlinetools-win.zip"
        $sdkZipUrl = "https://dl.google.com/android/repository/commandlinetools-win-13114758_latest.zip"
        if (-not (Test-Path (Join-Path $androidHome "cmdline-tools\latest\bin\sdkmanager.bat"))) {
            Download-ArchiveWithProgress $sdkZipUrl $zip 22 28 "Android cmdline-tools"
            $stage = Join-Path $ToolsDir "cmdline-tools-stage"
            if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
            Expand-Archive -Path $zip -DestinationPath $stage -Force
            $latestDir = Join-Path $androidHome "cmdline-tools\latest"
            New-Item -ItemType Directory -Force -Path $latestDir | Out-Null
            Copy-Item -Path (Join-Path $stage "cmdline-tools\*") -Destination $latestDir -Recurse -Force
            Remove-Item $stage -Recurse -Force
        }
        $sdkmanager = Get-SdkManagerPath $androidHome
    }

    if (-not $sdkmanager) {
        throw "sdkmanager not found under $androidHome"
    }

    $env:ANDROID_HOME = $androidHome
    Write-BuildProgress 25 "ANDROID_HOME=$androidHome"

    $platformJar = Join-Path $androidHome "platforms\android-34\android.jar"
    if (-not (Test-Path $platformJar)) {
        Accept-AndroidLicenses $sdkmanager
        Write-BuildProgress 30 "Installing Android SDK packages..."
        & $sdkmanager "platforms;android-34" "build-tools;34.0.0" "build-tools;28.0.3" "platform-tools"
        if ($LASTEXITCODE -ne 0) { throw "sdkmanager failed with exit $LASTEXITCODE" }
    }
    Write-BuildProgress 35 "Android SDK ready"
    Ensure-SdkToolsMarker $androidHome
    return $androidHome
}

function Ensure-SdkToolsMarker {
    param([string]$AndroidHome)
    $toolsDir = Join-Path $AndroidHome "tools"
    $sourceProps = Join-Path $toolsDir "source.properties"
    if (-not (Test-Path $sourceProps)) {
        New-Item -ItemType Directory -Force -Path $toolsDir | Out-Null
        @(
            "Pkg.Desc=Android SDK Tools"
            "Pkg.Revision=26.1.1"
        ) | Set-Content -Path $sourceProps -Encoding ascii
        Write-BuildProgress 36 "Created SDK tools/source.properties stub"
    }
}

function Ensure-MavenSettings {
    if (-not (Test-Path $MavenSettings)) {
        throw "Missing Maven settings: $MavenSettings"
    }
    Write-BuildProgress 38 "Maven mirror: Aliyun (central + google)"
}

function Ensure-DebugKeystore {
    param([string]$ProjectDir)

    # Prefer project keystore so every build shares one cert (required for overwrite install).
    $projectKs = Join-Path $ProjectDir "signing\hantransfer-debug.keystore"
    if (Test-Path $projectKs) { return $projectKs }

    $signingDir = Split-Path $projectKs
    New-Item -ItemType Directory -Force -Path $signingDir | Out-Null

    # Seed from the machine debug keystore once — keeps existing installs upgradable.
    $userKs = Join-Path $env:USERPROFILE ".android\debug.keystore"
    if (Test-Path $userKs) {
        Copy-Item $userKs $projectKs -Force
        Write-BuildProgress 82 "Seeded project keystore from ~/.android/debug.keystore"
        return $projectKs
    }

    Write-BuildProgress 82 "Creating project debug keystore..."
    $keytool = $null
    if ($env:JAVA_HOME) {
        $candidate = Join-Path $env:JAVA_HOME "bin\keytool.exe"
        if (Test-Path $candidate) { $keytool = $candidate }
    }
    if (-not $keytool) {
        $cmd = Get-Command keytool -ErrorAction SilentlyContinue
        if ($cmd) { $keytool = $cmd.Source }
    }
    if (-not $keytool) { throw "keytool not found (need JDK)" }
    & $keytool -genkeypair -v `
        -keystore $projectKs `
        -storepass android `
        -alias androiddebugkey `
        -keypass android `
        -keyalg RSA `
        -keysize 2048 `
        -validity 10000 `
        -dname "CN=hantransfer Debug,O=HANDAILY,C=CN"
    if ($LASTEXITCODE -ne 0) { throw "keytool failed with exit $LASTEXITCODE" }
    return $projectKs
}

function Invoke-ApkPackaging {
    param(
        [string]$AndroidHome,
        [string]$ProjectDir,
        [int]$VersionCode,
        [string]$VersionName
    )

    Write-BuildProgress 82 "Packaging APK (aapt + apksigner)..."
    if ($VersionCode -lt 1) { throw "VersionCode must be >= 1 (got $VersionCode)" }
    if ([string]::IsNullOrWhiteSpace($VersionName)) { throw "VersionName is required" }

    $target = Join-Path $ProjectDir "target"
    $classesZip = Join-Path $target "classes.zip"
    if (-not (Test-Path $classesZip)) {
        throw "Missing dex output: $classesZip"
    }

    $dexDir = Join-Path $target "dex-out"
    if (Test-Path $dexDir) { Remove-Item $dexDir -Recurse -Force }
    Expand-Archive -Path $classesZip -DestinationPath $dexDir -Force
    $classesDex = Join-Path $dexDir "classes.dex"
    if (-not (Test-Path $classesDex)) {
        throw "classes.dex not found inside $classesZip"
    }

    $buildTools = Join-Path $AndroidHome "build-tools\34.0.0"
    $aapt = Join-Path $buildTools "aapt.exe"
    $zipalign = Join-Path $buildTools "zipalign.exe"
    $apksigner = Join-Path $buildTools "apksigner.bat"
    $androidJar = Join-Path $AndroidHome "platforms\android-34\android.jar"
    $manifest = Join-Path $target "AndroidManifest.xml"
    $resDir = Join-Path $ProjectDir "src\main\res"
    $assetsDir = Join-Path $ProjectDir "src\main\assets"
    $unsignedApk = Join-Path $target "hantransfer-0.1.0-unsigned.apk"
    $alignedApk = Join-Path $target "hantransfer-0.1.0-aligned.apk"
    $signedApk = Join-Path $target "hantransfer-0.1.0.apk"

    foreach ($path in @($aapt, $zipalign, $apksigner, $androidJar, $manifest)) {
        if (-not (Test-Path $path)) { throw "Missing build tool: $path" }
    }

    # Inject version into packaged manifest — empty versionCode blocks overwrite installs on many OEMs.
    # Write attributes into the Maven-copied target manifest (aapt --version-* is ignored when already set / unreliable).
    Write-BuildProgress 83 "APK versionCode=$VersionCode versionName=$VersionName"
    $manifestText = [System.IO.File]::ReadAllText($manifest)
    if ($manifestText -match 'android:versionCode=') {
        $manifestText = [regex]::Replace($manifestText, 'android:versionCode="[^"]*"', "android:versionCode=`"$VersionCode`"")
    } else {
        $manifestText = [regex]::Replace(
            $manifestText,
            '(<manifest\b[^>]*package="[^"]*")',
            "`$1`n    android:versionCode=`"$VersionCode`""
        )
    }
    if ($manifestText -match 'android:versionName=') {
        $manifestText = [regex]::Replace($manifestText, 'android:versionName="[^"]*"', "android:versionName=`"$VersionName`"")
    } else {
        $manifestText = [regex]::Replace(
            $manifestText,
            "(android:versionCode=`"$VersionCode`")",
            "`$1`n    android:versionName=`"$VersionName`""
        )
    }
    [System.IO.File]::WriteAllText($manifest, $manifestText, [System.Text.UTF8Encoding]::new($false))

    if (Test-Path $unsignedApk) { Remove-Item $unsignedApk -Force }
    & $aapt package -f `
        -M $manifest `
        -S $resDir `
        -A $assetsDir `
        -I $androidJar `
        -F $unsignedApk
    if ($LASTEXITCODE -ne 0) { throw "aapt package failed with exit $LASTEXITCODE" }

    Copy-Item $classesDex (Join-Path $target "classes.dex") -Force
    Push-Location $target
    try {
        & $aapt add $unsignedApk "classes.dex"
        if ($LASTEXITCODE -ne 0) { throw "aapt add classes.dex failed with exit $LASTEXITCODE" }
    } finally {
        Pop-Location
    }

    if (Test-Path $alignedApk) { Remove-Item $alignedApk -Force }
    & $zipalign -f 4 $unsignedApk $alignedApk
    if ($LASTEXITCODE -ne 0) { throw "zipalign failed with exit $LASTEXITCODE" }

    $keystore = Ensure-DebugKeystore -ProjectDir $ProjectDir
    if (Test-Path $signedApk) { Remove-Item $signedApk -Force }
    & $apksigner sign `
        --ks $keystore `
        --ks-key-alias androiddebugkey `
        --ks-pass pass:android `
        --key-pass pass:android `
        --v1-signing-enabled true `
        --v2-signing-enabled true `
        --v3-signing-enabled true `
        --out $signedApk `
        $alignedApk
    if ($LASTEXITCODE -ne 0) { throw "apksigner failed with exit $LASTEXITCODE" }

    # Guard: refuse to publish APKs without a real versionCode (overwrite install would fail).
    $badging = & $aapt dump badging $signedApk 2>&1 | Out-String
    if ($badging -notmatch "versionCode='$VersionCode'" -or $badging -notmatch "versionName='$VersionName'") {
        throw "Packaged APK missing versionCode/versionName. badging=`n$badging"
    }

    Write-BuildProgress 88 "APK signed: $signedApk (v$VersionName /$VersionCode)"
}

function Invoke-MavenBuild {
    param(
        [string]$Mvn,
        [switch]$Clean
    )

    $goal = if ($Clean) { "clean package" } else { "package" }
    Write-BuildProgress 40 "Maven $goal starting..."

    $env:MAVEN_OPTS = "-Xms256m -Xmx1024m"
    $args = @(
        "--batch-mode",
        "-s", $MavenSettings,
        "-Dorg.slf4j.simpleLogger.log.org.apache.maven.cli.transfer.Slf4jMavenTransferListener=warn",
        "-T", "1C"
    )
    if (-not $Clean) {
        $args += "-o"
        Write-BuildProgress 42 "Maven offline mode (cached deps)"
    } else {
        $args += "-U"
        Write-BuildProgress 42 "Maven online mode (force update)"
    }
    $args += $goal

    Push-Location $MavenDir
    try {
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $Mvn
        $psi.WorkingDirectory = $MavenDir
        $psi.Arguments = ($args -join " ")
        $psi.UseShellExecute = $false
        $psi.RedirectStandardOutput = $true
        $psi.RedirectStandardError = $true
        $psi.CreateNoWindow = $true

        $proc = New-Object System.Diagnostics.Process
        $proc.StartInfo = $psi
        [void]$proc.Start()

        $phaseMap = [ordered]@{
            "generate-resources" = 45
            "copy-resources"     = 48
            "compile"            = 55
            "kotlin"             = 60
            "dex"                = 70
            "d8"                 = 72
            "antrun"             = 75
            "apk"                = 80
            "BUILD SUCCESS"      = 90
        }

        while (-not $proc.StandardOutput.EndOfStream) {
            $line = $proc.StandardOutput.ReadLine()
            if (-not $line) { continue }
            Write-Host $line
            foreach ($key in $phaseMap.Keys) {
                if ($line -match $key) {
                    Write-BuildProgress $phaseMap[$key] $line.Trim()
                    break
                }
            }
        }
        while (-not $proc.StandardError.EndOfStream) {
            $line = $proc.StandardError.ReadLine()
            if ($line) { Write-Host $line }
        }
        $proc.WaitForExit()
        if ($proc.ExitCode -ne 0) {
            throw "mvn $goal failed with exit $($proc.ExitCode)"
        }
    } finally {
        Pop-Location
    }
}

function Update-ApkVersion {
    param([string]$MavenDir)
    $pomPath = Join-Path $MavenDir "pom.xml"
    $pom = [xml](Get-Content $pomPath)
    $current = [string]$pom.project.version
    if ($current -match '^(\d+)\.(\d+)\.(\d+)$') {
        $major = [int]$Matches[1]
        $minor = [int]$Matches[2]
        $patch = [int]$Matches[3] + 1
        $versionName = "$major.$minor.$patch"
    } else {
        $versionName = "0.1.1"
    }
    $versionCode = [int]($versionName -split '\.' | ForEach-Object { [int]$_ } | ForEach-Object -Begin { $c = 0 } -Process { $c = $c * 100 + $_ } -End { $c })
    $pom.project.version = $versionName
    $pom.Save($pomPath)
    $display = $versionName
    $buildInfo = Join-Path $MavenDir "src\main\res\values\build_info.xml"
    @"
<?xml version="1.0" encoding="utf-8"?>
<resources>
    <string name="app_version_name">$versionName</string>
    <string name="app_version_code">$versionCode</string>
    <string name="app_version_display">$display</string>
</resources>
"@ | Set-Content -Path $buildInfo -Encoding UTF8
    Write-BuildProgress 12 "App version $display (code $versionCode)"
    return @{
        Name = $versionName
        Build = $versionCode
        Display = $display
    }
}

Write-Host "=== hantransfer APK build ==="
Write-BuildProgress 5 "Cleaning stale build processes..."
Stop-StaleBuildProcesses

Ensure-Java
$mvn = Ensure-Maven
$androidHome = Ensure-AndroidSdk
Ensure-MavenSettings
$appVersion = Update-ApkVersion -MavenDir $MavenDir

Invoke-MavenBuild -Mvn $mvn -Clean:$FullClean
Invoke-ApkPackaging `
    -AndroidHome $androidHome `
    -ProjectDir $MavenDir `
    -VersionCode ([int]$appVersion.Build) `
    -VersionName ([string]$appVersion.Name)

Write-BuildProgress 95 "Collecting APK output..."
# Prefer the explicitly signed output (avoid picking intermediate unsigned/aligned apks).
$signedPreferred = Join-Path $MavenDir "target\hantransfer-0.1.0.apk"
$apk = if (Test-Path $signedPreferred) {
    Get-Item $signedPreferred
} else {
    Get-ChildItem -Path (Join-Path $MavenDir "target") -Filter "*.apk" -Recurse |
        Where-Object { $_.Length -gt 0 -and $_.Name -notmatch "unsigned|aligned" } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1
}
if (-not $apk) {
    throw "APK not found under $MavenDir\target (or empty)"
}

$outDir = Join-Path $Root "hantransfer\release"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
$dest = Join-Path $outDir ("hantransfer-{0}-debug.apk" -f $appVersion.Name)
Copy-Item $apk.FullName $dest -Force
$latest = Join-Path $outDir "hantransfer-latest-debug.apk"
Copy-Item $apk.FullName $latest -Force
$meta = @{
    version_name = $appVersion.Name
    build = [int]$appVersion.Build
    display = $appVersion.Display
    filename = [IO.Path]::GetFileName($dest)
    size = (Get-Item $dest).Length
} | ConvertTo-Json -Compress
$latestJsonPath = Join-Path $outDir "latest.json"
[System.IO.File]::WriteAllText($latestJsonPath, $meta, [System.Text.UTF8Encoding]::new($false))

Write-BuildProgress 100 "APK ready: $dest"
Write-Host ""
Write-Host "Version: $($appVersion.Display)"
Write-Host "Published: hantransfer/release/latest.json (phone auto-update source)"
Write-Host "Install: adb install -r `"$dest`""
Write-Host "PC admin: http://127.0.0.1:7822/ -> 手机 App 更新"
