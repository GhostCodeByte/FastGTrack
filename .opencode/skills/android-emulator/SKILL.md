---
name: android-emulator
description: Automate and test Android apps running in an emulator via ADB. Use this skill whenever the user wants to interact with an Android emulator or device — including taking screenshots, tapping UI elements, typing text, reading UI hierarchy, scrolling, or running automated UI tests on an Expo or native Android app.
compatibility:
  requires: adb (Android Debug Bridge, part of Android SDK platform-tools)
---

# Android Emulator Automation Skill

Automate Android emulators using ADB (Android Debug Bridge). No extra tools needed — ADB ships with Android Studio and is available in the PATH on any machine with an emulator running.

## Prerequisites Check

Before doing anything, verify ADB is available and an emulator is connected:

```bash
adb devices
```

Expected output: one line ending in `emulator-XXXX  device`. If no device shows, the emulator isn't running or ADB server needs a restart (`adb kill-server && adb start-server`).

---

## Core Operations

### 1. Screenshot

Capture what's currently on screen and pull it to disk:

```bash
# Capture on device, pull to local file
adb shell screencap -p /sdcard/screen.png
adb pull /sdcard/screen.png ./screen.png
adb shell rm /sdcard/screen.png
```

To view immediately (Linux/macOS):
```bash
# After pulling: open or display the file
xdg-open ./screen.png   # Linux
open ./screen.png        # macOS
```

For agent use: after pulling, read the file as an image and describe/analyze it.

---

### 2. Read UI Hierarchy (better than screenshots for finding elements)

Dumps the full accessibility tree — gives you element text, resource IDs, bounds, and clickability. Faster and more reliable than coordinate-guessing from screenshots.

```bash
adb shell uiautomator dump /sdcard/ui.xml
adb pull /sdcard/ui.xml ./ui.xml
adb shell rm /sdcard/ui.xml
cat ./ui.xml
```

**How to read the output:**

Each `<node>` has:
- `text="Login"` — visible label
- `resource-id="com.myapp:id/btn_login"` — stable identifier (use this for tapping when available)
- `bounds="[100,200][300,250]"` — `[left,top][right,bottom]` — center = `((100+300)/2, (200+250)/2)` = `(200, 225)`
- `clickable="true"` — whether it accepts taps

**Parse bounds to get tap coordinates:**
```bash
# Extract bounds of a specific element by text
grep -o 'text="Login"[^/]*/>' ./ui.xml | grep -o 'bounds="[^"]*"'
```

---

### 3. Tap / Click

Tap by coordinates (get from UI hierarchy bounds, see above):

```bash
# adb shell input tap <x> <y>
adb shell input tap 200 225
```

Tap by resource ID (more reliable, survives layout changes):
```bash
# Find center coords from bounds first, then tap
# Or use uiautomator directly for ID-based interaction (see Advanced section)
```

---

### 4. Type Text

First tap the input field, then type:

```bash
# Tap into field first
adb shell input tap 200 400

# Type text (spaces need special handling)
adb shell input text "hello@example.com"

# For text with spaces, use %s
adb shell input text "hello%sworld"

# Special keys
adb shell input keyevent 66   # ENTER
adb shell input keyevent 67   # BACKSPACE
adb shell input keyevent 4    # BACK button
adb shell input keyevent 3    # HOME button
```

**Common keycodes:**
| Key | Code |
|-----|------|
| Enter | 66 |
| Backspace | 67 |
| Tab | 61 |
| Back | 4 |
| Home | 3 |
| Delete (forward) | 112 |

---

### 5. Scroll

```bash
# Swipe: adb shell input swipe <x1> <y1> <x2> <y2> [duration_ms]
# Scroll down (swipe up)
adb shell input swipe 500 800 500 300 300

# Scroll up (swipe down)
adb shell input swipe 500 300 500 800 300

# Scroll right to left
adb shell input swipe 800 500 200 500 300
```

---

## Typical Agent Workflow

For an agent navigating an app, the recommended loop is:

```
1. adb shell screencap + pull  ->  look at screenshot
2. adb shell uiautomator dump + pull  ->  find element by text/ID
3. Calculate tap coords from bounds
4. adb shell input tap <x> <y>  OR  adb shell input text "..."
5. Short wait (sleep 0.5-1s for animations)
6. Repeat from step 1
```

```bash
# Wait helper between actions
sleep 0.8
```

---

## Advanced: UIAutomator for Robust Interactions

For more reliable automation (especially when coordinates shift), use UIAutomator scripts. This requires a small Java/Kotlin test file, but can be triggered via ADB:

```bash
# Check if uiautomator test runner is available
adb shell pm list instrumentation
```

For most Expo/React Native app testing, the basic `input tap` + `uiautomator dump` approach is sufficient.

---

## Expo-Specific Notes

When your Expo app is running in the emulator:

- **Dev menu**: `adb shell input keyevent 82` (opens Expo dev menu)
- **Reload**: `adb shell input keyevent 82` then tap "Reload" OR `adb shell input text "rr"` (if RCTDevMenu is configured for keyboard shortcut)
- **App package**: usually `host.exp.exponent` for Expo Go, or your own bundle ID for standalone builds
- **Launch app**:
  ```bash
  # Open Expo Go
  adb shell monkey -p host.exp.exponent -c android.intent.category.LAUNCHER 1

  # Open your standalone app (replace with your bundle ID)
  adb shell monkey -p com.yourcompany.yourapp -c android.intent.category.LAUNCHER 1
  ```

---

## Troubleshooting

| Problem | Fix |
|---------|-----|
| `adb: command not found` | Add Android SDK platform-tools to PATH: `export PATH=$PATH:~/Android/Sdk/platform-tools` |
| `error: no devices/emulators found` | Make sure emulator is fully booted; run `adb kill-server && adb start-server` |
| `uiautomator dump` returns empty tree | App may be using a WebView — screenshot approach is better in that case |
| Text not typed correctly | Avoid special characters; for complex strings use clipboard: `adb shell am broadcast -a clipper.set -e text "your text"` then long-press paste |
| Tap doesn't register | Coordinates off — re-dump UI hierarchy and recalculate from fresh bounds |

---

## Reference: Full Screenshot + Analyze Loop (copy-paste ready)

```bash
#!/bin/bash
# Take screenshot, pull, display filename
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
FILENAME="screen_${TIMESTAMP}.png"
adb shell screencap -p /sdcard/screen.png
adb pull /sdcard/screen.png "./${FILENAME}"
adb shell rm /sdcard/screen.png
echo "Saved: ${FILENAME}"
```
