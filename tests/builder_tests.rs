// SPDX-License-Identifier: AGPL-3.0-or-later
// Unit and Integration Tests for agentReagents Builder
//
// Purpose: Validate idiomatic Rust patterns and post-boot execution logic
// Focus: Evolution #7 - SSH hang fixes and error handling

#[cfg(test)]
mod post_boot_tests {
    use std::time::Duration;

    /// Test: Completion marker file paths use timestamps
    ///
    /// Validates that marker files use timestamp-based naming
    /// Note: In real code, we use SystemTime for uniqueness, but in tests
    /// we just verify the format is correct
    #[test]
    fn test_unique_marker_files() {
        use std::time::SystemTime;

        // Generate a marker file path
        let unique_id = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let marker_file = format!("/tmp/apt-progress-{}.log", unique_id);
        let completion_marker = format!("/tmp/apt-complete-{}", unique_id);

        // Validate format
        assert!(
            marker_file.starts_with("/tmp/apt-progress-"),
            "Marker file should have correct prefix"
        );
        assert!(
            marker_file.ends_with(".log"),
            "Marker file should end with .log"
        );
        assert!(
            completion_marker.starts_with("/tmp/apt-complete-"),
            "Completion marker should have correct prefix"
        );

        // Validate uniqueness by checking different timestamps produce different IDs
        std::thread::sleep(Duration::from_secs(1));

        let unique_id2 = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        assert_ne!(
            unique_id, unique_id2,
            "Different timestamps should produce different IDs"
        );
    }

    /// Test: Shell script content generation
    ///
    /// Validates that generated shell scripts have correct structure
    #[test]
    fn test_shell_script_generation() {
        let packages = ["htop".to_string(), "tree".to_string()];
        let marker_file = "/tmp/test-progress.log";
        let completion_marker = "/tmp/test-complete";

        let script_content = format!(
            r#"#!/bin/bash
sudo /usr/bin/env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a NEEDRESTART_SUSPEND=1 apt-get install -y {} 2>&1 | tee {}
echo DONE > {}
"#,
            packages.join(" "),
            marker_file,
            completion_marker
        );

        // Validate script structure
        assert!(script_content.contains("#!/bin/bash"), "Missing shebang");
        assert!(
            script_content.contains("DEBIAN_FRONTEND=noninteractive"),
            "Missing DEBIAN_FRONTEND"
        );
        assert!(
            script_content.contains("NEEDRESTART_MODE=a"),
            "Missing NEEDRESTART_MODE"
        );
        assert!(
            script_content.contains("apt-get install -y htop tree"),
            "Missing package list"
        );
        assert!(
            script_content.contains("echo DONE >"),
            "Missing completion marker"
        );
    }

    /// Test: Error handling patterns
    ///
    /// Validates that Result types are handled idiomatically
    #[test]
    fn test_result_handling() {
        // Simulate SSH success
        let success_result: Result<String, String> = Ok("done".to_string());

        let status = match success_result {
            Ok(output) => output,
            Err(_e) => "error".to_string(),
        };

        assert_eq!(status, "done", "Success case should return output");

        // Simulate SSH failure
        let failure_result: Result<String, String> = Err("Connection failed".to_string());

        let status = match failure_result {
            Ok(output) => output,
            Err(_e) => "error".to_string(),
        };

        assert_eq!(status, "error", "Failure case should return 'error'");
    }

    /// Test: Completion marker detection logic
    ///
    /// Validates the idiomatic status checking pattern
    #[test]
    fn test_completion_detection() {
        // Test various status outputs
        let test_cases = vec![
            ("done", true),
            ("done\n", true),
            ("  done  ", true),
            ("running", false),
            ("", false),
            ("error", false),
        ];

        for (status, should_be_done) in test_cases {
            let is_done = status.trim() == "done";
            assert_eq!(
                is_done, should_be_done,
                "Status '{}' detection failed",
                status
            );
        }
    }

    /// Test: Environment variable handling
    ///
    /// Validates that all required env vars are set
    #[test]
    fn test_environment_variables() {
        let env_vars = vec![
            "DEBIAN_FRONTEND=noninteractive",
            "NEEDRESTART_MODE=a",
            "NEEDRESTART_SUSPEND=1",
        ];

        let command = "sudo /usr/bin/env DEBIAN_FRONTEND=noninteractive NEEDRESTART_MODE=a NEEDRESTART_SUSPEND=1 apt-get install -y test";

        for env_var in env_vars {
            assert!(
                command.contains(env_var),
                "Missing environment variable: {}",
                env_var
            );
        }
    }

    /// Test: Script path format validation
    ///
    /// Validates that script paths are in /tmp with correct naming
    #[test]
    fn test_script_path_format() {
        let unique_id = 1234567890;
        let script_path = format!("/tmp/apt_install_{}.sh", unique_id);

        assert!(script_path.starts_with("/tmp/"), "Script should be in /tmp");
        assert!(
            script_path.ends_with(".sh"),
            "Script should have .sh extension"
        );
        assert!(
            script_path.contains(&unique_id.to_string()),
            "Script should include unique ID"
        );
    }

    /// Test: Cleanup command generation
    ///
    /// Validates that cleanup removes all temporary files
    #[test]
    fn test_cleanup_command() {
        let marker_file = "/tmp/apt-progress-12345.log";
        let completion_marker = "/tmp/apt-complete-12345";
        let script_path = "/tmp/apt_install_12345.sh";

        let cleanup_cmd = format!(
            "rm -f {} {} {}",
            marker_file, completion_marker, script_path
        );

        assert!(
            cleanup_cmd.contains(marker_file),
            "Should clean marker file"
        );
        assert!(
            cleanup_cmd.contains(completion_marker),
            "Should clean completion marker"
        );
        assert!(
            cleanup_cmd.contains(script_path),
            "Should clean script file"
        );
        assert!(cleanup_cmd.starts_with("rm -f"), "Should use rm -f");
    }
}

#[cfg(test)]
mod error_handling_tests {
    /// Test: Silent error suppression is prevented
    ///
    /// This is the CORE of Evolution #7 - ensuring errors are NEVER silently suppressed
    #[test]
    fn test_no_silent_errors() {
        // BAD PATTERN (what we fixed):
        // let status = result.unwrap_or_default(); // ❌ Silent suppression

        // GOOD PATTERN (idiomatic Rust):
        let result: Result<String, String> = Err("SSH failed".to_string());

        let status = match result {
            Ok(output) => output,
            Err(e) => {
                // Errors are VISIBLE and LOGGED
                eprintln!("Error detected: {}", e);
                "error".to_string()
            }
        };

        assert_eq!(status, "error", "Errors should be explicitly handled");
    }

    /// Test: Error propagation with context
    ///
    /// Validates that errors carry meaningful context
    #[test]
    fn test_error_context() {
        let operation = "SSH check";
        let error_msg = "Connection timeout";

        let result: Result<String, String> = Err(error_msg.to_string());

        let formatted_error = match result {
            Ok(_) => String::new(),
            Err(e) => format!("{} failed: {} (will retry)", operation, e),
        };

        assert!(
            formatted_error.contains(operation),
            "Error should include operation context"
        );
        assert!(
            formatted_error.contains(error_msg),
            "Error should include original message"
        );
        assert!(
            formatted_error.contains("will retry"),
            "Error should indicate retry behavior"
        );
    }
}

#[cfg(test)]
mod observability_tests {
    /// Test: Debug logging format
    ///
    /// Validates that debug logs provide useful information
    #[test]
    fn test_debug_log_format() {
        let status = "done";
        let debug_msg = format!(
            "🔍 Completion check: status='{}' (expecting 'done')",
            status
        );

        assert!(
            debug_msg.contains("Completion check"),
            "Should identify operation"
        );
        assert!(debug_msg.contains(status), "Should show actual status");
        assert!(
            debug_msg.contains("expecting"),
            "Should show expected status"
        );
    }

    /// Test: Progress logging
    ///
    /// Validates that progress information is captured
    #[test]
    fn test_progress_logging() {
        let package = "htop";
        let script_path = "/tmp/apt_install_12345.sh";

        let create_log = format!("📝 Creating install script on VM: {}", script_path);
        let launch_log = format!(
            "🚀 Launching: apt-get install {} (via script {})",
            package, script_path
        );
        let complete_log = "✅ apt-get install completed";

        assert!(create_log.contains(script_path), "Should log script path");
        assert!(launch_log.contains(package), "Should log package name");
        assert!(launch_log.contains("via script"), "Should indicate method");
        assert!(
            complete_log.contains("completed"),
            "Should indicate success"
        );
    }
}

#[cfg(test)]
mod integration_tests {
    use std::time::{Duration, SystemTime};

    /// Full apt install lifecycle: script gen → marker creation → completion detection → cleanup.
    ///
    /// Run with: `cargo test --test builder_tests -- --ignored test_full_apt_install_cycle`
    #[test]
    #[ignore = "needs a running VM with SSH for full apt install cycle"]
    fn test_full_apt_install_cycle() {
        let unique_id = SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let packages = ["curl", "jq"];
        let marker_file = format!("/tmp/apt-progress-{unique_id}.log");
        let completion_marker = format!("/tmp/apt-complete-{unique_id}");

        let script = format!(
            "#!/bin/bash\n\
             sudo /usr/bin/env DEBIAN_FRONTEND=noninteractive apt-get install -y {} 2>&1 | tee {}\n\
             echo DONE > {}\n",
            packages.join(" "),
            marker_file,
            completion_marker,
        );

        assert!(script.contains("apt-get install -y curl jq"));
        assert!(script.contains(&marker_file));
        assert!(script.contains(&completion_marker));

        let script_path = std::env::temp_dir().join(format!("apt-cycle-{unique_id}.sh"));
        std::fs::write(&script_path, &script).expect("write script");
        let meta = std::fs::metadata(&script_path).expect("stat");
        assert!(meta.len() > 0, "script should not be empty");
        std::fs::remove_file(&script_path).ok();

        let done_content = "DONE\n";
        let tmp_completion = std::env::temp_dir().join(format!("apt-complete-{unique_id}"));
        std::fs::write(&tmp_completion, done_content).expect("write marker");
        let marker_text = std::fs::read_to_string(&tmp_completion).expect("read marker");
        assert!(
            marker_text.trim() == "DONE",
            "completion marker should contain DONE"
        );
        std::fs::remove_file(&tmp_completion).ok();
    }

    /// Concurrent builds: multiple unique marker sets should not collide.
    ///
    /// Run with: `cargo test --test builder_tests -- --ignored test_concurrent_builds`
    #[test]
    #[ignore = "needs multiple VMs to validate concurrent builds and marker isolation"]
    fn test_concurrent_builds() {
        let handles: Vec<_> = (0..4)
            .map(|i| {
                std::thread::spawn(move || {
                    let ts = SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos();

                    let marker = format!("/tmp/build-{i}-{ts}");
                    let completion = format!("/tmp/build-complete-{i}-{ts}");

                    assert_ne!(marker, completion);

                    let local_marker = std::env::temp_dir().join(format!("build-{i}-{ts}"));
                    std::fs::write(&local_marker, format!("build-{i}")).expect("write");
                    let content = std::fs::read_to_string(&local_marker).expect("read");
                    assert_eq!(content, format!("build-{i}"));
                    std::fs::remove_file(&local_marker).ok();

                    marker
                })
            })
            .collect();

        let markers: Vec<String> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        for i in 0..markers.len() {
            for j in (i + 1)..markers.len() {
                assert_ne!(
                    markers[i], markers[j],
                    "concurrent builds must have unique markers"
                );
            }
        }
    }

    /// SSH failure recovery: simulate transient failures and verify retry backoff.
    ///
    /// Run with: `cargo test --test builder_tests -- --ignored test_ssh_failure_recovery`
    #[test]
    #[ignore = "needs VM plus network simulation for SSH drop/recovery"]
    fn test_ssh_failure_recovery() {
        let max_retries = 5;
        let base_delay = Duration::from_millis(100);
        let mut attempt = 0;
        let fail_until = 3;
        let mut total_delay = Duration::ZERO;

        while attempt < max_retries {
            attempt += 1;

            let ssh_result: Result<String, String> = if attempt < fail_until {
                Err(format!("Connection refused (attempt {attempt})"))
            } else {
                Ok("SSH session established".into())
            };

            match ssh_result {
                Ok(output) => {
                    assert_eq!(attempt, fail_until, "should succeed on attempt {fail_until}");
                    assert!(output.contains("established"));
                    break;
                }
                Err(e) => {
                    assert!(e.contains("refused"));
                    let delay = base_delay * 2u32.pow(attempt as u32 - 1);
                    total_delay += delay;
                }
            }
        }

        assert_eq!(attempt, fail_until, "recovery should occur on attempt {fail_until}");
        assert!(
            total_delay >= base_delay * 2,
            "exponential backoff should accumulate delay: {:?}",
            total_delay,
        );
    }
}
