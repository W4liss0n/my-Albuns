function Measure-VerifiedProof([object] $Requirement) {
    $requiredText = [string] $Requirement.requiredText
    switch ([string] $Requirement.proofKind) {
        'rust-test' {
            $resultPattern = '(?m)^test\s+(?:[A-Za-z0-9_]+::)*' +
                [regex]::Escape($requiredText) +
                '\s+\.\.\.\s+(?<status>ok|ignored|FAILED)\s*$'
            $results = [regex]::Matches(
                [string] $Requirement.sourceText,
                $resultPattern
            )
            if ($results.Count -ne 1 -or
                    $results[0].Groups['status'].Value -ne 'ok') {
                return 0
            }
            return 1
        }
        'frontend-test' {
            $results = @(
                $Requirement.sourceData |
                    Where-Object {
                        [string]::Equals(
                            [string] $_.title,
                            $requiredText,
                            [System.StringComparison]::Ordinal
                        )
                    }
            )
            if ($results.Count -ne 1 -or
                    [string] $results[0].status -ne 'passed') {
                return 0
            }
            return 1
        }
        'exact-line' {
            return @(
                ([string] $Requirement.sourceText) -split "`r?`n" |
                    Where-Object {
                        [string]::Equals(
                            $_.Trim(),
                            $requiredText,
                            [System.StringComparison]::Ordinal
                        )
                    }
            ).Count
        }
        default {
            throw "Unknown issue 45 proof kind '$($Requirement.proofKind)'."
        }
    }
}

function New-VerifiedCriterion {
    param(
        [Parameter(Mandatory = $true)]
        [string] $Name,

        [Parameter(Mandatory = $true)]
        [object[]] $Requirements
    )

    $proofs = [System.Collections.Generic.List[object]]::new()
    $assertionCount = 0
    foreach ($requirement in @($Requirements)) {
        $requiredText = [string] $requirement.requiredText
        $matchCount = Measure-VerifiedProof -Requirement $requirement
        if ($matchCount -ne 1) {
            throw "Criterion '$Name' has no single successful named proof '$requiredText' in '$($requirement.source)'."
        }
        $proofs.Add([ordered]@{
            source = [string] $requirement.source
            name = $requiredText
            matchCount = $matchCount
        })
        $assertionCount += 1
    }
    $passed = $proofs.Count -eq @($Requirements).Count -and
        $assertionCount -eq $proofs.Count -and
        @($proofs | Where-Object { $_.matchCount -ne 1 }).Count -eq 0
    if (-not $passed) {
        throw "Criterion '$Name' did not retain every required named proof."
    }
    return [ordered]@{
        name = $Name
        passed = [bool] $passed
        assertionCount = $assertionCount
        proofs = @($proofs.ToArray())
    }
}

function Test-ProofParserContracts {
    $passedFrontend = [pscustomobject]@{ title = 'frontend proof'; status = 'passed' }
    $pendingFrontend = [pscustomobject]@{ title = 'frontend proof'; status = 'pending' }
    $cases = @(
        @{
            expected = 1
            requirement = @{
                proofKind = 'rust-test'
                sourceText = 'test module::rust_proof ... ok'
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'rust-test'
                sourceText = 'test module::rust_proof ... ignored'
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'rust-test'
                sourceText = "test a::rust_proof ... ok`ntest b::rust_proof ... ok"
                requiredText = 'rust_proof'
            }
        },
        @{
            expected = 1
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($passedFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($pendingFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'frontend-test'
                sourceData = @($passedFrontend, $passedFrontend)
                requiredText = 'frontend proof'
            }
        },
        @{
            expected = 1
            requirement = @{
                proofKind = 'exact-line'
                sourceText = "other`nexact proof"
                requiredText = 'exact proof'
            }
        },
        @{
            expected = 0
            requirement = @{
                proofKind = 'exact-line'
                sourceText = 'prefix exact proof suffix'
                requiredText = 'exact proof'
            }
        }
    )
    foreach ($case in $cases) {
        $actual = Measure-VerifiedProof -Requirement $case.requirement
        if ($actual -ne $case.expected) {
            throw "The fail-closed proof parser accepted or rejected the wrong fixture: expected=$($case.expected), actual=$actual."
        }
    }
    return $cases.Count
}

$expectedImagingRecoveryCheckNames = @(
    'protocol'
    'cache-temporary-cleanup'
    'imaging-sidecar-build'
    'production-recovery-integration'
    'cache-webview-canvas-export-journey'
    'obsolete-cache-cancellation-integration'
    'causal-cache-pause-integration'
    'actual-tauri-webview2-build'
    'actual-tauri-album-canvas-pixi-webview2'
)

$expectedWindowsPathCheckNames = @(
    'path-contract'
    'path-policy'
    'real-mapped-unc'
    'imaging-protocol'
    'imaging-sidecar-build'
    'sidecar-protocol-preflight'
    'desktop-host-build'
    'path-io-thread'
    'real-sidecar-frozen-plan'
    'desktop-long-path-manifest'
    'sidecar-long-path-manifest'
)

function Test-ExactPassedCheckSet(
    [object[]] $Checks,
    [string[]] $ExpectedNames
) {
    $actual = @($Checks)
    if ($actual.Count -ne @($ExpectedNames).Count) {
        return $false
    }
    $expected = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($name in @($ExpectedNames)) {
        if (-not $expected.Add([string] $name)) {
            throw "The expected check set duplicates '$name'."
        }
    }
    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($check in $actual) {
        if ($null -eq $check `
                -or $null -eq $check.PSObject.Properties['name'] `
                -or $null -eq $check.PSObject.Properties['passed'] `
                -or $check.name -isnot [string] `
                -or $check.passed -isnot [System.Boolean]) {
            return $false
        }
        $name = $check.name
        if ($check.passed -ne $true `
                -or [string]::IsNullOrWhiteSpace($name) `
                -or -not $expected.Contains($name) `
                -or -not $seen.Add($name)) {
            return $false
        }
    }
    return $seen.Count -eq $expected.Count
}

function Test-ExactPassedCheckSetContracts([string[]] $ExpectedNames) {
    $valid = @(
        $ExpectedNames | ForEach-Object {
            [pscustomobject]@{ name = $_; passed = $true }
        }
    )
    if (-not (Test-ExactPassedCheckSet -Checks $valid -ExpectedNames $ExpectedNames)) {
        throw 'The exact check validator rejected its complete passing fixture.'
    }
    $assertionCount = 1
    for ($removed = 0; $removed -lt $valid.Count; $removed++) {
        $fixture = @(
            for ($index = 0; $index -lt $valid.Count; $index++) {
                if ($index -ne $removed) { $valid[$index] }
            }
        )
        if (Test-ExactPassedCheckSet -Checks $fixture -ExpectedNames $ExpectedNames) {
            throw "The check validator accepted a fixture without '$($valid[$removed].name)'."
        }
        $assertionCount += 1
    }
    $duplicate = @(
        for ($index = 0; $index -lt $valid.Count; $index++) {
            if ($index -eq ($valid.Count - 1)) { $valid[0] } else { $valid[$index] }
        }
    )
    if (Test-ExactPassedCheckSet -Checks $duplicate -ExpectedNames $ExpectedNames) {
        throw 'The check validator accepted a duplicate in place of a required check.'
    }
    $assertionCount += 1
    foreach ($invalidPassed in @($false, 'false', 1, $null)) {
        $invalid = @(
            for ($index = 0; $index -lt $valid.Count; $index++) {
                [pscustomobject]@{
                    name = $valid[$index].name
                    passed = if ($index -eq 0) { $invalidPassed } else { $true }
                }
            }
        )
        if (Test-ExactPassedCheckSet -Checks $invalid -ExpectedNames $ExpectedNames) {
            $type = if ($null -eq $invalidPassed) {
                'null'
            }
            else {
                $invalidPassed.GetType().FullName
            }
            throw "The check validator accepted a non-true Boolean value of type '$type'."
        }
        $assertionCount += 1
    }
    $missingPassed = @(
        for ($index = 0; $index -lt $valid.Count; $index++) {
            if ($index -eq 0) {
                [pscustomobject]@{ name = $valid[$index].name }
            }
            else {
                $valid[$index]
            }
        }
    )
    if (Test-ExactPassedCheckSet -Checks $missingPassed -ExpectedNames $ExpectedNames) {
        throw 'The check validator accepted a required check without a passed property.'
    }
    return $assertionCount + 1
}

function Test-ExactFalseBoolean([object] $Value) {
    return $Value -is [System.Boolean] -and $Value -eq $false
}

function ConvertFrom-DesignMatrix([string] $Markdown) {
    $section = [regex]::Match(
        $Markdown,
        '(?ms)^## Matriz do design 0010\s*\r?\n(?<body>.*?)(?=^##\s|\z)'
    )
    if (-not $section.Success) {
        throw 'The issue 45 research has no design 0010 matrix section.'
    }
    $rows = [System.Collections.Generic.List[object]]::new()
    foreach ($line in @($section.Groups['body'].Value -split "`r?`n")) {
        $match = [regex]::Match(
            $line,
            '^\|(?<scenario>[^|]+)\|(?<producer>[^|]+)\|(?<effect>[^|]+)\|(?<proof>[^|]+)\|\s*$'
        )
        if (-not $match.Success) {
            continue
        }
        $scenario = $match.Groups['scenario'].Value.Trim()
        $rawProof = $match.Groups['proof'].Value.Trim()
        $proofMatch = [regex]::Match(
            $rawProof,
            '^`(?<proof>[A-Za-z0-9_-]+)`$'
        )
        if (-not $proofMatch.Success) {
            continue
        }
        $proof = $proofMatch.Groups['proof'].Value
        $rows.Add([pscustomobject]@{
            scenario = $scenario
            producer = $match.Groups['producer'].Value.Trim()
            consumerEffect = $match.Groups['effect'].Value.Trim()
            proof = $proof
            key = "$scenario => $proof"
        })
    }
    return @($rows.ToArray())
}

function Get-NormativeDesignScenarios([string] $Markdown) {
    $blocks = [System.Collections.Generic.List[object]]::new()
    $collecting = $false
    $current = [System.Collections.Generic.List[string]]::new()
    foreach ($line in @($Markdown -split "`r?`n")) {
        $row = [regex]::Match(
            $line,
            '^\|(?<scenario>[^|]+)\|(?<result>[^|]+)\|\s*$'
        )
        if (-not $row.Success) {
            if ($collecting -and $current.Count -ne 0) {
                $blocks.Add(@($current.ToArray()))
            }
            $collecting = $false
            $current.Clear()
            continue
        }
        $scenario = $row.Groups['scenario'].Value.Trim()
        $result = $row.Groups['result'].Value.Trim()
        if ($scenario -match '^-+$' -and $result -match '^-+$') {
            $collecting = $true
            $current.Clear()
            continue
        }
        if ($collecting) {
            $current.Add($scenario)
        }
    }
    if ($collecting -and $current.Count -ne 0) {
        $blocks.Add(@($current.ToArray()))
    }
    $normative = @($blocks | Where-Object { @($_).Count -eq 14 })
    if ($normative.Count -ne 1) {
        throw 'The normative design 0010 must contain exactly one 14-row two-column scenario matrix.'
    }
    return @($normative[0])
}

function Test-DesignMatrixCoverage(
    [object[]] $Rows,
    [object[]] $Expected
) {
    if (@($Rows).Count -ne @($Expected).Count) {
        return $false
    }
    $expectedByScenario = [System.Collections.Generic.Dictionary[string, string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($entry in @($Expected)) {
        if ($expectedByScenario.ContainsKey([string] $entry.scenario)) {
            throw "The expected design matrix duplicates '$($entry.scenario)'."
        }
        $expectedByScenario.Add(
            [string] $entry.scenario,
            [string] $entry.proof
        )
    }
    $seen = [System.Collections.Generic.HashSet[string]]::new(
        [System.StringComparer]::Ordinal
    )
    foreach ($row in @($Rows)) {
        $scenario = [string] $row.scenario
        if (-not $seen.Add($scenario) -or
                [string]::IsNullOrWhiteSpace([string] $row.producer) -or
                [string]::IsNullOrWhiteSpace([string] $row.consumerEffect) -or
                [string]::Equals(
                    [string] $row.producer,
                    [string] $row.consumerEffect,
                    [System.StringComparison]::Ordinal
                ) -or
                -not $expectedByScenario.ContainsKey($scenario) -or
                -not [string]::Equals(
                    [string] $row.proof,
                    $expectedByScenario[$scenario],
                    [System.StringComparison]::Ordinal
                )) {
            return $false
        }
    }
    return $seen.Count -eq $expectedByScenario.Count
}

function Test-DesignMatrixContracts(
    [object[]] $Rows,
    [object[]] $Expected
) {
    if (-not (Test-DesignMatrixCoverage -Rows $Rows -Expected $Expected)) {
        throw 'The design 0010 matrix is missing, duplicated, extra, or mapped to the wrong proof.'
    }
    $assertionCount = 1
    for ($removed = 0; $removed -lt @($Rows).Count; $removed++) {
        $fixture = @(
            for ($index = 0; $index -lt @($Rows).Count; $index++) {
                if ($index -ne $removed) { $Rows[$index] }
            }
        )
        if (Test-DesignMatrixCoverage -Rows $fixture -Expected $Expected) {
            throw 'The design matrix validator accepted a fixture with one normative row removed.'
        }
        $assertionCount += 1
    }
    return $assertionCount
}

$script:Issue45GateProofSources = $null

function Set-Issue45GateProofSources {
    param(
        [Parameter(Mandatory = $true)]
        [string] $RustText,

        [Parameter(Mandatory = $true)]
        [object[]] $FrontendData
    )

    $script:Issue45GateProofSources = [pscustomobject]@{
        RustText = $RustText
        FrontendData = @($FrontendData)
    }
}

function Get-Issue45GateProofSources {
    if ($null -eq $script:Issue45GateProofSources) {
        throw 'The issue 45 proof sources were not initialized.'
    }
    return $script:Issue45GateProofSources
}

function New-RustProof([string] $Name) {
    $sources = Get-Issue45GateProofSources
    return @{
        source = 'rust-tests'
        proofKind = 'rust-test'
        sourceText = $sources.RustText
        requiredText = $Name
    }
}

function New-FrontendProof([string] $Name) {
    $sources = Get-Issue45GateProofSources
    return @{
        source = 'frontend-tests'
        proofKind = 'frontend-test'
        sourceData = $sources.FrontendData
        requiredText = $Name
    }
}

function New-ExactProof(
    [string] $Source,
    [string] $Text,
    [string] $Name
) {
    return @{
        source = $Source
        proofKind = 'exact-line'
        sourceText = $Text
        requiredText = $Name
    }
}

function New-Issue45VerifiedCriteria {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)]
        [string] $RustText,

        [Parameter(Mandatory = $true)]
        [object[]] $FrontendData,

        [Parameter(Mandatory = $true)]
        [object[]] $DesignMatrixRows,

        [Parameter(Mandatory = $true)]
        [string] $ImagingProofText,

        [Parameter(Mandatory = $true)]
        [string] $WindowsProofText,

        [Parameter(Mandatory = $true)]
        [string] $NarrowApiProofText
    )

    try {
        $designMatrixProofText = @(
            $designMatrixRows | ForEach-Object { $_.key }
        ) -join "`n"

        Set-Issue45GateProofSources `
            -RustText $RustText `
            -FrontendData $FrontendData
        $completeMatrixRequirements = [System.Collections.Generic.List[object]]::new()
        foreach ($row in $designMatrixRows) {
            $completeMatrixRequirements.Add(
                (New-ExactProof `
                    -Source 'research-matrix' `
                    -Text $designMatrixProofText `
                    -Name $row.key)
            )
        }
        foreach ($proofName in @(
                'cache_consumes_authoritative_identity_transitions_without_owning_them'
                'a_new_authorized_identity_reserves_an_independent_empty_namespace'
                'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes'
                'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state'
                'corrupted_or_incompatible_index_is_discarded_and_rebuilt'
                'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation'
                'reopening_after_host_death_recovers_the_contained_processors_temporary'
                'project_open_during_free_space_is_serialized_by_namespace_reservation'
                'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup'
                'export_plan_rejects_missing_originals_at_the_typed_plan_stage'
            )) {
            $completeMatrixRequirements.Add((New-RustProof -Name $proofName))
        }
        $completeMatrixRequirements.Add(
            (New-ExactProof `
                -Source 'windows-paths' `
                -Text $windowsProofText `
                -Name 'real-mapped-unc')
        )
        $criteria = @(
            New-VerifiedCriterion `
                -Name 'authorized-independent-empty-namespace' `
                -Requirements @(
                    (New-RustProof -Name 'a_new_authorized_identity_reserves_an_independent_empty_namespace')
                    (New-RustProof -Name 'cache_consumes_authoritative_identity_transitions_without_owning_them')
                )
            New-VerifiedCriterion `
                -Name 'authoritative-absent-unavailable-and-visual-context' `
                -Requirements @(
                    (New-RustProof -Name 'resolver_monitor_and_runtime_keep_observed_state_outside_media_refs')
                    (New-RustProof -Name 'public_host_runtime_retry_reinspects_without_mutating_media_ref_or_project')
                    (New-RustProof -Name 'explicit_retry_rejects_absent_occurrences_without_changing_runtime')
                    (New-RustProof -Name 'explicit_retry_changes_unavailable_to_absent_after_authoritative_inspection')
                    (New-RustProof -Name 'explicit_retry_can_establish_the_first_observation_for_provisional_unavailability')
                    (New-RustProof -Name 'explicit_retry_preserves_unavailable_when_the_new_context_still_cannot_access_the_root')
                    (New-RustProof -Name 'explicit_retry_keeps_unavailable_when_cache_adoption_fails_and_can_be_retried')
                    (New-RustProof -Name 'authoritative_media_availability_maps_exhaustively_without_cache_failures')
                    (New-RustProof -Name 'registry_publication_failure_becomes_cache_unavailable_without_source_retry')
                    (New-RustProof -Name 'absent_or_unavailable_media_preserves_the_last_known_preview_with_its_typed_state')
                    (New-FrontendProof -Name 'offers retry only for an unavailable occurrence and keeps Relink exclusive to absent')
                    (New-FrontendProof -Name 'retries an unavailable occurrence explicitly and refreshes it without Relink')
                    (New-FrontendProof -Name 'keeps retry actionable after an unavailable-media IPC failure without mutating Project')
                    (New-FrontendProof -Name 'replaces unavailable retry with a cache-only failure after authoritative refresh')
                    (New-FrontendProof -Name 'registers the media-change listener before the first preview demand')
                    (New-FrontendProof -Name 'normalizes typed unavailable-media retry failures at the IPC adapter')
                    (New-FrontendProof -Name 'keeps the last known preview when linked media becomes unavailable')
                    (New-FrontendProof -Name 'keeps the last representation only as visual context when the Original is absent')
                    (New-RustProof -Name 'export_plan_rejects_missing_originals_at_the_typed_plan_stage')
                )
            New-VerifiedCriterion `
                -Name 'relink-occurrence-stable-change-and-reappearance' `
                -Requirements @(
                    (New-RustProof -Name 'public_relink_command_updates_only_the_selected_occurrence_and_participates_in_history')
                    (New-RustProof -Name 'public_relink_flow_reinspects_and_invalidates_only_the_selected_occurrence')
                    (New-RustProof -Name 'monitor_consolidates_rapid_observations_and_invalidates_only_stable_content_changes')
                    (New-FrontendProof -Name 'offers retry only for an unavailable occurrence and keeps Relink exclusive to absent')
                )
            New-VerifiedCriterion `
                -Name 'discardable-complete-index-and-candidate-validation' `
                -Requirements @(
                    (New-RustProof -Name 'corrupted_or_incompatible_index_is_discarded_and_rebuilt')
                    (New-RustProof -Name 'duplicate_media_entries_make_the_discardable_cache_index_non_current')
                    (New-RustProof -Name 'cache_engine_publishes_index_last_reuses_and_invalidates_only_the_requested_media')
                    (New-RustProof -Name 'failed_validation_discards_the_candidate_and_preserves_the_last_published_generation')
                    (New-RustProof -Name 'a_wrong_response_correlation_discards_the_unpublished_candidate_generation')
                    (New-RustProof -Name 'repeated_crashes_after_candidate_publication_leave_no_orphan_generation')
                )
            New-VerifiedCriterion `
                -Name 'request-fingerprint-variant-revalidation-and-obsolete-collection' `
                -Requirements @(
                    (New-RustProof -Name 'terminal_fingerprint_reopens_the_frozen_path_after_atomic_replacement')
                    (New-RustProof -Name 'authoritative_demand_revision_rejects_queued_and_late_obsolete_work')
                    (New-RustProof -Name 'obsolete_job_that_finishes_does_not_publish_and_discards_its_candidate_generation')
                    (New-RustProof -Name 'failed_validation_discards_the_candidate_and_preserves_the_last_published_generation')
                    (New-RustProof -Name 'held_cache_storage_creates_and_drops_temporaries_in_the_physical_namespace')
                    (New-RustProof -Name 'held_cache_storage_removes_only_the_physical_file_after_a_junction_swap')
                    (New-RustProof -Name 'held_cache_storage_replaces_only_the_physical_file_after_a_junction_swap')
                    (New-RustProof -Name 'recursive_cleanup_mutates_only_the_held_directory_after_a_junction_swap')
                )
            New-VerifiedCriterion `
                -Name 'processor-restart-once-then-nonblocking-suspension' `
                -Requirements @(
                    (New-RustProof -Name 'repeated_processor_crashes_suspend_new_cache_work_after_one_restart')
                    (New-RustProof -Name 'repeated_processor_failure_suspends_before_fallible_recovery_cleanup')
                    (New-FrontendProof -Name 'shows a non-blocking warning when repeated processor failures suspend Cache')
                    (New-FrontendProof -Name 'registers the Cache warning listener before the first preview demand')
                )
            New-VerifiedCriterion `
                -Name 'tracer-44-host-death-and-real-recovery' `
                -Requirements @(
                    (New-RustProof -Name 'terminating_the_host_closes_its_job_and_terminates_the_active_processor')
                    (New-RustProof -Name 'attach_rejects_a_recycled_pid_identity_without_containing_or_killing_the_observed_process')
                    (New-RustProof -Name 'guarded_writer_claim_storage_publishes_reads_and_conditionally_removes_by_handle')
                    (New-RustProof -Name 'writer_wait_finishes_in_the_held_namespace_and_preserves_external_claim_files')
                    (New-RustProof -Name 'fragmented_handshake_preserves_the_exact_process_instance')
                    (New-RustProof -Name 'processor_handshake_reports_the_exact_instance_seen_through_the_spawned_child_handle')
                    (New-RustProof -Name 'reopening_after_host_death_recovers_the_contained_processors_temporary')
                    (New-RustProof -Name 'free_closed_projects_after_host_death_waits_before_measuring_and_removing')
                    (New-RustProof -Name 'clear_all_after_host_death_waits_before_measuring_and_removing')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'protocol')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'cache-temporary-cleanup')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'imaging-sidecar-build')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'production-recovery-integration')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'cache-webview-canvas-export-journey')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'obsolete-cache-cancellation-integration')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'causal-cache-pause-integration')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'actual-tauri-webview2-build')
                    (New-ExactProof -Source 'imaging-recovery' -Text $imagingProofText -Name 'actual-tauri-album-canvas-pixi-webview2')
                )
            New-VerifiedCriterion `
                -Name 'local-unc-mapped-and-long-paths' `
                -Requirements @(
                    (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'path-contract')
                    (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'path-policy')
                    (New-ExactProof -Source 'windows-paths' -Text $windowsProofText -Name 'real-mapped-unc')
                    (New-RustProof -Name 'prepares_the_cache_only_as_directories_below_the_authorized_root')
                )
            New-VerifiedCriterion `
                -Name 'measure-free-reserve-and-safe-total-cleanup' `
                -Requirements @(
                    (New-RustProof -Name 'measures_and_frees_only_namespaces_without_an_active_owner')
                    (New-RustProof -Name 'schedules_total_cleanup_while_a_project_is_active_and_runs_it_at_safe_startup')
                    (New-RustProof -Name 'project_open_during_free_space_is_serialized_by_namespace_reservation')
                    (New-RustProof -Name 'active_namespace_measurement_tolerates_real_writer_promotion_and_exclusive_files')
                    (New-RustProof -Name 'scheduled_cleanup_keeps_the_runtime_responsive_until_the_exact_writer_exits')
                    (New-RustProof -Name 'reopening_after_host_death_recovers_the_contained_processors_temporary')
                    (New-RustProof -Name 'free_closed_projects_quiesces_writers_before_measuring_removed_bytes')
                    (New-RustProof -Name 'clear_all_quiesces_writers_before_measuring_removed_bytes')
                    (New-RustProof -Name 'free_closed_projects_after_host_death_waits_before_measuring_and_removing')
                    (New-RustProof -Name 'clear_all_after_host_death_waits_before_measuring_and_removing')
                    (New-RustProof -Name 'reserved_namespace_recovery_discards_abandoned_files_and_preserves_indexed_generation')
                    (New-RustProof -Name 'active_namespace_with_different_windows_casing_is_never_releasable')
                    (New-RustProof -Name 'active_cache_root_with_different_windows_casing_shares_every_reservation')
                    (New-RustProof -Name 'scheduled_cleanup_never_reads_or_deletes_a_marker_through_a_state_junction')
                )
            New-VerifiedCriterion `
                -Name 'narrow-api-without-universal-coordinator' `
                -Requirements @(
                    (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'cache_service_status')
                    (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'free_closed_project_cache')
                    (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'clear_all_cache')
                    (New-ExactProof -Source 'cache-service-source' -Text $narrowApiProofText -Name 'no-bulk-cache-namespace-inspection')
                    (New-FrontendProof -Name 'maps the Project and media ports to the desktop commands')
                    (New-FrontendProof -Name 'initializes the native dialog used by the productive relink command')
                    (New-RustProof -Name 'global_cache_service_commands_are_explicitly_allowed_only_to_global_window')
                )
            New-VerifiedCriterion `
                -Name 'complete-design-0010-producer-consumer-matrix' `
                -Requirements @($completeMatrixRequirements.ToArray())
        )
        if ($criteria.Count -ne 11 -or @($criteria | Where-Object { -not $_.passed }).Count -ne 0) {
            throw 'The issue 45 criteria matrix is incomplete or contains an unproved criterion.'
        }
        return @($criteria)
    }
    finally {
        $script:Issue45GateProofSources = $null
    }
}
