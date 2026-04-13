// Copyright © 2026 Cyberus Technology GmbH
//
// SPDX-License-Identifier: Apache-2.0
//

// build.rs generates a CpuProfiles enum with a variant for each
// CPU profile in arch/x86_64/cpu_profiles and also has the default
// host variant as well.
//
// Furthermore the build script also generates the functions
// `compressed_cpuid_data`, `compressed_msr_data` for obtaining the
// compressed JSON data associated with the given cpu profile.
include!(concat!(env!("OUT_DIR"), "/generated_cpu_profiles.rs"));
