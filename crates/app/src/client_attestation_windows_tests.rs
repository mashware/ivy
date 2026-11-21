mod programs {
    mod tests {
        macro_rules! windows_unix_omission {
            ($name:ident) => {
                #[test]
                #[ignore = "unix-only: exercises Unix native client discovery and process identity"]
                fn $name() {}
            };
        }
        windows_unix_omission!(desktop_path_discovers_native_claude_in_its_user_installation);
        windows_unix_omission!(native_claude_version_symlink_is_discovered_with_colon_in_home);
        windows_unix_omission!(explicit_path_keeps_precedence_over_native_claude_installation);
        windows_unix_omission!(unsupported_path_client_does_not_fall_back_to_another_installation);
        windows_unix_omission!(native_claude_fallback_refuses_scripts_missing_or_relative_home);
        windows_unix_omission!(native_fallback_does_not_discover_other_clients_or_nonexecutables);
        windows_unix_omission!(trusted_path_discards_empty_and_relative_entries);
    }
    mod child {
        mod tests {
            macro_rules! windows_unix_omission {
                    ($name:ident) => {
                        #[test]
                        #[ignore = "unix-only: exercises Unix resolver-child lifetime, process groups and pipe EOF"]
                        fn $name() {}
                    };
                }
            windows_unix_omission!(volta_stdout_and_success_are_collected_under_one_deadline);
            windows_unix_omission!(volta_timeout_does_not_wait_after_kill_and_the_child_is_reaped);
            windows_unix_omission!(
                volta_descendant_holding_stdout_cannot_extend_the_exchange_deadline
            );
            windows_unix_omission!(
                resolver_cleanup_exhaustion_rejects_before_spawning_another_child
            );
        }
    }
}
