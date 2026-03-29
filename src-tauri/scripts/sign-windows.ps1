param(
  [Parameter(Mandatory = $true)]
  [string]$BinaryPath
)

$certPath = $env:WINDOWS_CERTIFICATE_PATH
$certPassword = $env:WINDOWS_CERTIFICATE_PASSWORD

if ([string]::IsNullOrWhiteSpace($certPath) -or [string]::IsNullOrWhiteSpace($certPassword)) {
  Write-Host "Signing skipped: WINDOWS_CERTIFICATE_PATH or WINDOWS_CERTIFICATE_PASSWORD is not set."
  exit 0
}

if (-not (Test-Path -Path $certPath)) {
  Write-Error "Signing certificate was not found at '$certPath'."
  exit 1
}

$signtool = Get-Command -Name "signtool.exe" -ErrorAction SilentlyContinue
if ($null -eq $signtool) {
  Write-Error "signtool.exe is not available on PATH."
  exit 1
}

& $signtool.Source sign `
  /fd SHA256 `
  /td SHA256 `
  /tr "http://timestamp.digicert.com" `
  /f "$certPath" `
  /p "$certPassword" `
  "$BinaryPath"

if ($LASTEXITCODE -ne 0) {
  Write-Error "signtool failed with exit code $LASTEXITCODE."
  exit $LASTEXITCODE
}
