use tree_sitter::Language;

extern "C" {
    pub fn tree_sitter_tsx() -> Language;
    pub fn tree_sitter_typescript() -> Language;
}