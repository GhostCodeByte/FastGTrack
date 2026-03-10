param(
    [ValidateSet("x86_64-linux-android", "aarch64-linux-android")]
    [string]$Target = "x86_64-linux-android",

    [switch]$Release,

    [switch]$Install,

    [string]$Device = ""
)

$ErrorActionPreference = "Stop"

function Require-Command($Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Command '$Name' wurde nicht gefunden. Bitte installieren oder zum PATH hinzufuegen."
    }
}

Require-Command cargo
Require-Command rustup

if (-not (Get-Command cargo-apk -ErrorAction SilentlyContinue)) {
    throw "cargo-apk fehlt. Installiere es mit: cargo install cargo-apk"
}

if (-not $env:ANDROID_HOME -and $env:ANDROID_SDK_ROOT) {
    $env:ANDROID_HOME = $env:ANDROID_SDK_ROOT
}

if (-not $env:ANDROID_HOME) {
    $sdkCandidate = Join-Path $env:LOCALAPPDATA "Android\Sdk"
    if (Test-Path $sdkCandidate) {
        $env:ANDROID_HOME = $sdkCandidate
        $env:ANDROID_SDK_ROOT = $sdkCandidate
    }
}

if (-not $env:ANDROID_NDK_ROOT) {
    $ndkRoot = Join-Path $env:ANDROID_HOME "ndk"
    if (Test-Path $ndkRoot) {
        $latestNdk = Get-ChildItem $ndkRoot -Directory | Sort-Object Name -Descending | Select-Object -First 1
        if ($latestNdk) {
            $env:ANDROID_NDK_ROOT = $latestNdk.FullName
        }
    }
}

if (-not $env:JAVA_HOME) {
    $javaCandidate = "C:\Program Files\Android\Android Studio\jbr"
    if (Test-Path $javaCandidate) {
        $env:JAVA_HOME = $javaCandidate
    }
}

if (-not $env:ANDROID_HOME) {
    throw "ANDROID_HOME / ANDROID_SDK_ROOT nicht gesetzt und SDK nicht gefunden."
}

if (-not $env:ANDROID_NDK_ROOT) {
    throw "ANDROID_NDK_ROOT nicht gesetzt und keine NDK-Installation in '$($env:ANDROID_HOME)\ndk' gefunden."
}

if (-not $env:JAVA_HOME) {
    throw "JAVA_HOME nicht gesetzt und Android-Studio-Java wurde nicht gefunden."
}

Write-Host "Using ANDROID_HOME=$($env:ANDROID_HOME)"
Write-Host "Using ANDROID_NDK_ROOT=$($env:ANDROID_NDK_ROOT)"
Write-Host "Using JAVA_HOME=$($env:JAVA_HOME)"

rustup target add $Target | Out-Host

$apkArgs = @("apk", "build", "--lib", "--target", $Target)
if ($Release) {
    $apkArgs += "--release"
}

& cargo @apkArgs
if ($LASTEXITCODE -ne 0) {
    throw "APK-Build fehlgeschlagen."
}

$apkRoots = @(
    (Join-Path (Join-Path "target" $Target) "apk"),
    (Join-Path (Join-Path "target" ($(if ($Release) { "release" } else { "debug" }))) "apk"),
    (Join-Path "target" "apk")
) | Select-Object -Unique

$apk = $null
foreach ($apkRoot in $apkRoots) {
    if (Test-Path $apkRoot) {
        $apk = Get-ChildItem $apkRoot -Filter "*.apk" -Recurse | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($apk) {
            break
        }
    }
}

if (-not $apk) {
    throw "Keine APK gefunden. Gepruefte Ordner: $($apkRoots -join ', ')"
}

Write-Host "APK erstellt: $($apk.FullName)"

if ($Install) {
    Require-Command adb
    $adbArgs = @()
    if ($Device) {
        $adbArgs += @("-s", $Device)
    }
    & adb @adbArgs install -r $apk.FullName
    if ($LASTEXITCODE -ne 0) {
        throw "APK-Installation fehlgeschlagen."
    }
    Write-Host "APK installiert."
}
