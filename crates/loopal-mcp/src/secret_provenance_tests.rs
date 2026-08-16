use loopal_secret_client::SecretString;

use super::SecretProvenance;

fn seed(value: &str) -> Vec<(String, SecretString)> {
    vec![("token".into(), SecretString::from(value))]
}

#[test]
fn establishes_matches_rejects_rotation_and_resets() {
    let provenance = SecretProvenance::default();

    provenance.establish(&seed("first")).unwrap();
    provenance.establish(&seed("first")).unwrap();
    assert!(provenance.establish(&seed("second")).is_err());

    provenance.reset().unwrap();
    provenance.establish(&seed("second")).unwrap();
    provenance.establish(&seed("second")).unwrap();
}

#[test]
fn distinguishes_secret_names_and_empty_seed() {
    let provenance = SecretProvenance::default();
    provenance.establish(&[]).unwrap();
    provenance.establish(&[]).unwrap();
    assert!(provenance.establish(&seed("value")).is_err());

    let left = SecretProvenance::default();
    left.establish(&[("left".into(), SecretString::from("same"))])
        .unwrap();
    assert!(
        left.establish(&[("right".into(), SecretString::from("same"))])
            .is_err()
    );
}
