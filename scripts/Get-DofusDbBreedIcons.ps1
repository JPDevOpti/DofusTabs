[CmdletBinding()]
param(
    [Parameter()]
    [string]$OutputDirectory = (Join-Path $PSScriptRoot "..\DofusTabs\Resources\Icons\DofusDB"),

    [Parameter()]
    [ValidateRange(1, 1000)]
    [int]$MaxBreedId = 200,

    [Parameter()]
    [ValidateSet("fr", "en")]
    [string]$NameLanguage = "en",

    [Parameter()]
    [ValidateRange(5, 120)]
    [int]$TimeoutSec = 25,

    [Parameter()]
    [switch]$KeepSymbolFileName,

    [Parameter()]
    [switch]$CleanOutput
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$apiBaseUrl = "https://api.dofusdb.fr"

function Get-HttpStatusCode {
    param([Parameter(Mandatory = $true)]$Exception)

    if ($null -eq $Exception.Response) {
        return $null
    }

    try {
        return [int]$Exception.Response.StatusCode
    }
    catch {
        return $null
    }
}

function ConvertTo-SafeFileName {
    param([Parameter(Mandatory = $true)][string]$Value)

    if ([string]::IsNullOrWhiteSpace($Value)) {
        return $null
    }

    $decomposed = $Value.Normalize([Text.NormalizationForm]::FormD)
    $sb = New-Object Text.StringBuilder

    foreach ($ch in $decomposed.ToCharArray()) {
        $category = [Globalization.CharUnicodeInfo]::GetUnicodeCategory($ch)
        if ($category -eq [Globalization.UnicodeCategory]::NonSpacingMark) {
            continue
        }

        if ([char]::IsLetterOrDigit($ch)) {
            [void]$sb.Append($ch)
            continue
        }

        [void]$sb.Append('_')
    }

    $normalized = [regex]::Replace($sb.ToString(), "_+", "_").Trim('_')
    if ([string]::IsNullOrWhiteSpace($normalized)) {
        return $null
    }

    return $normalized
}

function Get-LocalizedText {
    param(
        [Parameter(Mandatory = $false)]$Value,
        [Parameter(Mandatory = $true)][string]$Language
    )

    if ($null -eq $Value) {
        return $null
    }

    if ($Value -is [string]) {
        return $Value
    }

    if ($Value -is [System.Collections.IDictionary]) {
        if ($Value.Contains($Language)) {
            return [string]$Value[$Language]
        }

        foreach ($fallback in @("en", "fr", "es", "pt", "de")) {
            if ($Value.Contains($fallback)) {
                return [string]$Value[$fallback]
            }
        }

        return $null
    }

    $languageProperty = $Value.PSObject.Properties[$Language]
    if ($null -ne $languageProperty -and -not [string]::IsNullOrWhiteSpace([string]$languageProperty.Value)) {
        return [string]$languageProperty.Value
    }

    foreach ($fallback in @("en", "fr", "es", "pt", "de")) {
        $fallbackProperty = $Value.PSObject.Properties[$fallback]
        if ($null -ne $fallbackProperty -and -not [string]::IsNullOrWhiteSpace([string]$fallbackProperty.Value)) {
            return [string]$fallbackProperty.Value
        }
    }

    return $null
}

if ($CleanOutput -and (Test-Path -LiteralPath $OutputDirectory)) {
    Remove-Item -LiteralPath $OutputDirectory -Recurse -Force
}

New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null

$results = New-Object System.Collections.Generic.List[object]
$downloaded = 0
$missing = 0
$errors = 0

Write-Host "Downloading DofusDB breed icons..."
Write-Host "Output directory: $OutputDirectory"

for ($id = 1; $id -le $MaxBreedId; $id++) {
    $breedUrl = "$apiBaseUrl/breeds/$id"
    $breed = $null

    try {
        $breed = Invoke-RestMethod -Uri $breedUrl -Method Get -TimeoutSec $TimeoutSec
    }
    catch {
        $statusCode = Get-HttpStatusCode -Exception $_.Exception
        if ($statusCode -eq 404) {
            $missing++
            continue
        }

        $errors++
        Write-Warning "Skipping breed id=$id (metadata error: $($_.Exception.Message))"
        continue
    }

    $classNameFr = Get-LocalizedText -Value $breed.shortName -Language "fr"
    $classNameEn = Get-LocalizedText -Value $breed.shortName -Language "en"

    if ([string]::IsNullOrWhiteSpace($classNameFr) -or $classNameFr -eq "BreedData") {
        $classNameFr = Get-LocalizedText -Value $breed.className -Language "fr"
    }

    if ([string]::IsNullOrWhiteSpace($classNameEn) -or $classNameEn -eq "BreedData") {
        $classNameEn = Get-LocalizedText -Value $breed.className -Language "en"
    }

    $preferredName = if ($NameLanguage -eq "fr") { $classNameFr } else { $classNameEn }
    if ([string]::IsNullOrWhiteSpace($preferredName)) {
        $preferredName = if (-not [string]::IsNullOrWhiteSpace($classNameEn)) { $classNameEn } else { $classNameFr }
    }

    $safeName = if ([string]::IsNullOrWhiteSpace($preferredName)) { $null } else { ConvertTo-SafeFileName -Value $preferredName }

    if ($KeepSymbolFileName) {
        $fileName = "symbol_$id.png"
    }
    elseif (-not [string]::IsNullOrWhiteSpace($safeName)) {
        $fileName = "{0:D3}_{1}.png" -f $id, $safeName
    }
    else {
        $fileName = "symbol_$id.png"
    }

    $iconUrl = "$apiBaseUrl/img/breeds/symbol_$id.png"
    $outputFile = Join-Path $OutputDirectory $fileName

    try {
        Invoke-WebRequest -Uri $iconUrl -Method Get -UseBasicParsing -OutFile $outputFile -TimeoutSec $TimeoutSec
        $downloaded++
        Write-Host ("[{0:D3}] {1}" -f $id, $fileName)

        $results.Add([PSCustomObject]@{
            id = $id
            file = $fileName
            classNameFr = $classNameFr
            classNameEn = $classNameEn
            metadataUrl = $breedUrl
            iconUrl = $iconUrl
        }) | Out-Null
    }
    catch {
        $statusCode = Get-HttpStatusCode -Exception $_.Exception
        if ($statusCode -eq 404) {
            $missing++
            continue
        }

        $errors++
        Write-Warning "Could not download icon for breed id=$id ($($_.Exception.Message))"
    }
}

$manifestPath = Join-Path $OutputDirectory "dofusdb-breeds-manifest.json"
$results | Sort-Object id | ConvertTo-Json -Depth 4 | Out-File -LiteralPath $manifestPath -Encoding UTF8

$attributionPath = Join-Path $OutputDirectory "ATTRIBUTION.txt"
@(
    "Data sourced from DofusDB. Use subject to NCPUL-AI 1.0.",
    "Donnees issues de DofusDB. Utilisation soumise a la LPNC-IA 1.0.",
    "Source endpoint pattern: https://api.dofusdb.fr/img/breeds/symbol_{id}.png"
) | Out-File -LiteralPath $attributionPath -Encoding UTF8

Write-Host ""
Write-Host "Done."
Write-Host "Downloaded: $downloaded"
Write-Host "Missing/404: $missing"
Write-Host "Errors: $errors"
Write-Host "Manifest: $manifestPath"
Write-Host "Attribution: $attributionPath"
