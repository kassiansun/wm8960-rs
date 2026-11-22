#[cfg(not(feature = "codegen"))]
fn main() {}

#[cfg(feature = "codegen")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use chiptool::ir::{BitOffset, BlockItemInner, Field, IR};
    use std::{collections::BTreeMap, env, fmt::Write as _, fs, io, path::PathBuf};

    const REGISTER_COUNT: usize = 56;
    const REGISTER_BITS: u32 = 9;
    const INPUT: &str = "wm8960.yaml";
    let manifest_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let checked_in = manifest_dir.join("src/generated/regmap.rs");

    println!("cargo:rerun-if-changed={INPUT}");
    println!("cargo:rerun-if-changed={}", checked_in.display());

    let mut map: IR = serde_yaml::from_slice(&fs::read(manifest_dir.join(INPUT))?)?;
    let mut block = map.blocks.remove("WM8960").ok_or("Missing block/WM8960")?;
    block.items.sort_by_key(|item| item.byte_offset);

    let mut output = String::from("// @generated from wm8960.yaml by build.rs; do not edit.\n");
    output.push_str("pub mod registers {\n");
    let mut seen = [false; REGISTER_COUNT];
    for item in block.items {
        let BlockItemInner::Register(_) = item.inner else {
            return Err(format!("Expected a register for {}", item.name).into());
        };
        if item.array.is_some() {
            return Err(format!("Unsupported register array {}", item.name).into());
        }
        let addr = usize::try_from(item.byte_offset)?;
        if addr >= REGISTER_COUNT || seen[addr] {
            return Err(format!("Invalid address value for register {}", item.name).into());
        }
        seen[addr] = true;
        let prefix = if item.name.starts_with(|c: char| c.is_ascii_digit()) {
            "_"
        } else {
            ""
        };
        writeln!(
            output,
            "    pub const {prefix}{}: u8 = {addr:#04x};",
            item.name
        )?;
    }
    output.push_str("}\n\npub mod fields {\n    use crate::Field;\n\n");
    for (name, fieldset) in map.fieldsets.into_iter().collect::<BTreeMap<_, _>>() {
        for field in fieldset.fields {
            let Field {
                name: field,
                bit_offset,
                bit_size,
                array,
                ..
            } = field;
            if array.is_some() {
                return Err(format!("Unsupported field array {name}.{field}").into());
            }
            let BitOffset::Regular(bit_offset) = bit_offset else {
                return Err(format!("Unsupported split bit field {name}.{field}").into());
            };
            if bit_size == 0 || bit_size > REGISTER_BITS || bit_offset > REGISTER_BITS - bit_size {
                return Err(format!("Invalid bit range for {name}.{field}").into());
            }
            let mask = ((1u16 << bit_size) - 1) << bit_offset;
            writeln!(
                output,
                "    pub const {name}_{field}: Field = Field {{ mask: {mask:#05x}, shift: {bit_offset} }};"
            )?;
        }
    }

    writeln!(output, "}}\n\n")?;

    fs::write(out_dir.join("regmap.rs"), &output)?;
    match fs::read(&checked_in) {
        Ok(current) if current == output.as_bytes() => {}
        Ok(_) => fs::write(&checked_in, &output)?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            fs::create_dir_all(checked_in.parent().unwrap())?;
            fs::write(&checked_in, &output)?;
        }
        Err(err) => return Err(err.into()),
    }
    Ok(())
}
