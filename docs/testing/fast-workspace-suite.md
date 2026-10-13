# Fast workspace test suite

Owner-requested reduction on 2026-10-12: keep 50 fast, distinct regression tests;
delete the other 186, including the ignored GPU test. Removed tests remain in Git
history, not as disabled tests or a second slow suite.

Selection favors asynchronous asset loading/residency, rendering correctness,
save and editor data integrity, shared flight controls, registry consistency,
and canonical terrain ownership. It deliberately drops broad simulation scenarios,
repeated variations, and diagnostics with narrower value. This reduces coverage
of combat, trading, autopilot, mining, and other removed scenarios.

Run `cargo test -q --workspace`. There are no ignored tests in this suite.

## Selection timings

Each pre-change release test was invoked separately, single-threaded on one CPU,
using the compiled binaries from the latest Studio performance checkpoint.
Timings include process startup and isolated registry initialization; OS caches
were warm and a bounded build ran concurrently. These are selection measurements,
not a controlled end-to-end speedup benchmark. Two removed Studio tests failed
when isolated; the old baseline is therefore not claimed as a fresh full pass.

Total old isolated runtime: 101.487s.
Selected isolated runtime: 8.217s.

| Retained test | Isolated seconds |
|---|---:|
| `analytic_derivative_matches_independent_owner_face_goldens` | 0.0031 |
| `app::preparation_tests::promotion_runs_update_before_first_game_draw` | 0.0006 |
| `app::preparation_tests::publishes_only_complete_factory_and_keeps_errors` | 0.0007 |
| `asset_prediction::tests::swept_lateral_and_receding` | 0.0007 |
| `assets::mounts::tests::ancestor_frames_are_applied_once_and_duplicates_refused` | 0.0010 |
| `assets::tests::concurrent_demand_coalesces_and_ready_requests_hit` | 0.0026 |
| `assets::tests::failed_and_oversize_results_are_not_ready_hits` | 0.0026 |
| `assets::tests::fitting_job_runs_while_large_job_waits_for_a_lease` | 0.0026 |
| `assets::tests::leases_prevent_eviction_and_release_is_counted_once` | 0.0428 |
| `assets::tests::packages_are_bounded` | 0.0017 |
| `assets::tests::priority_and_cancellation_control_queued_work` | 0.0006 |
| `assets::tests::scene_cancel_only_removes_queued_speculation` | 0.0006 |
| `content::tests::the_base_pack_loads_and_its_hulls_are_sound` | 1.0109 |
| `equipment_visual::tests::authored_alpha_survives_clipping_and_placed_transforms` | 0.0014 |
| `equipment_visual::tests::pending_stand_promotes_and_world_uses_same_model` | 0.0286 |
| `equipment_visual::tests::streamed_projection_matches_reference_at_near_plane` | 0.0008 |
| `every_layer_round_trips_yaml_and_json` | 0.0590 |
| `every_shader_module_parses_and_validates` | 0.0117 |
| `fine_oblique_edges_preserve_vertex_ownership` | 0.0014 |
| `fire_control::tests::a_slug_fired_along_the_lead_meets_the_target` | 0.0007 |
| `independent_water_domain_goldens` | 0.0012 |
| `integrate::tests::same_inputs_same_world` | 0.0021 |
| `interior::ship_code::tests::apply_undo_restores_whole_document_and_decks` | 0.5892 |
| `interior::ship_code::tests::draft_only_changes_are_dirty_and_not_saved_as_applied` | 0.5918 |
| `interior::ship_code::tests::external_file_changes_are_not_overwritten` | 0.6011 |
| `interior::ship_code::tests::rejected_source_does_not_mutate_open_design` | 0.8400 |
| `interior::ship_code::tests::save_reopen_is_one_document_with_decks_and_metadata` | 0.5767 |
| `malformed_files_and_queries_are_errors` | 0.0043 |
| `models::streaming_tests::completed_asset_removes_cached_ground_box_and_eviction_restores_it` | 0.0011 |
| `packages::tests::exhaustion_never_wraps_and_repacking_never_reuses` | 0.0005 |
| `packages::tests::whole_transfer_keeps_identity_split_ends_it` | 0.0005 |
| `pairs::tests::a_head_on_pass_is_caught_and_bounces_conserving_momentum` | 0.0005 |
| `pbr::streaming_tests::cyclic_nodes_refused_before_walk` | 0.0006 |
| `pbr::streaming_tests::image_dimensions_refused_before_expansion` | 0.0007 |
| `pbr::streaming_tests::owner_lease_survives_frame_and_worker_clones_without_pinning_source_model` | 0.0006 |
| `pbr::streaming_tests::prepared_textures_keep_colour_space_and_all_mips` | 0.0009 |
| `renderer::canvas_alpha_tests::alpha_spans_objects_and_ui_but_not_canvas_boundaries` | 0.0006 |
| `renderer::globe_stream_tests::globe_upload_budget_never_exposes_partial_or_skips_mips` | 0.0006 |
| `save::packing::tests::labels_lots_tare_and_high_water_counters_survive_repeated_load` | 0.8267 |
| `save::packing::tests::legacy_save_does_not_invent_labels` | 0.8330 |
| `save::packing::tests::works_partial_packages_and_keyed_inventory_restore_with_fresh_views` | 0.7888 |
| `semantic_faults_reject_without_silent_repair` | 0.0404 |
| `ship::flight_control_tests::live_drive_matches_frozen_controller_over_mixed_spans` | 0.5066 |
| `stale_source_or_invalid_new_document_is_rejected` | 0.0401 |
| `test_drive::tests::last_fuel_derates_realized_control_torque` | 0.0008 |
| `test_drive::tests::studio_step_matches_shared_controller_with_rotated_body` | 0.0007 |
| `text_editor::tests::invalid_paste_is_atomic_and_edit_after_undo_clears_redo` | 0.0074 |
| `the_charted_world_is_as_the_registry_has_it` | 0.7070 |
| `thrusters::tests::zero_yaw_authority_does_not_disable_balanced_lift` | 0.0009 |
| `visual_assets::tests::real_asset_cold_warm_and_scene_return_accounting` | 0.0777 |

Raw selection measurements: `out/review/test-prune/baseline.json`.

## Verification

Final `cargo test -q --workspace`: 50 passed, zero failed/ignored, no warnings.
Sum of reported test-harness execution times: 3.79s (compilation excluded).
Registry build passed. Production-token comparison against the original tree
confirmed no runtime code changes; remaining changes are tests and test helpers.
