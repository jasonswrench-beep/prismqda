# Runs a program with its processor affinity set to the efficiency cores only (CPUs 16 to 31). The performance cores of the i9-13900K in
# this computer crash plain Python, so Python and Rust test programs are started through this.
#   powershell -ExecutionPolicy Bypass -File pin.ps1 <program> [arguments]
$ErrorActionPreference = "Stop"
(Get-Process -Id $PID).ProcessorAffinity = [IntPtr]::new(4294901760)
$exe = $args[0]
$rest = @()
if ($args.Count -gt 1) { $rest = $args[1..($args.Count - 1)] }
& $exe @rest
exit $LASTEXITCODE
