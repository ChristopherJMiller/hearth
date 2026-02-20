-- Remove the legacy role_closures table.
-- Per-machine closures (built by hearth-build-worker using real hardware configs)
-- have superseded the generic role-based closure system.
DROP TABLE IF EXISTS role_closures;
