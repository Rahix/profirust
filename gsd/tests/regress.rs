#[test]
fn regression_tests() {
    for gsd_file in std::fs::read_dir("tests/data/")
        .unwrap()
        .filter_map(|p| p.ok())
        .map(|p| p.path())
        .filter(|p| {
            p.extension()
                .map(|e| e.to_ascii_lowercase() == "gsd")
                .unwrap_or(false)
        })
    {
        let name = gsd_file.file_stem().unwrap().to_string_lossy().to_string();
        let gsd = gsd_parser::parse_from_file(gsd_file);
        insta::assert_debug_snapshot!(name.as_ref(), gsd);
    }
}

#[test]
fn regression_tests_prm() {
    for gsd_file in std::fs::read_dir("tests/data/")
        .unwrap()
        .filter_map(|p| p.ok())
        .map(|p| p.path())
        .filter(|p| {
            p.extension()
                .map(|e| e.to_ascii_lowercase() == "gsd")
                .unwrap_or(false)
        })
    {
        let name = gsd_file.file_stem().unwrap().to_string_lossy().to_string();
        let gsd = gsd_parser::parse_from_file(gsd_file);
        let prm = gsd_parser::PrmBuilder::new(&gsd);
        insta::assert_debug_snapshot!(format!("{}-PRM", name).as_ref(), prm.as_bytes());
    }
}
