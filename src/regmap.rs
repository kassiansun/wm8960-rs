#[cfg(feature = "codegen")]
include!(concat!(env!("OUT_DIR"), "/regmap.rs"));

#[cfg(not(feature = "codegen"))]
include!("generated/regmap.rs");
