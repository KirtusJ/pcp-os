param(
    [Parameter(Mandatory)][string]$Jdk,
    [Parameter(Mandatory)][string]$BuildTools,
    [Parameter(Mandatory)][string]$AndroidJar,
    [Parameter(Mandatory)][string]$Output,
    [Parameter(Mandatory)][string]$Keystore,
    [string]$KeyAlias = 'helpdesk'
)
$ErrorActionPreference = 'Stop'
$env:JAVA_HOME = $Jdk
$env:PATH = "$Jdk/bin;$env:PATH"
function Run($tool, [string[]]$arguments) {
    & $tool @arguments
    if ($LASTEXITCODE) { throw "$tool exited with $LASTEXITCODE" }
}
$build = Join-Path $Output ([guid]::NewGuid().ToString())
$classes = Join-Path $build 'classes'
New-Item -ItemType Directory -Force $classes | Out-Null
Run "$Jdk/bin/javac.exe" @('-encoding','UTF-8','--release','8','-classpath',$AndroidJar,'-d',$classes,"$PSScriptRoot/src/org/pcpos/helpdesk/Desk.java")
Run "$Jdk/bin/jar.exe" @('cf',"$build/app.jar",'-C',$classes,'.')
Run "$BuildTools/d8.bat" @('--min-api','26','--lib',$AndroidJar,'--output',$build,"$build/app.jar")
Run "$BuildTools/aapt2.exe" @('link','-I',$AndroidJar,'--manifest',"$PSScriptRoot/AndroidManifest.xml",'-o',"$build/base.apk")
Push-Location $build
try { Run "$BuildTools/aapt.exe" @('add','base.apk','classes.dex') } finally { Pop-Location }
Run "$BuildTools/zipalign.exe" @('-f','4',"$build/base.apk","$build/aligned.apk")
Run "$BuildTools/apksigner.bat" @('sign','--ks',$Keystore,'--ks-key-alias',$KeyAlias,'--ks-pass','env:HELPDESK_KEYSTORE_PASSWORD','--out',"$build/signed.apk","$build/aligned.apk")
Run "$BuildTools/apksigner.bat" @('verify','--verbose',"$build/signed.apk")
Copy-Item "$build/signed.apk" "$Output/Director-HelpDesk.apk" -Force
