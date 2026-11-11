# Builds the Windows packages for CVBuilder:
#   platforms\windows\dist\CVBuilder-<version>-windows-x64-portable.zip   (unzip and run; data stays next to the exe)
#   platforms\windows\dist\CVBuilder-<version>-x64.msix                   (MSIX installer)
# Signing: pass -Pfx/-PfxPassword for a real code-signing certificate. Without one, a throwaway
# self-signed test certificate is made; that MSIX installs only where the test certificate is trusted.
# Microsoft Store: pass -Store. It uses CVBuilder's Store identity from Partner Center (Product
# identity); -IdentityName, -Publisher "CN=…" and -PublisherDisplayName override it. The MSIX is left unsigned, since
# the Store signs it, and is written as CVBuilder-<version>-x64-store.msix for uploading there.
param([string]$Pfx = "", [string]$PfxPassword = "", [string]$Publisher = "CN=CVBuilder Test",
      [switch]$Store, [string]$IdentityName = "", [string]$PublisherDisplayName = "")
$ErrorActionPreference = "Stop"
# CVBuilder's identity in the Microsoft Store (Partner Center → Product identity; not secret).
if ($Store) {
  if ($IdentityName -eq "") { $IdentityName = "SoloSaftis.CVBuilder" }
  if ($Publisher -eq "CN=CVBuilder Test") { $Publisher = "CN=BE27F9C3-2592-4EE3-87D1-736943301DFA" }
  if ($PublisherDisplayName -eq "") { $PublisherDisplayName = "SoloSaftis" }
} else {
  if ($IdentityName -eq "") { $IdentityName = "CVBuilder.Familie" }
  if ($PublisherDisplayName -eq "") { $PublisherDisplayName = "CVBuilder" }
}
$root = Resolve-Path "$PSScriptRoot\..\..\.."
Set-Location $root
$version = (Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"').Matches[0].Groups[1].Value
$msixVersion = (($version -split '-')[0]) + ".0"
$makeappx = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin" -Recurse -Filter makeappx.exe | Where-Object { $_.DirectoryName -like "*\x64" } | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $makeappx) { throw "makeappx.exe not found: install the Windows 10/11 SDK" }
$kit = $makeappx.Directory
cargo build -p cvbuilder-windows --release --locked
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
$dist = Join-Path $root "platforms\windows\dist"; New-Item -ItemType Directory -Force $dist | Out-Null
$exe = Join-Path $root "target\release\CVBuilder.exe"

# ---- portable zip ----
$name = "CVBuilder-$version-windows-x64-portable"
# The zip is made from a clean folder of its own. The unpacked folder next to it ($run) may be in
# use, with the person's own drafts in CVBuilder-data: only the program files there are replaced.
$stage = Join-Path $dist "pack-$name"
if (Test-Path $stage) { Remove-Item -Recurse -Force $stage }
New-Item -ItemType Directory -Force "$stage\CVBuilder-data" | Out-Null
Copy-Item $exe $stage
Copy-Item "$PSScriptRoot\LES-MEG.txt" "$stage\LES MEG.txt"
Set-Content "$stage\CVBuilder-data\Om denne mappen.txt" "Her lagrer CVBuilder utkast og innstillinger i bærbar modus. API-nøkler lagres ikke her." -Encoding UTF8
node "$root\packaging\third-party.cjs" "$stage\THIRD-PARTY.txt"
Compress-Archive -Path "$stage\*" -DestinationPath "$dist\$name.zip" -Force
$run = Join-Path $dist $name
New-Item -ItemType Directory -Force "$run\CVBuilder-data" | Out-Null
Get-ChildItem $stage -File | Copy-Item -Destination $run -Force
if (-not (Test-Path "$run\CVBuilder-data\Om denne mappen.txt")) { Copy-Item "$stage\CVBuilder-data\Om denne mappen.txt" "$run\CVBuilder-data\" }
Write-Output "Portable: $dist\$name.zip"

# ---- MSIX ----
$msixStage = Join-Path $dist "msix-stage"
if (Test-Path $msixStage) { Remove-Item -Recurse -Force $msixStage }
New-Item -ItemType Directory -Force "$msixStage\Assets" | Out-Null
Copy-Item $exe $msixStage
Copy-Item "$stage\THIRD-PARTY.txt" $msixStage
$assets = Join-Path $root "assets"
Copy-Item "$assets\cvbuilder-150.png" "$msixStage\Assets\Square150x150Logo.png"
Copy-Item "$assets\cvbuilder-44.png" "$msixStage\Assets\Square44x44Logo.png"
Copy-Item "$assets\cvbuilder-50.png" "$msixStage\Assets\StoreLogo.png"
(Get-Content "$PSScriptRoot\AppxManifest.xml" -Raw).Replace("__VERSION__", $msixVersion).Replace("__PUBLISHER__", $Publisher).Replace("__IDENTITY__", $IdentityName).Replace("__PUBLISHER_NAME__", [Security.SecurityElement]::Escape($PublisherDisplayName)) | Set-Content "$msixStage\AppxManifest.xml" -Encoding UTF8
$msix = Join-Path $dist ($(if ($Store) { "CVBuilder-$version-x64-store.msix" } else { "CVBuilder-$version-x64.msix" }))
& "$($kit.FullName)\makeappx.exe" pack /o /d $msixStage /p $msix | Out-Null
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
if ($Store) {
  Remove-Item -Recurse -Force $msixStage
  Write-Output "MSIX for the Microsoft Store (unsigned; the Store signs it): $msix"
  exit 0
}
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
