/*									*\
	NSIS Generic Uninstaller 1.42
		;Scripted by AgnoMan Sen
	Works with NSIS 1.8-2.0(+)
\*									*/
 
!ifndef EASYUNINSTALLER_c0nf16x000001
!verbose Push
!verbose 3
	!macro EASYUNINSTALLER_funcM3x000001
		!ifndef EASYUNINSTALLER_c0nf16x000001
			!define EASYUNINSTALLER_c0nf16x000001
		!endif
		!ifndef EASYUNINSTALLER_c0nf16x000002
			!define EASYUNINSTALLER_c0nf16x000002
		!endif
		!ifdef EASYUNINSTALLER_c0nf16x000003
			!undef EASYUNINSTALLER_c0nf16x000003
		!endif
		!include EasyUninstall.nsh
	!macroend
	!macro EASYUNINSTALLER_funcM3x000002
		!ifndef EASYUNINSTALLER_c0nf16x000001
			!define EASYUNINSTALLER_c0nf16x000001
		!endif
		!ifdef EASYUNINSTALLER_c0nf16x000002
			!undef EASYUNINSTALLER_c0nf16x000002
		!endif
		!ifndef EASYUNINSTALLER_c0nf16x000003
			!define EASYUNINSTALLER_c0nf16x000003
		!endif
		!include EasyUninstall.nsh
	!macroend
 
	!define CONFIGUREUNINSTALL "!insertmacro EASYUNINSTALLER_funcM3x000001"
 
	!define IncludeUninstaller "!insertmacro EASYUNINSTALLER_funcM0x000000"
	!define IncludeUninstallerSection "!insertmacro EASYUNINSTALLER_funcM3x000002"
 
	!define UninstallerRootKey "!define _UninstallerRootKey"
	!define DONOTCONFIRMUNINSTALL "!define _DONOTCONFIRMUNINSTALL"
	!define DONOTINCLUDEUNINSTALLPAGE "!define _DONOTINCLUDEUNINSTALLPAGE"
 
	!define SILENTUNINSTALL "!define _SilentUninst"
	!define RemoveOnly "!define _RemoveOnly"
	!define ProductID "!define _ProductID"
	!define RegOwner "!define _RegOwner"
	!define RegCompany "!define _RegCompany"
	!define HelpLink "!define _HelpLink"
	!define HelpTelephone "!define _HelpTelephone"
	!define URLUpdateInfo "!define _URLUpdateInfo"
	!define URLInfoAbout "!define _URLInfoAbout"
	!define DisplayName "!define _DisplayName"
	!define DisplayIcon "!define _DisplayIcon"
	!define DpIconIndex "!define _DpIconIndex"
	!define DisplayVersion "!define _DisplayVersion"
	!define ModifyPath "!define _ModifyPath"
	!define VersionMajor "!define _VersionMajor"
	!define VersionMinor "!define _VersionMinor"
	!define EstimatedSize "!define _EstimatedSize"
	!define Comments "!define _Comments"
	!define InstallSource "!define _InstallSource"
	!define ParentKeyName "!define _ParentKeyName"
	!define ParentDisplayName "!define _ParentDisplayName"
!verbose Pop
!endif
 
!ifdef EASYUNINSTALLER_c0nf16x000002
 
	!ifdef MUI_INCLUDED
		!ifndef _DONOTCONFIRMUNINSTALL
			!insertmacro MUI_UNPAGE_CONFIRM
		!endif
		!ifndef _DONOTINCLUDEUNINSTALLPAGE		
			!insertmacro MUI_UNPAGE_INSTFILES
		!endif
	!endif
 
 
	!ifndef _RemoveOnly
		!define _RemoveOnly 1
	!endif
 
	!macro EASYUNINSTALLER_funcM0x000001
		!verbose Push
		!verbose 3
			!ifndef _UninstallerRootKey
				!define RR01_Root  HKLM
			!else
				!define RR01_Root ${_UninstallerRootKey}
			!endif
			!ifndef RR01_Key
				!define RR01_Key "SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall"
			!endif
			!ifndef _ProductID
				!define RR01_Name "$(^Name)"
			!else
				!define RR01_Name "${_ProductID}"
			!endif
			!ifndef _DisplayName
				!define RR01_DisplayName "${RR01_Name}"
			!else
				!define RR01_DisplayName "${_DisplayName}"
			!endif
			!ifdef _DisplayIcon
				!ifdef _DpIconIndex
					!define RR01_DisplayIcon "${_DisplayIcon},${_DpIconIndex}"
				!else
					!define RR01_DisplayIcon "${_DisplayIcon}"
				!endif
			!endif
	!verbose Pop
	!macroend
 
	!macro EASYUNINSTALLER_funcM0x000002
		!ifdef RR01_Name
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "ProductID" "${RR01_Name}"
		!endif
		!ifdef _RegOwner
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "RegOwner" "${_RegOwner}"
		!endif
		!ifdef _RegCompany
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "RegCompany" "${_RegCompany}"
		!endif
		!ifdef _HelpLink
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "HelpLink" "${_HelpLink}"
		!endif
		!ifdef _HelpTelephone
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "HelpTelephone" "${_HelpTelephone}"
		!endif
		!ifdef RR01_DisplayName
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "DisplayName" "${RR01_DisplayName}"
		!endif
		!ifdef _URLUpdateInfo
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "URLUpdateInfo" "${_URLUpdateInfo}"
		!endif
		!ifdef _URLInfoAbout
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "URLInfoAbout" "${_URLInfoAbout}"
		!endif
		!ifndef _SilentUninst
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "UninstallString" "$\"$INSTDIR\uninstall.exe$\""
		!else
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "QuietUninstallString" "$\"$INSTDIR\uninstall.exe$\" /S"
		!endif
		!ifdef RR01_DisplayIcon
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "DisplayIcon" "${RR01_DisplayIcon}"
		!endif
		!ifdef _DisplayVersion
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "DisplayVersion" "${_DisplayVersion}"
		!endif
		!ifdef _RemoveOnly
			StrCmp ${_RemoveOnly} 1 +1 RR01RS7SK1
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoModify" "0x00000001"
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoRepair" "0x00000001"
				Goto RR01RS7
		RR01RS7SK1:
			StrCmp ${_RemoveOnly} 0 +1 RR01RS7
			!ifdef _ModifyPath
				WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "ModifyPath" "${_ModifyPath}"
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoModify" "0x00000000"
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoRepair" "0x00000001"
			!else
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoModify" "0x00000000"
				WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "NoRepair" "0x00000000"
			!endif
		RR01RS7:
		!endif
		!ifdef _VersionMajor
			WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "VersionMajor" "${_VersionMajor}"
		!endif
		!ifdef _VersionMinor
			WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "VersionMinor" "${_VersionMinor}"
		!endif
		!ifdef _EstimatedSize
			WriteRegDWORD ${RR01_Root} "${RR01_Key}\${RR01_Name}" "EstimatedSize" "${_EstimatedSize}"
		!endif
		!ifdef _Comments
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "Comments" "${_Comments}"
		!endif
		WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "InstallLocation" "$INSTDIR"
		!ifdef _InstallSource
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "InstallSource" "${_InstallSource}"
		!endif
		!ifdef _ParentKeyName
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "ParentKeyName" "${_ParentKeyName}"
		!endif
		!ifdef _ParentDisplayName
			WriteRegStr ${RR01_Root} "${RR01_Key}\${RR01_Name}" "ParentDisplayName" "${_ParentDisplayName}"
		!endif
	!macroend
 
	!macro EASYUNINSTALLER_funcM0x000000
		WriteUninstaller "$INSTDIR\uninstall.exe"
		!insertmacro EASYUNINSTALLER_funcM0x000001
		!insertmacro EASYUNINSTALLER_funcM0x000002
	!macroend
 
!endif
 
!ifdef EASYUNINSTALLER_c0nf16x000003
 
	Var RR01
	!macro EASYUNINSTALLER_funcM1x000001
		!verbose Push
		!verbose 4
			ReadRegStr $RR01 ${RR01_Root} "${RR01_Key}\${RR01_Name}" "InstallLocation"
				StrCmp $RR01 "A:\Program Files" RR01ER00 +1
				StrCmp $RR01 "B:\Program Files" RR01ER00 +1
				StrCmp $RR01 "C:\Program Files" RR01ER00 +1
				StrCmp $RR01 "D:\Program Files" RR01ER00 +1
				StrCmp $RR01 "E:\Program Files" RR01ER00 +1
				StrCmp $RR01 "F:\Program Files" RR01ER00 +1
				StrCmp $RR01 "G:\Program Files" RR01ER00 +1
				StrCmp $RR01 "H:\Program Files" RR01ER00 +1
				StrCmp $RR01 "I:\Program Files" RR01ER00 +1
				StrCmp $RR01 "J:\Program Files" RR01ER00 +1
				StrCmp $RR01 "K:\Program Files" RR01ER00 +1
				StrCmp $RR01 "L:\Program Files" RR01ER00 +1
				StrCmp $RR01 "M:\Program Files" RR01ER00 +1
				StrCmp $RR01 "N:\Program Files" RR01ER00 +1
				StrCmp $RR01 "O:\Program Files" RR01ER00 +1
				StrCmp $RR01 "P:\Program Files" RR01ER00 +1
				StrCmp $RR01 "Q:\Program Files" RR01ER00 +1
				StrCmp $RR01 "R:\Program Files" RR01ER00 +1
				StrCmp $RR01 "S:\Program Files" RR01ER00 +1
				StrCmp $RR01 "T:\Program Files" RR01ER00 +1
				StrCmp $RR01 "U:\Program Files" RR01ER00 +1
				StrCmp $RR01 "V:\Program Files" RR01ER00 +1
				StrCmp $RR01 "W:\Program Files" RR01ER00 +1
				StrCmp $RR01 "X:\Program Files" RR01ER00 +1
				StrCmp $RR01 "Y:\Program Files" RR01ER00 +1
				StrCmp $RR01 "Z:\Program Files" RR01ER00 RR01ER00SK
	RR01ER00:
		HideWindow
		MessageBox MB_OK|MB_ICONSTOP \
		"!Error: (RR01FNC0)$\n$\n$\tThe program has been installed to '$RR01' only!$\n$\tCan not remove, otherwise system will be corrupt.$\n$\tYou can still remove the program manually.$\n$\t$\t-Sorry for the inconvienence."
		Quit
	RR01ER00SK:
		!verbose Pop
	!macroend
 
	!macro EASYUNINSTALLER_funcM1x000002
		DeleteRegKey ${RR01_Root} "${RR01_Key}\${RR01_Name}"
	!macroend
 
	Function un.onUninstSuccess
		HideWindow
		MessageBox MB_OK \
		"Uninstallation complete.  Have a nice day!" \
		IDOK +1
	FunctionEnd
 
	Section Uninstall
		!insertmacro EASYUNINSTALLER_funcM1x000001
		Delete "$INSTDIR\uninstall.exe"
		!insertmacro EASYUNINSTALLER_funcM1x000002
		RMDir /r "$INSTDIR"
		RMDir /r "$INSTDIR"
		SetAutoClose true
	SectionEnd
 
!endif