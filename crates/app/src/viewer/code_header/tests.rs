use super::*;

#[test]
fn a_relative_path_is_counted_from_the_root_and_only_inside_it() {
    let root = Some(Path::new("/r"));
    assert_eq!(
        relative(Path::new("/r/src/a.ts"), root),
        Some(Path::new("src/a.ts"))
    );
    assert_eq!(relative(Path::new("/elsewhere/a.ts"), root), None);
    // A sibling whose name starts like the root is not inside it.
    assert_eq!(relative(Path::new("/r2/a.ts"), root), None);
    assert_eq!(relative(Path::new("/r"), root), None);
    assert_eq!(relative(Path::new("/r/src/a.ts"), None), None);
}
