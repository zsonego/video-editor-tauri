# Mac App Store provisioning profile

Place the Mac App Store Connect provisioning profile here as:

```text
src-tauri/profiles/aicut-mas.provisionprofile
```

This file is required by `src-tauri/tauri.appstore.conf.json` and is intentionally ignored by git.

The profile must be a macOS "Mac App Store Connect" profile whose Bundle ID matches:

```text
com.loogear.aicut
```
