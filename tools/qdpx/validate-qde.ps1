# Checks a project.qde (the XML inside a .qdpx file) against the REFI-QDA project schema.
#
#   powershell -ExecutionPolicy Bypass -File tools\qdpx\validate-qde.ps1 -Xml path\to\project.qde
#
# To get project.qde out of a .qdpx file, unzip it (a .qdpx file is a ZIP). The test suite keeps a file for this when the environment
# variable PRISMQDA_KEEP_QDPX is set to a path:
#
#   $env:PRISMQDA_KEEP_QDPX = 'C:\Temp\check.qdpx'; cargo test -p prismqda-export --test qdpx a_project_written
#
# It uses the XML validator that comes with Windows PowerShell, so nothing needs to be installed. Prints VALID, or INVALID and the first
# problems with their line numbers.
param(
    [Parameter(Mandatory = $true)][string]$Xml,
    [string]$Xsd = ''
)

if ($Xsd -eq '') { $Xsd = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) 'Project.xsd' }

$schemas = New-Object System.Xml.Schema.XmlSchemaSet
$sr = [System.Xml.XmlReader]::Create($Xsd)
[void]$schemas.Add('urn:QDA-XML:project:1.0', $sr)
$sr.Close()
$schemas.Compile()

$script:errors = New-Object System.Collections.Generic.List[string]
$settings = New-Object System.Xml.XmlReaderSettings
$settings.ValidationType = [System.Xml.ValidationType]::Schema
$settings.Schemas = $schemas
$settings.ValidationFlags = [System.Xml.Schema.XmlSchemaValidationFlags]::ReportValidationWarnings
$settings.add_ValidationEventHandler({ param($s, $e) $script:errors.Add(("{0}: line {1}: {2}" -f $e.Severity, $e.Exception.LineNumber, $e.Message)) })

$reader = [System.Xml.XmlReader]::Create($Xml, $settings)
try { while ($reader.Read()) { } } catch { $script:errors.Add("not well-formed: " + $_.Exception.Message) }
$reader.Close()

if ($script:errors.Count -eq 0) { 'VALID' } else { "INVALID ($($script:errors.Count))"; $script:errors | Select-Object -First 15 }
