use shadow_rs::{BuildPattern, ShadowBuilder};

fn main() {
    ShadowBuilder::builder()
        .build_pattern(BuildPattern::RealTime)
        .build()
        .expect("generate shadow-rs build metadata");
}
