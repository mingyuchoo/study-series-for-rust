# Root emulator observations

Observer: /root. Device: isolated Stillnote_API_36, emulator-5558, API 36.
Product APK built from source manifest 229b82eff40350a72cdff5fca6529d2b0bef9808fa7f2655f502588a949fa17d; subsequent test-only refinements do not change these product observations.

## Observed

- Actual MainActivity at 1920x1200 physical pixels/density240: Korean dark UI, expanded sidebar, primary yellow capture, Pretendard, readable white system icons. Screenshot `.artifacts/screenshots/tablet-ko-dark.png`, inspected with view_image; reviewer independently inspected it. Main content scrolls below the viewport.
- Reset physical size/density to AVD defaults 1080x2400 before verification.
- Installed debug APK only on isolated emulator-5558 after instrumentation had uninstalled it.
- Actual MainActivity calendar: used native Android text input to change date to 2024-02-01, dismissed keyboard with Back, tapped Monthly, swiped content up. UIAutomator showed day29 at text bounds [116,1737][255,1790]. Tapped physical coordinate (180,1750), swiped back toward top; UIAutomator confirmed Daily and editable date 2024-02-29. This checks actual pointer navigation, rather than invoking the semantics callback.
- An earlier native-input capture persisted an ASCII draft to internal journal.json and retained it after restarting MainActivity. Later instrumentation cleanup uninstalled that isolated app and removed the demonstration data.

## Limits

- Automated synthetic Android InputConnection Hangul protocol checks are separate from a physical Korean keyboard. Installed Gboard on this image has no Korean subtype; actual Korean IME/device behavior is unverified.
- Spoken TalkBack and actual-device accessibility have not been manually verified.
- Screenshots support observed layouts only; they do not prove every device/font/keyboard combination.

## Native keyboard capture on final product APK

On emulator-5558 at default 1080x2400, installed APK built in final-06 (manifest 77b6ce45a7c0d35a963329c4088df47cb62942fc0360ecfb9c021a5e13a56676). Opened MainActivity, focused draft, entered `Native_keyboard_capture` through Android native input/Gboard, left keyboard visible, physically swiped app content upward, and tapped the visible Capture button at (540,820). `run-as app.stillnote cat files/journal.json` confirmed one durable Daily Open Task with exactly that text. Screenshot `.artifacts/screenshots/phone-native-keyboard-final.png` was taken before the pointer capture and visually inspected; keyboard, draft and capture button are visible in separate usable areas. This supports real native keyboard reachability with Latin input on this emulator; it does not verify Korean keyboard composition.

## Final APK first launch

After final-12 automated PASS on manifest 7837fb850a062d64b087ca74865ffbfd050a9b24f7c00e92c5b555d1d99a4cb9, installed APK SHA256 b2e416026fee3baa0bcd8bae452ba94486393e4dc940ca17bf055342596888c3 on isolated emulator-5560 and launched real MainActivity. UIAutomator idle snapshot and physical screencap show blank first launch, Korean/System default resolved to Light, phone navigation and readable system icons; zero entries are visible. Screenshot `.artifacts/screenshots/phone-ko-light-current.png` was inspected by coordinator. No user journal was copied into the app.
