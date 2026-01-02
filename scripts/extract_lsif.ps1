$in = 'compile_commands.json'
$out_json = 'sources.json'
$out_txt = 'sources.txt'
$uris = @()
Get-Content $in | ForEach-Object {
    $line = $_.Trim()
    if ([string]::IsNullOrWhiteSpace($line)) { return }
    try {
        $obj = $line | ConvertFrom-Json
    } catch {
        return
    }
    if ($obj.type -eq 'vertex' -and $obj.label -eq 'document') {
        $uri = $obj.uri
        if ($uri -like 'file://*') {
            $u = $uri.Substring(7)
            if ($u.StartsWith('/') -and $u.Length -ge 3 -and $u[2] -eq ':') { $u = $u.Substring(1) }
            $path = [System.Uri]::UnescapeDataString($u)
        } else {
            $path = $uri
        }
        $uris += $path
    }
}
$uris | Out-File -Encoding utf8 -FilePath $out_txt
$uris | ConvertTo-Json -Depth 4 | Out-File -Encoding utf8 -FilePath $out_json
Write-Host "written $($uris.Count) sources to $out_json and $out_txt"