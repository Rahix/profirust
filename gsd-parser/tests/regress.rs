use std::path::PathBuf;

#[rstest::rstest]
fn regress(#[files("tests/data/*.[gG][sS][dD]")] gsd_file: PathBuf) {
    let name = gsd_file.file_stem().unwrap().to_string_lossy().to_string();
    let gsd = gsd_parser::parse_from_file(gsd_file);
    insta::assert_debug_snapshot!(name.as_ref(), gsd);
}

#[rstest::rstest]
fn regress_prm(#[files("tests/data/*.[gG][sS][dD]")] gsd_file: PathBuf) {
    let name = gsd_file.file_stem().unwrap().to_string_lossy().to_string();
    let gsd = gsd_parser::parse_from_file(gsd_file);
    let mut prm = gsd_parser::PrmBuilder::new(&gsd.user_prm_data).unwrap();

    // Try setting all the available parameters to some reasonable values.
    for (_, prm_ref) in gsd.user_prm_data.data_ref.iter() {
        if let Some(texts) = prm_ref.text_ref.as_ref() {
            let text = if texts.len() > 1 {
                texts.keys().nth(1).unwrap()
            } else {
                // Fallback when the list only has one text...
                texts.keys().next().unwrap()
            };
            prm.set_prm_from_text(&prm_ref.name, text).unwrap();

            // Test that trying a wrong text doens't panic
            let res = prm.set_prm_from_text(&prm_ref.name, "InvalidTextAllTheWay");
            assert!(res.is_err());
        } else {
            let v = match &prm_ref.constraint {
                gsd_parser::PrmValueConstraint::MinMax(_, max) => *max,
                gsd_parser::PrmValueConstraint::Enum(values) => *values.last().unwrap(),
                gsd_parser::PrmValueConstraint::Unconstrained => 1,
            };
            prm.set_prm(&prm_ref.name, v).unwrap();

            // Test that an invalid value results in an error rather than a panic
            let res = prm.set_prm(&prm_ref.name, i64::MIN);
            assert!(res.is_err());

            // Test that trying a text PRM doesn't panic
            let res = prm.set_prm_from_text(&prm_ref.name, "InvalidTextAllTheWay");
            assert!(res.is_err());
        }
    }

    // Test that a non-existent PRM doesn't panic
    let res = prm.set_prm("ThisPrmNeverEverExistsEver", 0);
    assert!(res.is_err());

    insta::assert_debug_snapshot!(format!("{}-PRM", name).as_ref(), prm.as_bytes());
}
