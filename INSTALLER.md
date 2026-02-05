# Installing glimpse

## Using the Installer

Download `glimpse-setup.exe` from the [releases page](https://github.com/user/glimpse/releases) and run it as Administrator.

The installer will:
1. Let you choose which file extensions to register (`.gltf`, `.glb`, `.bbmodel`, `.json`)
2. Copy `glimpse.dll` to your Program Files directory
3. Write the necessary COM and shell extension registry keys
4. Prompt you to restart Windows Explorer so thumbnails appear immediately

> **Tip:** Set your folder view to **Medium**, **Large**, or **Extra Large** icons to see thumbnails.

## Uninstalling

Use **Add or Remove Programs** in Windows Settings, or run the uninstaller from the install directory. The uninstaller removes the DLL, cleans up all registry keys, and restarts Explorer.

## Building the Installer from Source

**Prerequisites:**
- [Rust](https://rustup.rs/) with the `stable-x86_64-pc-windows-msvc` toolchain
- [Inno Setup 6](https://jrsoftware.org/isinfo.php)

```powershell
cargo build --release
& "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\glimpse.iss
```

The output is `target\installer\glimpse-setup.exe`.

## Troubleshooting

| Problem | Solution |
|---------|----------|
| No thumbnails appear | Restart Explorer. Verify the DLL path in the registry under `HKLM\SOFTWARE\Classes\CLSID\{A4C82A78-...}\InprocServer32`. |
| Thumbnails are a solid color | The model may reference external textures that cannot be loaded from a stream. |
| Installer fails | Make sure you are running as Administrator. |
| DLL locked / cannot overwrite | Close all Explorer windows and kill `dllhost.exe`, then retry. |
