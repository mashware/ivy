# These portable checks exercise the launch oracle without substituting for native execution.
$ErrorActionPreference = 'Stop'
$errors = $null
$ast = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'test-launch.ps1'), [ref]$null, [ref]$errors)
if ($errors.Count) { $errors | Format-List; exit 1 }
$type = $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.StringConstantExpressionAst] -and $n.Value.StartsWith('using System;') }, $true)
Add-Type -TypeDefinition $type[0].Value
$publisherErrors = $null
$publisherAst = [System.Management.Automation.Language.Parser]::ParseFile((Join-Path $PSScriptRoot 'publish-provenance-witness.ps1'), [ref]$null, [ref]$publisherErrors)
if ($publisherErrors.Count) { $publisherErrors | Format-List; exit 1 }
$publisherType = $publisherAst.FindAll({ param($n) $n -is [System.Management.Automation.Language.StringConstantExpressionAst] -and $n.Value.StartsWith('using System;') }, $true)
if ($publisherType.Count -ne 1) { throw 'Publication witness native identity binding is missing' }
Add-Type -TypeDefinition $publisherType[0].Value
$function = $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Saved-Leaves' }, $true)
Invoke-Expression $function[0].Extent.Text
$leaf = '{"leaf":{"panel_width":36,"folded":false}}' | ConvertFrom-Json
if ((Saved-Leaves $leaf) -ne 1) { throw 'valid leaf rejected' }
$branch = @{branch=@{orientation='columns';children=@(@{weight=1;node=$leaf}, @{weight=1;node=$leaf})}}
if ((Saved-Leaves $branch) -ne 2) { throw 'valid branch rejected' }
foreach ($invalid in @(@{leaf=@{panel_width=0;folded=$false}}, @{leaf=@{panel_width=36}}, @{branch=@{children=@()}},
    @{branch=@{children=@(@{weight=1;node=$leaf}, @{weight=1;node=$leaf})}},
    @{branch=@{orientation='columns';children=@(@{node=$leaf}, @{weight=1;node=$leaf})}},
    @{branch=@{orientation='columns';children=@(@{weight=0;node=$leaf}, @{weight=1;node=$leaf})}},
    @{branch=@{orientation='columns';children=@(@{weight=1;node=$branch}, @{weight=1;node=$leaf})}})) {
    $failed = $false
    try { Saved-Leaves $invalid } catch { $failed = $true }
    if (-not $failed) { throw 'invalid layout accepted' }
}
$environment = $ast.FindAll({ param($n) $n -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $n.Name -eq 'Set-LaunchEnvironment' }, $true)
Invoke-Expression $environment[0].Extent.Text
foreach ($control in 'none', 'shell') {
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.Environment['PATH'] = 'preserved-path'
    $start.Environment['ComSpec'] = 'preserved-command'
    foreach ($name in 'SHELL', 'PANEL_ID', 'PANEL_DIR', 'XDG_CONFIG_HOME', 'MEASURE_FRAMES', 'IVY_TEST') {
        $start.Environment[$name] = 'remove-me'
    }
    $profile = Join-Path ([IO.Path]::GetTempPath()) 'ivy-launch-contract-profile'
    Set-LaunchEnvironment $start $profile $control
    if ($start.Environment['APPDATA'] -ne $profile) { throw 'APPDATA is not isolated' }
    foreach ($name in 'SHELL', 'PANEL_ID', 'PANEL_DIR', 'XDG_CONFIG_HOME', 'MEASURE_FRAMES', 'IVY_TEST') {
        if ($start.Environment.ContainsKey($name)) { throw "inherited override survived: $name" }
    }
    if ($control -eq 'none') {
        if ($start.Environment['PATH'] -ne 'preserved-path' -or $start.Environment['ComSpec'] -ne 'preserved-command') { throw 'normal shell resolution was changed' }
    } elseif ($start.Environment['PATH'] -ne $profile -or $start.Environment['ComSpec'] -ne (Join-Path $profile 'missing-cmd.exe')) {
        throw 'shell control did not prevent default and fallback shell startup'
    }
}
'PowerShell syntax, C# native interop compilation, saved tree and isolated launch environment fixtures passed.'
