use std::path::PathBuf;
fn main() {
    let typescript_dir: PathBuf = ["tree-sitter-typescript", "typescript", "src"].iter().collect();
    let tsx_dir: PathBuf = ["tree-sitter-typescript", "tsx", "src"].iter().collect();

    cc::Build::new()
        .include(&typescript_dir)
        .file(typescript_dir.join("parser.c"))
        .file(typescript_dir.join("scanner.c"))
        .compile("tree-sitter-typescript");

    cc::Build::new()
        .include(&tsx_dir)
        .file(tsx_dir.join("parser.c"))
        .file(tsx_dir.join("scanner.c"))
        .compile("tree-sitter-tsx");
}
