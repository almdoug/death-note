; Instalador do Death Note (Inno Setup 6)
;
; Compilar:  iscc /DAppVersion=0.2.0 installer\death-note.iss
; Saída:     dist\DeathNote-Setup-<versão>.exe
;
; Instala por usuário (sem pedir administrador) em %LOCALAPPDATA%\Programs,
; o que permite ao app se atualizar sozinho sem prompt do UAC.
;
; Parâmetro extra /RELAUNCH=1: reabre o app ao terminar (usado pelo atualizador).

#ifndef AppVersion
  #define AppVersion "0.0.0"
#endif
#define AppName "Death Note"
#define AppExe "death-note.exe"
#define ProgId "DeathNote.Document"
#define RepoUrl "https://github.com/almdoug/death-note"

[Setup]
; Não altere o AppId: é ele que liga as atualizações à instalação existente.
AppId={{8F3A2C71-5B4E-4D8A-9C1F-2E7B6D5A4C30}
AppName={#AppName}
AppVersion={#AppVersion}
AppVerName={#AppName} {#AppVersion}
AppPublisher=almdoug
AppPublisherURL={#RepoUrl}
AppSupportURL={#RepoUrl}/issues
AppUpdatesURL={#RepoUrl}/releases
VersionInfoVersion={#AppVersion}
DefaultDirName={autopf}\{#AppName}
DefaultGroupName={#AppName}
DisableProgramGroupPage=yes
DisableDirPage=auto
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir=..\dist
OutputBaseFilename=DeathNote-Setup-{#AppVersion}
SetupIconFile=..\assets\icon.ico
UninstallDisplayIcon={app}\{#AppExe}
UninstallDisplayName={#AppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=force
RestartApplications=no
ChangesAssociations=yes

[Languages]
Name: "brazilianportuguese"; MessagesFile: "compiler:Languages\BrazilianPortuguese.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "contextmenu"; Description: "Adicionar ""Abrir com Death Note"" ao menu de contexto dos arquivos"; GroupDescription: "Integração com o Windows:"
Name: "assocdnote"; Description: "Abrir arquivos .dnote com o Death Note"; GroupDescription: "Integração com o Windows:"

[Files]
Source: "..\target\release\{#AppExe}"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
; Atalho no Menu Iniciar (pesquisável) e, opcionalmente, na área de trabalho.
Name: "{autoprograms}\{#AppName}"; Filename: "{app}\{#AppExe}"; Comment: "Bloco de notas em abas"
Name: "{autodesktop}\{#AppName}"; Filename: "{app}\{#AppExe}"; Tasks: desktopicon

[Registry]
; Tipo de documento do Death Note
Root: HKA; Subkey: "Software\Classes\{#ProgId}"; ValueType: string; ValueName: ""; ValueData: "Anotação do Death Note"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\{#ProgId}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"
Root: HKA; Subkey: "Software\Classes\{#ProgId}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""

; .dnote abre no Death Note (opcional)
Root: HKA; Subkey: "Software\Classes\.dnote"; ValueType: string; ValueName: ""; ValueData: "{#ProgId}"; Flags: uninsdeletevalue; Tasks: assocdnote
Root: HKA; Subkey: "Software\Classes\.dnote\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue

; Aparece em "Abrir com" para texto e Markdown, sem mudar o programa padrão
Root: HKA; Subkey: "Software\Classes\.txt\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.md\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.markdown\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\.log\OpenWithProgids"; ValueType: string; ValueName: "{#ProgId}"; ValueData: ""; Flags: uninsdeletevalue

; Registro do aplicativo (nome amigável na lista "Abrir com")
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}"; ValueType: string; ValueName: "FriendlyAppName"; ValueData: "{#AppName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe},0"
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".dnote"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".txt"; ValueData: ""
Root: HKA; Subkey: "Software\Classes\Applications\{#AppExe}\SupportedTypes"; ValueType: string; ValueName: ".md"; ValueData: ""

; "Abrir com Death Note" no menu de contexto de qualquer arquivo (opcional)
Root: HKA; Subkey: "Software\Classes\*\shell\DeathNote"; ValueType: string; ValueName: ""; ValueData: "Abrir com Death Note"; Flags: uninsdeletekey; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\DeathNote"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\{#AppExe},0"; Tasks: contextmenu
Root: HKA; Subkey: "Software\Classes\*\shell\DeathNote\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#AppExe}"" ""%1"""; Tasks: contextmenu

; Permite abrir pelo Executar (Win+R) digitando "dn"
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\dn.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\{#AppExe}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\dn.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"
; Remove o comando antigo "dnote" (v0.2.0 e v0.3.0)
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\dnote.exe"; ValueType: none; Flags: deletekey

[Run]
Filename: "{app}\{#AppExe}"; Description: "{cm:LaunchProgram,{#AppName}}"; Flags: nowait postinstall skipifsilent
; Atualização silenciosa feita pelo próprio app: reabre ao terminar.
Filename: "{app}\{#AppExe}"; Flags: nowait; Check: ShouldRelaunch

[Code]
function ShouldRelaunch: Boolean;
begin
  Result := ExpandConstant('{param:RELAUNCH|0}') = '1';
end;
