# Call the user's prompt first: prompt frameworks inspect $? and $LASTEXITCODE (D124).
$global:__IvyOriginalPrompt = $function:prompt
function global:prompt {
    $ivyPromptText = & $global:__IvyOriginalPrompt
    $ivyLocation = Get-Location
    $ivyReport = ''
    if ($ivyLocation.Provider.Name -eq 'FileSystem') {
        $ivyPath = $ivyLocation.ProviderPath.Replace('\', '/')
        if ($ivyPath.StartsWith('//')) {
            $ivyParts = $ivyPath.Substring(2).Split('/')
            $ivyHost = $ivyParts[0]
            $ivyTail = @($ivyParts | Select-Object -Skip 1 | ForEach-Object { [uri]::EscapeDataString($_) }) -join '/'
            $ivyReport = "file://$ivyHost/$ivyTail"
        } else {
            $ivyParts = @($ivyPath.Split('/') | ForEach-Object { [uri]::EscapeDataString($_) })
            # A drive colon is URI syntax; a colon in a filename remains escaped.
            if ($ivyPath -match '^[A-Za-z]:/') {
                $ivyParts[0] = $ivyPath.Substring(0, 2)
                $ivyReport = 'file:///' + ($ivyParts -join '/')
            } else {
                $ivyReport = 'file://' + ($ivyParts -join '/')
            }
        }
    }
    [Console]::Write(([string][char]27 + ']7;' + $ivyReport + [char]27 + '\'))
    $ivyPromptText
}

if (Get-Command codex -CommandType Alias,Function -ErrorAction Ignore) {
    [Console]::Error.WriteLine('Ivy: existing codex alias or function preserved; automatic panel routing is inactive.')
} elseif ($env:IVY_CODEX_HELPER) {
    function global:codex {
        $ivyReal = $null
        foreach ($ivyName in 'codex.exe', 'codex.cmd', 'codex.bat') {
            $ivyReal = Get-Command $ivyName -CommandType Application -ErrorAction Ignore | Select-Object -First 1
            if ($ivyReal) { break }
        }
        if (-not $ivyReal) {
            [Console]::Error.WriteLine('Ivy: codex executable not found in PATH.')
            $global:LASTEXITCODE = 127
            return
        }
        if (Test-Path -LiteralPath $env:IVY_CODEX_HELPER -PathType Leaf) {
            & $env:IVY_CODEX_HELPER $ivyReal.Source @args
        } else {
            [Console]::Error.WriteLine('Ivy: codex helper unavailable; automatic panel routing is inactive.')
            & $ivyReal.Source @args
        }
    }
}
