; Buddy 的 Windows per-user 安装器。由 bundle-windows.ps1 通过 /D 参数调用。
; 不要在此处写入或删除 %APPDATA%\com.buddy.chat：其中保存用户设置和聊天记录。

Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ShowUninstDetails show

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "StrFunc.nsh"
!include "FileFunc.nsh"
${UnStrStr}

!ifndef VERSION
  !error "缺少 VERSION（例如 /DVERSION=0.1.12）"
!endif
!ifndef STAGE_DIR
  !error "缺少 STAGE_DIR"
!endif
!ifndef OUTFILE
  !error "缺少 OUTFILE"
!endif
!ifndef APP_NAME
  !define APP_NAME "Buddy"
!endif
!ifndef APP_ID
  !define APP_ID "Buddy"
!endif
!ifndef BUDDY_MUTEX
  !define BUDDY_MUTEX "Local\com.buddy.chat.v2.instance"
!endif

!define UNINSTALL_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
!define RUN_KEY "Software\Microsoft\Windows\CurrentVersion\Run"
!define START_MENU_DIR "$SMPROGRAMS\${APP_NAME}"

Name "${APP_NAME} ${VERSION}"
OutFile "${OUTFILE}"
InstallDir "$LOCALAPPDATA\Programs\${APP_NAME}"
InstallDirRegKey HKCU "${UNINSTALL_KEY}" "InstallLocation"
BrandingText "Buddy"

!ifdef ICON_FILE
  Icon "${ICON_FILE}"
  UninstallIcon "${ICON_FILE}"
!endif

!define MUI_ABORTWARNING
!define MUI_WELCOMEPAGE_TITLE "安装 ${APP_NAME}"
!define MUI_WELCOMEPAGE_TEXT "此向导将在当前 Windows 用户下安装 ${APP_NAME}。$\r$\n$\r$\n安装不会请求管理员权限，也不会修改其他用户的数据。"
!define MUI_DIRECTORYPAGE_TEXT_TOP "${APP_NAME} 安装在当前用户的本地应用目录。"
!define MUI_FINISHPAGE_RUN "$INSTDIR\buddy.exe"
!define MUI_FINISHPAGE_RUN_TEXT "立即启动 ${APP_NAME}"
!define MUI_FINISHPAGE_LINK "Buddy 项目主页"
!define MUI_FINISHPAGE_LINK_LOCATION "https://github.com/dcdyouget/buddy"

!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${STAGE_DIR}\LICENSE-GPL-3.0-or-later"
!insertmacro MUI_PAGE_COMPONENTS
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "SimpChinese"

; 默认使用当前用户的 Shell 文件夹，避免在管理员上下文留下错误的快捷方式。
Function .onInit
  SetShellVarContext current
FunctionEnd

; 使用独占写入句柄检测正在运行的 Buddy。不能用 Rename：运行中的 EXE
; 可能允许 FILE_SHARE_DELETE。提示用户关闭后重试，绝不强制结束其进程。
Function EnsureBuddyClosed
check:
  ; Buddy 运行时持有这个单实例 mutex。它覆盖旧的便携版和已安装版，
  ; 在文件共享策略意外放宽时仍能可靠阻止覆盖安装。
  System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${BUDDY_MUTEX}") p .r0'
  IntCmp $0 0 no_mutex
  System::Call 'kernel32::CloseHandle(p r0)'
  Goto busy
no_mutex:
  IfFileExists "$INSTDIR\buddy.exe" 0 finished
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\buddy.exe", i 0x40000000, i 0, p 0, i 3, i 0, p 0) p .r0'
  IntCmp $0 -1 busy
  System::Call 'kernel32::CloseHandle(p r0)'
  Goto finished
busy:
  IfSilent cancelled
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Buddy 正在运行，无法更新其程序文件。请先从托盘退出 Buddy，然后点击“重试”。" /SD IDCANCEL IDRETRY check IDCANCEL cancelled
cancelled:
  SetErrorLevel 32
  Abort
finished:
FunctionEnd

Section "${APP_NAME}（必选）" SecCore
  SectionIn RO
  SetShellVarContext current
  Call EnsureBuddyClosed
  SetOutPath "$INSTDIR"
  File "${STAGE_DIR}\buddy.exe"
  File "${STAGE_DIR}\VCRUNTIME140.dll"
  File "${STAGE_DIR}\LICENSE"
  File "${STAGE_DIR}\LICENSE-GPL-3.0-or-later"
  File "${STAGE_DIR}\LICENSE-APACHE-2.0"
  File "${STAGE_DIR}\THIRD_PARTY_NOTICES.md"
  File "${STAGE_DIR}\icon.ico"
  WriteUninstaller "$INSTDIR\Uninstall ${APP_NAME}.exe"

  CreateDirectory "${START_MENU_DIR}"
  CreateShortcut "${START_MENU_DIR}\${APP_NAME}.lnk" "$INSTDIR\buddy.exe" "" "$INSTDIR\icon.ico"
  CreateShortcut "${START_MENU_DIR}\卸载 ${APP_NAME}.lnk" "$INSTDIR\Uninstall ${APP_NAME}.exe"

  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "DisplayIcon" "$INSTDIR\icon.ico"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "Publisher" "Buddy"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "URLInfoAbout" "https://github.com/dcdyouget/buddy"
  WriteRegStr HKCU "${UNINSTALL_KEY}" "UninstallString" "$\"$INSTDIR\Uninstall ${APP_NAME}.exe$\""
  WriteRegStr HKCU "${UNINSTALL_KEY}" "QuietUninstallString" "$\"$INSTDIR\Uninstall ${APP_NAME}.exe$\" /S"
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  WriteRegDWORD HKCU "${UNINSTALL_KEY}" "EstimatedSize" "$0"
SectionEnd

Section /o "创建桌面快捷方式" SecDesktop
  SetShellVarContext current
  CreateShortcut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\buddy.exe" "" "$INSTDIR\icon.ico"
SectionEnd

Function un.onInit
  SetShellVarContext current
un_init_check:
  ; 先在初始化阶段检查单实例 mutex，保证静默卸载不会在 Buddy 仍运行时
  ; 删除注册表和快捷方式。文件锁检测仍在卸载 section 中作为后备。
  System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${BUDDY_MUTEX}") p .r0'
  IntCmp $0 0 un_init_done
  System::Call 'kernel32::CloseHandle(p r0)'
  IfSilent un_init_cancelled
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Buddy 正在运行，无法卸载。请先从托盘退出 Buddy，然后点击“重试”。" /SD IDCANCEL IDRETRY un_init_check IDCANCEL un_init_cancelled
un_init_cancelled:
  SetErrorLevel 32
  Abort
un_init_done:
FunctionEnd

Section "Uninstall"
  SetShellVarContext current
un_check:
  ; 这里必须放在卸载 section 中：un.onInit 运行时卸载器的路径尚未初始化。
  System::Call 'kernel32::OpenMutexW(i 0x00100000, i 0, w "${BUDDY_MUTEX}") p .r0'
  IntCmp $0 0 un_file_check
  System::Call 'kernel32::CloseHandle(p r0)'
  Goto un_busy
un_file_check:
  ; $INSTDIR 在卸载 section 中已初始化为原安装目录。
  IfFileExists "$INSTDIR\buddy.exe" 0 un_continue
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\buddy.exe", i 0x40000000, i 0, p 0, i 3, i 0, p 0) p .r0'
  IntCmp $0 -1 un_busy
  System::Call 'kernel32::CloseHandle(p r0)'
  Goto un_continue
un_busy:
  IfSilent un_cancelled
  MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Buddy 正在运行，无法卸载。请先从托盘退出 Buddy，然后点击“重试”。" /SD IDCANCEL IDRETRY un_check IDCANCEL un_cancelled
un_cancelled:
  SetErrorLevel 32
  Abort
un_continue:
  ; 只移除 Run 中确实指向当前安装目录的 Buddy 值，避免影响其他安装或程序。
  ReadRegStr $0 HKCU "${RUN_KEY}" "${APP_ID}"
  ${UnStrStr} $1 $0 "$INSTDIR\buddy.exe"
  ${If} $1 != ""
    DeleteRegValue HKCU "${RUN_KEY}" "${APP_ID}"
  ${EndIf}

  Delete "$DESKTOP\${APP_NAME}.lnk"
  Delete "${START_MENU_DIR}\${APP_NAME}.lnk"
  Delete "${START_MENU_DIR}\卸载 ${APP_NAME}.lnk"
  RMDir "${START_MENU_DIR}"

  Delete "$INSTDIR\Uninstall ${APP_NAME}.exe"
  Delete "$INSTDIR\buddy.exe"
  Delete "$INSTDIR\VCRUNTIME140.dll"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\LICENSE-GPL-3.0-or-later"
  Delete "$INSTDIR\LICENSE-APACHE-2.0"
  Delete "$INSTDIR\THIRD_PARTY_NOTICES.md"
  Delete "$INSTDIR\icon.ico"
  RMDir "$INSTDIR"
  DeleteRegKey HKCU "${UNINSTALL_KEY}"
SectionEnd
