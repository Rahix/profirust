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
    let mut prm = gsd_parser::PrmBuilder::new(&gsd.user_prm_data);

    // Try setting all the available parameters to some reasonable values.
    for (_, prm_ref) in gsd.user_prm_data.data_ref.iter() {
        if let Some(texts) = prm_ref.text_ref.as_ref() {
            let text = if texts.len() > 1 {
                texts.keys().nth(1).unwrap()
            } else {
                // Fallback when the list only has one text...
                texts.keys().next().unwrap()
            };
            prm.set_prm_from_text(&prm_ref.name, text);
        } else {
            let v = match &prm_ref.constraint {
                gsd_parser::PrmValueConstraint::MinMax(_, max) => *max,
                gsd_parser::PrmValueConstraint::Enum(values) => *values.last().unwrap(),
                gsd_parser::PrmValueConstraint::Unconstrained => 1,
            };
            prm.set_prm(&prm_ref.name, v);
        }
    }

    insta::assert_debug_snapshot!(format!("{}-PRM", name).as_ref(), prm.as_bytes());
}
