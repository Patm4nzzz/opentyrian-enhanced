# PSScriptAnalyzer for rewrite/scripts/*.ps1 (run by rewrite/scripts/lint.sh if pwsh is installed).
@{
    Severity     = @('Error', 'Warning')
    ExcludeRules = @(
        # Intended console output of the interactive scripts: Write-Output would put it
        # in the pipeline and mix it with the returned values.
        'PSAvoidUsingWriteHost',
        # New-CleanStartInfo builds a ProcessStartInfo object; it changes
        # no state, ShouldProcess makes no sense.
        'PSUseShouldProcessForStateChangingFunctions'
    )
}
