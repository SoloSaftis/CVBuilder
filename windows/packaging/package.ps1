# Builds the Windows packages for CVBuilder 2:
#   windows\dist\CVBuilder-<version>-windows-x64-portable.zip   (unzip and run; data stays next to the exe)
#   windows\dist\CVBuilder-<version>-x64.msix                   (MSIX installer)
# Signing: pass -Pfx/-PfxPassword for a real code-signing certificate. Without one, a throwaway
# self-signed test certificate is made; that MSIX installs only where the test certificate is trusted.
param([string]$Pfx = "", [string]$PfxPassword = "", [string]$Publisher = "CN=CVBuilder Test")
$ErrorActionPreference = "Stop"
$root = Resolve-Path "$PSScriptRoot\..\.."
Set-Location $root
$version = (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$msixVersion = (($version -split '-')[0]) + ".0"
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe | Where-Object { $_.DirectoryName -like "*\x64" } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found: install the Windows 10/11 SDK" }
$kit = $makeappx.Directory
cargo build -p cvbuilder-windows --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$dist = Join-Path $root "windows\dist"; New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path $root "target\release\CVBuilder.exe"

# ---- portable zip ----
$name = "CVBuilder-$version-windows-x64-portable"
$stage = Join-Path $dist $name
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\CVBuilder-data" | Out-Null
Copy-Item $exe $stage
Copy-Item "$PSScriptRoot\LES-MEG.txt" "$stage\LES MEG.txt"
Set-Content "$stage\CVBuilder-data\Om denne mappen.txt" "Her lagrer CVBuilder utkast og innstillinger i bærbar modus. API-nøkler lagres ikke her." -Encoding UTF8
node "$root\shared\packaging\third-party.cjs" "$stage\THIRD-PARTY.txt"
Compress-Archive -Path "$stage\*" -DestinationPath "$dist\$name.zip" -Force
Write-Output "Portable: $dist\$name.zip"

# ---- MSIX ----
$msixStage = Join-Path $dist "msix-stage"
if (Test-Path $msixStage) { Remove-Item -Recurse -Force $msixStage }
New-Item -ItemType Directory -Force "$msixStage\Assets" | Out-Null
Copy-Item $exe $msixStage
Copy-Item "$stage\THIRD-PARTY.txt" $msixStage
$assets = Join-Path $root "shared\assets"
Copy-Item "$assets\cvbuilder-150.png" "$msixStage\Assets\Square150x150Logo.png"
Copy-Item "$assets\cvbuilder-44.png" "$msixStage\Assets\Square44x44Logo.png"
Copy-Item "$assets\cvbuilder-50.png" "$msixStage\Assets\StoreLogo.png"
(Get-Content "$PSScriptRoot\AppxManifest.xml" -Raw).Replace("__VERSION__", $msixVersion).Replace("__PUBLISHER__", $Publisher) | Set-Content "$msixStage\AppxManifest.xml" -Encoding UTF8
$msix = Join-Path $dist "CVBuilder-$version-x64.msix"
& "$($kit.FullName)\makeappx.exe" pack /o /d $msixStage /p $msix | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
if ($Pfx -eq "") {
  # Throwaway test certificate: made, exported next to the package, then removed from the store.
  $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName "CVBuilder test signing" -CertStoreLocation "Cert:\CurrentUser\My" -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
  $PfxPassword = [guid]::NewGuid().ToString()
  $Pfx = Join-Path $dist "msix-test-signing.pfx"
  Export-PfxCertificate -Cert $cert -FilePath $Pfx -Password (ConvertTo-SecureString $PfxPassword -AsPlainText -Force) | Out-Null
  Export-Certificate -Cert $cert -FilePath (Join-Path $dist "CVBuilder-test-certificate.cer") | Out-Null
  Remove-Item "Cert:\CurrentUser\My\$($cert.Thumbprint)"
  Write-Output "Signed with a TEST certificate. To install on a test PC, first trust CVBuilder-test-certificate.cer (Trusted People)."
}
& "$($kit.FullName)\signtool.exe" sign /fd SHA256 /f $Pfx /p $PfxPassword $msix | Out-Null
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
Remove-Item -Recurse -Force $msixStage
Write-Output "MSIX: $msix"
if ($Pfx -like "*msix-test-signing.pfx") { Remove-Item -Force $Pfx }  # its password was never stored
