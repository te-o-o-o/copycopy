//! Each case is a text and what copycopy pastes of it with both boxes ticked.
//! Tokens are assembled at run time so no scanner mistakes this file for a
//! leak.

use copycopy_core::mask::mask;

#[test]
fn masks_emails_and_secrets() {
    let ghp = format!("ghp_{}", "a1B2".repeat(9));
    let aws = format!("AKIA{}", "Q7".repeat(8));
    let jwt = format!(
        "eyJ{}.eyJ{}.{}",
        "x".repeat(15),
        "y".repeat(15),
        "z".repeat(20)
    );
    let cases = [
        (
            "write to jane.doe+cc@example.co.uk.".to_string(),
            "write to [email].",
        ),
        (
            "no address here: @handle, a@b".to_string(),
            "no address here: @handle, a@b",
        ),
        (
            format!("export GITHUB_TOKEN={ghp}"),
            "export GITHUB_TOKEN=[secret]",
        ),
        (format!("key {aws} done"), "key [secret] done"),
        (
            format!("Authorization: Bearer {jwt}"),
            "Authorization: Bearer [secret]",
        ),
        (
            "DB_PASSWORD=\"hunter2hunter\"".to_string(),
            "DB_PASSWORD=\"[secret]\"",
        ),
        (
            "{\"api_key\": \"abcdef123456\"}".to_string(),
            "{\"api_key\": \"[secret]\"}",
        ),
        (
            "postgres://admin:s3cr3tpw@db.example.com/app".to_string(),
            "postgres://admin:[secret]@db.example.com/app",
        ),
        ("max_tokens: 512".to_string(), "max_tokens: 512"),
        ("pip install sk-learn".to_string(), "pip install sk-learn"),
        (
            "a\n-----BEGIN RSA PRIVATE KEY-----\nMIIabc\n-----END RSA PRIVATE KEY-----\nb"
                .to_string(),
            "a\n[secret]\nb",
        ),
        ("café → ops@exemple.fr".to_string(), "café → [email]"),
    ];
    for (input, want) in &cases {
        assert_eq!(mask(input, true, true), *want, "input: {input}");
    }
}

#[test]
fn each_box_masks_only_its_kind() {
    let text = "ops@example.com password=hunter2hunter";
    assert_eq!(mask(text, true, false), "[email] password=hunter2hunter");
    assert_eq!(mask(text, false, true), "ops@example.com password=[secret]");
    assert_eq!(mask(text, false, false), text);
}
