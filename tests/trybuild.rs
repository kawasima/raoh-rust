//! What the API refuses to compile is part of its contract too.

#[test]
fn misuse_does_not_compile() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/*.rs");
}
