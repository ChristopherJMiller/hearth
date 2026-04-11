pub mod builder;
pub mod cache;
pub mod config_gen;
pub mod evaluator;
pub mod orchestrator;
pub mod policy_eval;
pub mod sbom;
pub mod user_env;

/// Common Nix CLI options for tarball cache busting and binary cache substitution.
///
/// Returns args like `["--option", "tarball-ttl", "0", "--option", "extra-substituters", "http://..."]`.
/// Used by both `nix-eval-jobs` and `nix build` invocations.
pub fn nix_extra_args() -> Vec<String> {
    let mut args = vec![
        "--option".into(),
        "tarball-ttl".into(),
        "0".into(),
        "--option".into(),
        "narinfo-cache-negative-ttl".into(),
        "0".into(),
    ];
    if let Ok(url) = std::env::var("ATTIC_CACHE_URL") {
        args.extend([
            "--option".into(),
            "extra-substituters".into(),
            url,
        ]);
    }
    args
}
