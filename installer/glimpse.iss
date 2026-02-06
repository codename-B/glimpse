; glimpse Inno Setup installer script
; Installs the glimpse COM shell extension for 3D model thumbnails
; in Windows Explorer.
;
; Build:
;   cargo build --release
;   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" installer\glimpse.iss

#define MyAppName "glimpse"
#define MyAppVersion "0.2.0"
#define MyAppPublisher "glimpse Contributors"
#define MyAppURL "https://github.com/user/glimpse"
#define MyAppDescription "3D model thumbnail previews in Windows Explorer"
#define MyCLSID "{{A4C82A78-4C33-4420-83C4-F77C8C80514D}"
#define MyThumbGUID "{{E357FCCD-A995-4576-B01F-234630154E96}"

[Setup]
AppId={#MyCLSID}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
DefaultDirName={pf}\{#MyAppName}
DefaultGroupName={#MyAppName}
UninstallDisplayName={#MyAppName} Thumbnail Provider
OutputDir=..\target\installer
OutputBaseFilename=glimpse-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
MinVersion=6.1sp1
PrivilegesRequired=admin
DisableProgramGroupPage=yes
DisableDirPage=yes
LicenseFile=..\LICENSE

[Files]
Source: "..\target\release\glimpse.dll"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\glimpse-cli.exe"; DestDir: "{app}"; Flags: ignoreversion; Tasks: install_cli

; ---------------------------------------------------------------------------
; Tasks -- extension selection checkboxes
; ---------------------------------------------------------------------------
[Tasks]
Name: "ext_gltf";    Description: "Register .gltf (glTF 3D Model)";         GroupDescription: "File extensions to handle:"; Flags: checkedonce
Name: "ext_glb";     Description: "Register .glb (GLB 3D Model)";           GroupDescription: "File extensions to handle:"; Flags: checkedonce
Name: "ext_bbmodel"; Description: "Register .bbmodel (Blockbench 3D Model)"; GroupDescription: "File extensions to handle:"; Flags: checkedonce
Name: "ext_json";    Description: "Register .json (Vintage Story models)";  GroupDescription: "File extensions to handle:"; Flags: checkedonce
Name: "install_cli"; Description: "Install glimpse-cli.exe (command-line tool)"; GroupDescription: "Additional components:"

; ---------------------------------------------------------------------------
; Registry -- COM registration (always) + per-extension (conditional)
; ---------------------------------------------------------------------------
[Registry]

; --- COM InprocServer32 (always) ---
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#MyCLSID}"; ValueType: string; ValueName: ""; ValueData: "glimpse Thumbnail Provider"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#MyCLSID}\InprocServer32"; ValueType: string; ValueName: ""; ValueData: "{app}\glimpse.dll"; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\CLSID\{#MyCLSID}\InprocServer32"; ValueType: string; ValueName: "ThreadingModel"; ValueData: "Both"

; --- Shell Extensions Approved (always) ---
Root: HKLM; Subkey: "SOFTWARE\Microsoft\Windows\CurrentVersion\Shell Extensions\Approved"; ValueType: string; ValueName: "{#MyCLSID}"; ValueData: "glimpse Thumbnail Provider"; Flags: uninsdeletevalue

; --- .gltf ---
Root: HKLM; Subkey: "SOFTWARE\Classes\.gltf"; ValueType: string; ValueName: ""; ValueData: "gltffile"; Tasks: ext_gltf
Root: HKLM; Subkey: "SOFTWARE\Classes\.gltf"; ValueType: string; ValueName: "Content Type"; ValueData: "model/gltf+json"; Tasks: ext_gltf
Root: HKLM; Subkey: "SOFTWARE\Classes\.gltf"; ValueType: string; ValueName: "PerceivedType"; ValueData: "document"; Tasks: ext_gltf
Root: HKLM; Subkey: "SOFTWARE\Classes\.gltf\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_gltf; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\gltffile"; ValueType: string; ValueName: ""; ValueData: "glTF 3D Model"; Tasks: ext_gltf; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\gltffile\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_gltf; Flags: uninsdeletekey

; --- .glb ---
Root: HKLM; Subkey: "SOFTWARE\Classes\.glb"; ValueType: string; ValueName: ""; ValueData: "glbfile"; Tasks: ext_glb
Root: HKLM; Subkey: "SOFTWARE\Classes\.glb"; ValueType: string; ValueName: "Content Type"; ValueData: "model/gltf-binary"; Tasks: ext_glb
Root: HKLM; Subkey: "SOFTWARE\Classes\.glb"; ValueType: string; ValueName: "PerceivedType"; ValueData: "document"; Tasks: ext_glb
Root: HKLM; Subkey: "SOFTWARE\Classes\.glb\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_glb; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\glbfile"; ValueType: string; ValueName: ""; ValueData: "GLB 3D Model"; Tasks: ext_glb; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\glbfile\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_glb; Flags: uninsdeletekey

; --- .bbmodel ---
Root: HKLM; Subkey: "SOFTWARE\Classes\.bbmodel"; ValueType: string; ValueName: ""; ValueData: "bbmodelfile"; Tasks: ext_bbmodel
Root: HKLM; Subkey: "SOFTWARE\Classes\.bbmodel"; ValueType: string; ValueName: "Content Type"; ValueData: "application/json"; Tasks: ext_bbmodel
Root: HKLM; Subkey: "SOFTWARE\Classes\.bbmodel"; ValueType: string; ValueName: "PerceivedType"; ValueData: "document"; Tasks: ext_bbmodel
Root: HKLM; Subkey: "SOFTWARE\Classes\.bbmodel\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_bbmodel; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\bbmodelfile"; ValueType: string; ValueName: ""; ValueData: "Blockbench 3D Model"; Tasks: ext_bbmodel; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\bbmodelfile\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_bbmodel; Flags: uninsdeletekey

; --- .json (Vintage Story -- do NOT override file type association) ---
Root: HKLM; Subkey: "SOFTWARE\Classes\.json\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_json; Flags: uninsdeletekey
Root: HKLM; Subkey: "SOFTWARE\Classes\jsonfile\ShellEx\{#MyThumbGUID}"; ValueType: string; ValueName: ""; ValueData: "{#MyCLSID}"; Tasks: ext_json; Flags: uninsdeletevalue

; ---------------------------------------------------------------------------
; Clean up install directory on uninstall
; ---------------------------------------------------------------------------
[UninstallDelete]
Type: filesandordirs; Name: "{app}"

; ---------------------------------------------------------------------------
; Pascal Script -- process management + dynamic class registration
; ---------------------------------------------------------------------------
[Code]

// Kill a process by image name (silently ignores if not running).
procedure KillProcess(Name: String);
var
  ResultCode: Integer;
begin
  Exec('taskkill.exe', '/F /IM ' + Name, '', SW_HIDE, ewWaitUntilTerminated, ResultCode);
end;

// Kill Explorer and dllhost so the DLL file handle is released.
// Required before overwriting or deleting the DLL.
procedure KillShellProcesses;
begin
  KillProcess('dllhost.exe');
  KillProcess('explorer.exe');
  Sleep(500);
end;

// Start Explorer again.
procedure StartExplorer;
var
  ResultCode: Integer;
begin
  Exec(ExpandConstant('{win}\explorer.exe'), '', '', SW_SHOWNORMAL, ewNoWait, ResultCode);
end;

// Register the thumbnail handler on an existing file-type class if one
// exists for the given extension.
procedure RegisterOnExistingClass(Extension: String);
var
  ClassName: String;
begin
  if RegQueryStringValue(HKEY_CLASSES_ROOT, Extension, '', ClassName) then
  begin
    if (ClassName <> '') then
    begin
      RegWriteStringValue(
        HKEY_LOCAL_MACHINE,
        'SOFTWARE\Classes\' + ClassName + '\ShellEx\{E357FCCD-A995-4576-B01F-234630154E96}',
        '',
        '{A4C82A78-4C33-4420-83C4-F77C8C80514D}');
    end;
  end;
end;

// ---------------------------------------------------------------
// Install events
// ---------------------------------------------------------------
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then
  begin
    // Kill shell processes BEFORE file copy so the DLL is not locked
    KillShellProcesses;
  end;

  if CurStep = ssPostInstall then
  begin
    // Register on any existing file-type classes that override our defaults
    // (e.g. Blockbench may have set .bbmodel to "Blockbench Project")
    if IsTaskSelected('ext_gltf') then
      RegisterOnExistingClass('.gltf');
    if IsTaskSelected('ext_glb') then
      RegisterOnExistingClass('.glb');
    if IsTaskSelected('ext_bbmodel') then
      RegisterOnExistingClass('.bbmodel');
    if IsTaskSelected('ext_json') then
      RegisterOnExistingClass('.json');

    // Ask user whether to restart Explorer now
    if MsgBox('Installation complete.' + #13#10 + #13#10 +
              'Windows Explorer must be restarted for thumbnails to appear.' + #13#10 +
              'Restart Explorer now?',
              mbConfirmation, MB_YESNO) = IDYES then
    begin
      StartExplorer;
    end
    else
    begin
      // Explorer was killed during install -- must restart it regardless
      StartExplorer;
    end;
  end;
end;

// ---------------------------------------------------------------
// Uninstall events
// ---------------------------------------------------------------
function InitializeUninstall: Boolean;
begin
  Result := True;
  KillShellProcesses;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    // Always restart Explorer after uninstall (it was killed above)
    StartExplorer;
  end;
end;
