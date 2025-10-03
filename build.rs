use shadow_rs::{BuildPattern, ShadowBuilder};
use std::env;

fn python_version_from_rust(version: &str) -> String {
    let mut parts = version.splitn(2, '+');
    let core = parts.next().unwrap_or(version);
    let local = parts.next();

    let mut python = String::new();

    if let Some((release, prerelease)) = core.split_once('-') {
        python.push_str(release);

        let mut segments = prerelease.split('.');
        let tag = segments.next().unwrap_or_default();
        let normalized = match tag {
            "alpha" | "a" => "a",
            "beta" | "b" => "b",
            "rc" | "pre" | "preview" => "rc",
            "dev" => ".dev",
            _ => {
                python.push('-');
                python.push_str(prerelease);
                if let Some(local) = local {
                    python.push('+');
                    python.push_str(local);
                }
                return python;
            }
        };

        let suffix = segments.collect::<Vec<_>>().join("");

        if normalized == ".dev" {
            python.push_str(normalized);
            if suffix.is_empty() {
                python.push('0');
            } else {
                python.push_str(&suffix);
            }
        } else {
            python.push_str(normalized);
            python.push_str(&suffix);
        }
    } else {
        python.push_str(core);
    }

    if let Some(local) = local {
        python.push('+');
        python.push_str(local);
    }

    python
}

fn main() {
    let version = env::var("CARGO_PKG_VERSION").expect("missing CARGO_PKG_VERSION");
    let python_version = python_version_from_rust(&version);
    println!("cargo:rustc-env=SYNCX_PY_VERSION={python_version}");

    ShadowBuilder::builder()
        .build_pattern(BuildPattern::RealTime)
        .build()
        .expect("generate shadow-rs build metadata");
}
