;--------------------------------
;Include Modern UI
  Unicode True
  
  !AddIncludeDir "."
  !include "MUI2.nsh"
  !include "FileFunc.nsh"
  
;  !include EasyUninstall.nsh 
;	${CONFIGUREUNINSTALL}
  
  !define lbm "LittleBigMouse"
  !define lbm_file "LittleBigMouse.Ui.Avalonia.exe"
  
  !define main "..\LittleBigMouse.Ui\LittleBigMouse.Ui.Avalonia"
  !define daemon "..\LittleBigMouse.Daemon"
  !define main_out_dir "${main}\bin\x64\Release\net8.0"
  !define daemon_out_dir "${daemon}\bin\x64\Release"
  
  !getdllversion "${main_out_dir}\${lbm_file}" Expv_
  !define lbm_version "${Expv_1}.${Expv_2}.${Expv_3}.${Expv_4}"
  
  !define uninstall_reg "Software\Microsoft\Windows\CurrentVersion\Uninstall\${lbm}"

;--------------------------------
;General

  ;Name and file
  Name "Little Big Mouse"
  OutFile "${lbm}-${lbm_version}.exe"

  ;Default installation folder
  InstallDir "$PROGRAMFILES64\${lbm}"
  
  ;Get installation folder from registry if available
  InstallDirRegKey HKCU "Software\Mgth\${lbm}" ""

  ;Request application privileges for Windows Vista
  RequestExecutionLevel admin
 
;---------------------------------
;General
 
  !define MUI_ICON "${main}\Assets\lbm-logo.ico"
  !define MUI_UNICON "${main}\Assets\lbm-logo.ico"
;  !define MUI_SPECIALBITMAP "Bitmap.bmp"

;--------------------------------
;Interface Settings

  !define MUI_ABORTWARNING

;--------------------------------
;Pages

;  !insertmacro MUI_PAGE_LICENSE "${NSISDIR}\Docs\Modern UI\License.txt"
;  !insertmacro MUI_PAGE_COMPONENTS
  !insertmacro MUI_PAGE_DIRECTORY
  !insertmacro MUI_PAGE_INSTFILES
  
  !define MUI_FINISHPAGE_RUN "$INSTDIR\${lbm_file}"
  !insertmacro MUI_PAGE_FINISH
  
  !insertmacro MUI_UNPAGE_CONFIRM
  !insertmacro MUI_UNPAGE_INSTFILES
 
 
;--------------------------------
;Language
 
  !insertmacro MUI_LANGUAGE "English"
 
 
;-------------------------------- 
;Installer Sections     
Section "install" 

;Delete Files 
  RMDir /r "$INSTDIR\*.*"    
 
  nsProcess::_KillProcess "${lbm_file}"
  nsProcess::_KillProcess "LittleBigMouse.Hook.exe"
  nsProcess::_KillProcess "LittleBigMouse_Control.exe"
  nsProcess::_KillProcess "LittleBigMouse_Daemon.exe"
 
;Add files
  SetOutPath "$INSTDIR"
 
  File "${main_out_dir}\*.exe"
  File "${daemon_out_dir}\*.exe"
  File /r "${main_out_dir}\*.dll"
  File "${main_out_dir}\*.json"
 
;create desktop shortcut
  CreateShortCut "$DESKTOP\${lbm}.lnk" "$INSTDIR\${lbm_file}" ""
 
;create start-menu items
  CreateDirectory "$SMPROGRAMS\${lbm}"
  CreateShortCut "$SMPROGRAMS\${lbm}\Uninstall.lnk" "$INSTDIR\Uninstall.exe" "" "$INSTDIR\Uninstall.exe" 0
  CreateShortCut "$SMPROGRAMS\${lbm}\${lbm}.lnk" "$INSTDIR\${lbm_file}" "" "$INSTDIR\${lbm_file}" 0
 
;write uninstall information to the registry
  WriteRegStr HKLM "${uninstall_reg}" "DisplayName" "${lbm}"
  WriteRegStr HKLM "${uninstall_reg}" "UninstallString" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKLM "${uninstall_reg}" "DisplayVersion" "${lbm_version}"
  WriteRegStr HKLM "${uninstall_reg}" "DisplayIcon" "$INSTDIR\${lbm_file}"
  WriteRegStr HKLM "${uninstall_reg}" "InstallLocation" "$INSTDIR"
  
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD HKLM "${uninstall_reg}" "EstimatedSize" "$0"

  WriteUninstaller "$INSTDIR\Uninstall.exe"
 
SectionEnd
 
 
;--------------------------------    
;Uninstaller Section  
Section "Uninstall"
  nsProcess::_KillProcess "${lbm_file}"
  nsProcess::_KillProcess "LittleBigMouse.Hook.exe"

Delete Files 
  RMDir /r "$INSTDIR\*.*"    
 
;Remove the installation directory
  RMDir "$INSTDIR"
 
;Delete Start Menu Shortcuts
  Delete "$DESKTOP\${lbm}.lnk"
  Delete "$SMPROGRAMS\${lbm}\*.*"
  RmDir  "$SMPROGRAMS\${lbm}"
 
;Delete Uninstaller And Unistall Registry Entries
;  DeleteRegKey HKEY_LOCAL_MACHINE "SOFTWARE\${lbm}"
  DeleteRegKey HKEY_LOCAL_MACHINE "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall\${lbm}"  
 
SectionEnd
 
;Section CreateUninstaller ;section name is irrelevant
;	${IncludeUninstaller} ;Tells EasyUninstall.nsh to write the uninstaller.
;SectionEnd 

 
;eof