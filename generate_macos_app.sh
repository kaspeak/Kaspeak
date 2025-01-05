#!/bin/bash

# Check if cargo is installed
if ! command -v cargo &> /dev/null
then
    echo "Error: cargo is not installed. Please install it to continue."
    exit 1
fi

# Check if cargo-bundle is installed
if ! cargo bundle --help &> /dev/null
then
    echo "Error: cargo-bundle is not installed."
    echo "To install cargo-bundle, run:"
    echo "    cargo install cargo-bundle"
    exit 1
fi

# Execute bundling command
TARGET="aarch64-apple-darwin"
BUNDLE_CMD="cargo bundle --release --target $TARGET"
echo "Executing: $BUNDLE_CMD"
WARN_COUNT=$($BUNDLE_CMD 2>&1 | grep -ic "warning")

if ! $BUNDLE_CMD; then
    echo "Error: Failed to execute bundling."
    exit 1
fi

if [ "$WARN_COUNT" -gt 0 ]; then
    echo "Bundle created with $WARN_COUNT warnings."
else
    echo "Bundle created successfully with no warnings."
fi

# Find the generated bundle path
BUNDLE_PATH="./target/$TARGET/release/bundle/osx"
if [ ! -d "$BUNDLE_PATH" ]; then
    echo "Error: Bundle not found in $BUNDLE_PATH."
    exit 1
fi

# Locate the Info.plist file inside the bundle
INFO_PLIST=$(find "$BUNDLE_PATH" -name "Info.plist" | head -n 1)
if [ -z "$INFO_PLIST" ]; then
    echo "Error: Info.plist file not found."
    exit 1
fi

# Add key to Info.plist
KEY="<key>NSMicrophoneUsageDescription</key>"

if grep -q "$KEY" "$INFO_PLIST"; then
    echo "The key already exists in Info.plist."
else
    echo "Adding the key to Info.plist."
    /usr/libexec/PlistBuddy -c "Add :NSMicrophoneUsageDescription string 'Kaspeak voice recording'" "$INFO_PLIST" 2>/dev/null || {
        echo "<key>NSMicrophoneUsageDescription</key>" >> "$INFO_PLIST"
        echo "<string>Kaspeak voice recording</string>" >> "$INFO_PLIST"
    }
fi

# Output the path to the .app file
APP_PATH=$(find "$BUNDLE_PATH" -name "*.app" | head -n 1)
if [ -n "$APP_PATH" ]; then
    echo "Bundle created successfully. Path to the .app file: $APP_PATH"
else
    echo "Error: .app file not found."
    exit 1
fi
