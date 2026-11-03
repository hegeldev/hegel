use super::*;

#[test]
fn label_from_name_is_the_engines_fnv1a_hash() {
    assert_eq!(label_from_name(""), 0xcbf29ce484222325);
    assert_eq!(label_from_name("a"), 0xaf63dc4c8601ec8c);
    assert_eq!(label_from_name("foobar"), 0x85944171f73967e8);
}

#[test]
fn default_labels_come_from_the_type_name() {
    struct Alpha;
    struct Beta;
    impl crate::generators::Generator<()> for Alpha {
        fn do_draw(&self, _tc: &crate::TestCase) {}
    }
    impl crate::generators::Generator<()> for Beta {
        fn do_draw(&self, _tc: &crate::TestCase) {}
    }
    use crate::generators::Generator;
    assert_eq!(
        Alpha.label(),
        label_from_name(std::any::type_name::<Alpha>())
    );
    assert_ne!(Alpha.label(), Beta.label());
}

#[test]
fn combining_distinguishes_order_and_arity() {
    let a = label_from_name("a");
    let b = label_from_name("b");
    assert_ne!(combine_labels(&[a, b]), combine_labels(&[b, a]));
    assert_ne!(combine_labels(&[a]), a);
    assert_ne!(combine_labels(&[a]), combine_labels(&[a, a]));
    assert_ne!(combine_labels(&[]), combine_labels(&[a]));
}
