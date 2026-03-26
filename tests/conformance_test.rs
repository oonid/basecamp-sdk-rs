mod conformance;

use std::path::Path;

use conformance::ConformanceRunner;

#[test]
fn run_conformance_tests() {
    let tests_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("basecamp-sdk")
        .join("conformance")
        .join("tests");

    let runner = ConformanceRunner::new(tests_dir);
    let all_passed = runner.run_all_and_report();
    assert!(all_passed, "Some conformance tests failed");
}
