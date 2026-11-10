' Show startup progress for desktop launches; keep login startup quiet.
Option Explicit
Dim shell, files, launcher, powershell, command, windowStyle
Set shell = CreateObject("WScript.Shell")
Set files = CreateObject("Scripting.FileSystemObject")
launcher = files.BuildPath(files.GetParentFolderName(WScript.ScriptFullName), "launch.ps1")
powershell = shell.ExpandEnvironmentStrings("%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe")
command = """" & powershell & """ -NoProfile -ExecutionPolicy Bypass -File """ & launcher & """"
windowStyle = 1
If WScript.Arguments.Count > 0 Then
    If WScript.Arguments(0) = "--background" Then
        command = command & " -Background"
        windowStyle = 0
    End If
    If WScript.Arguments(0) = "--quick" Then
        launcher = files.BuildPath(files.GetParentFolderName(WScript.ScriptFullName), "launch-quick.ps1")
        command = """" & powershell & """ -NoProfile -ExecutionPolicy Bypass -File """ & launcher & """"
        windowStyle = 0
    End If
End If
shell.Run command, windowStyle, False
