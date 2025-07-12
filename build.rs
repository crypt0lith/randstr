use std::{env, path::Path, process::Command};

const UCD_TABLES_DIR: &str = "build/ucd";
const UCD_TABLES_CODEGEN: &str = "build/ucd/generate.py";
const UCD_TABLES_OUTFILE: &str = "ucd_tables.rs";

fn main() {
    for fname in ["generate.py", "properties.json"] {
        println!(
            "cargo:rerun-if-changed={}",
            Path::new(&UCD_TABLES_DIR).join(fname).to_str().unwrap()
        );
    }
    assert!(
        Command::new("python")
            .arg(UCD_TABLES_CODEGEN)
            .arg(
                Path::new(&env::var("OUT_DIR").unwrap())
                    .join(UCD_TABLES_OUTFILE)
                    .to_str()
                    .unwrap()
            )
            .status()
            .expect(&format!("failed to run {}", UCD_TABLES_CODEGEN))
            .success(),
        "{} failed",
        UCD_TABLES_CODEGEN
    );
}
