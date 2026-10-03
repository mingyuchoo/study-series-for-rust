#!/usr/bin/env bash
set -euo pipefail

usage() {
  echo 'Usage: bash scripts/release.sh [--check-formatting]'
  echo 'macOS: .app, .dmg, .pkg; Debian/Ubuntu Linux: .tar.gz, .deb'
}
die() { echo "error: $*" >&2; exit 1; }
require() { command -v "$1" >/dev/null 2>&1 || die "Required command not found: $1"; }
check_formatting=false
for argument in "$@"; do
  case "$argument" in
    --check-formatting) check_formatting=true ;;
    -h|--help) usage; exit 0 ;;
    *) usage >&2; die "Unknown argument: $argument" ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
cd "$PROJECT_ROOT"
for tool in cargo rustc python3 tar; do require "$tool"; done
platform="$(uname -s)"
case "$platform" in
  Darwin)
    for tool in xcrun pkgbuild hdiutil shasum otool; do require "$tool"; done
    xcrun -sdk macosx metal -v >/dev/null 2>&1 || die 'Install Metal Toolchain: xcodebuild -downloadComponent MetalToolchain'
    ;;
  Linux)
    for tool in dpkg-deb dpkg-shlibdeps sha256sum install; do require "$tool"; done
    ;;
  *) die 'Supported systems: macOS and Debian/Ubuntu Linux. On Windows use scripts/release.ps1.' ;;
esac

rust_info="$(rustc -vV)"
target="$(printf '%s\n' "$rust_info" | sed -n 's/^host: //p')"
case "$platform:$target" in
  Darwin:x86_64-apple-darwin|Darwin:aarch64-apple-darwin) ;;
  Linux:x86_64-unknown-linux-gnu) deb_arch=amd64 ;;
  Linux:aarch64-unknown-linux-gnu) deb_arch=arm64 ;;
  *) die "Unsupported native target: $target" ;;
esac
metadata="$(cargo metadata --no-deps --format-version 1 --locked)"
package_info="$(printf '%s' "$metadata" | python3 -c '
import json, sys
m = json.load(sys.stdin)
p = next(p for p in m["packages"] if p["name"] == "window-app")
print(p["version"] + "\t" + m["target_directory"])
')"
IFS=$'\t' read -r version target_dir <<< "$package_info"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || die 'Installer requires a numeric major.minor.patch version.'

echo '==> Formatting Rust code'
if "$check_formatting"; then cargo fmt --all -- --check; else cargo fmt --all; fi
echo '==> Linting release targets'
cargo clippy --workspace --all-targets --release --locked --target "$target" -- -D warnings
echo '==> Building release targets'
cargo build --workspace --all-targets --release --locked --target "$target"
echo '==> Testing release targets'
cargo test --workspace --release --locked --target "$target"

binary="$target_dir/$target/release/window-app"
[[ -x "$binary" ]] || die "Missing executable: $binary"
dist="$PROJECT_ROOT/dist/$version/$target"
mkdir -p "$dist"
stage="$(mktemp -d "$dist/.stage-XXXXXX")"
trap 'rm -rf -- "$stage"' EXIT
artifact_name="window-app-$version-$target"

if [[ "$platform" == Darwin ]]; then
  echo '==> Creating macOS application bundle'
  app="$stage/window-app.app"
  mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
  cp "$binary" "$app/Contents/MacOS/window-app"
  cp README.md "$app/Contents/Resources/README.md"
  # External dylibs need explicit relocation/signing before they can be shipped.
  dependencies="$(otool -L "$binary")"
  while IFS= read -r dependency; do
    case "$dependency" in
      /System/Library/*|/usr/lib/*|'') ;;
      *) die "Non-system dylib requires bundling: $dependency" ;;
    esac
  done < <(printf '%s\n' "$dependencies" | tail -n +2 | sed -E 's/^[[:space:]]+//; s/ \(compatibility version.*$//')
  cat > "$app/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleIdentifier</key><string>io.github.mingyuchoo.window-app</string>
<key>CFBundleName</key><string>window-app</string>
<key>CFBundleExecutable</key><string>window-app</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>$version</string>
<key>CFBundleVersion</key><string>$version</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
EOF
  echo '==> Creating DMG and PKG installer'
  mkdir -p "$stage/dmg"
  cp -R "$app" "$stage/dmg/"
  ln -s /Applications "$stage/dmg/Applications"
  hdiutil create -ov -volname window-app -srcfolder "$stage/dmg" -format UDZO "$dist/$artifact_name.dmg"
  pkgbuild --component "$app" --install-location /Applications \
    --identifier io.github.mingyuchoo.window-app --version "$version" "$dist/$artifact_name.pkg"
  # Publish the .app only after both packaging steps succeed.
  if [[ -e "$dist/window-app.app" || -L "$dist/window-app.app" ]]; then
    mv "$dist/window-app.app" "$stage/previous.app"
  fi
  mv "$app" "$dist/window-app.app"
  (cd "$dist"; shasum -a 256 "$artifact_name.dmg" "$artifact_name.pkg" > SHA256SUMS)
else
  echo '==> Creating portable archive'
  mkdir -p "$stage/window-app"
  cp "$binary" README.md "$stage/window-app/"
  tar -czf "$dist/$artifact_name.tar.gz" -C "$stage" window-app
  echo '==> Creating Debian installer'
  deb_root="$stage/deb"
  mkdir -p "$deb_root/DEBIAN" "$deb_root/usr/bin" "$deb_root/usr/share/applications" "$stage/debian"
  install -m 755 "$binary" "$deb_root/usr/bin/window-app"
  cat > "$deb_root/usr/share/applications/window-app.desktop" <<'EOF'
[Desktop Entry]
Type=Application
Name=window-app
Comment=GPUI window application
Exec=window-app
Terminal=false
Categories=Utility;
EOF
  # dpkg-shlibdeps uses debian/control and the host's installed library metadata.
  cat > "$stage/debian/control" <<EOF
Source: window-app
Section: utils
Priority: optional
Maintainer: window-app maintainers <noreply@users.noreply.github.com>

Package: window-app
Architecture: any
Description: GPUI window application
EOF
  dependency_info="$(cd "$stage"; dpkg-shlibdeps -O -e"$deb_root/usr/bin/window-app")"
  dependencies="$(printf '%s\n' "$dependency_info" | sed -n 's/^shlibs:Depends=//p')"
  [[ -n "$dependencies" ]] || die 'Could not determine Debian runtime dependencies.'
  cat > "$deb_root/DEBIAN/control" <<EOF
Package: window-app
Version: $version
Section: utils
Priority: optional
Architecture: $deb_arch
Maintainer: window-app maintainers <noreply@users.noreply.github.com>
Depends: $dependencies
Description: GPUI window application
 A GPU-accelerated desktop application built with Rust and GPUI.
EOF
  chmod 755 "$deb_root/DEBIAN"
  chmod 644 "$deb_root/DEBIAN/control" "$deb_root/usr/share/applications/window-app.desktop"
  dpkg-deb --root-owner-group --build "$deb_root" "$dist/$artifact_name.deb"
  (cd "$dist"; sha256sum "$artifact_name.tar.gz" "$artifact_name.deb" > SHA256SUMS)
fi
echo "==> Release artifacts: $dist"
