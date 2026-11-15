//! RFC 8439 §2.8.2. Public inputs only. This dependency is confined to tests.
use chacha20poly1305::{
    aead::{AeadInPlace, KeyInit},
    ChaCha20Poly1305, Nonce, Tag,
};

fn hex(s: &str) -> Vec<u8> {
    assert_eq!(s.len() % 2, 0);
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i+2], 16).unwrap()).collect()
}

#[test]
fn published_aead_vector_and_tampering() {
    let key: Vec<u8> = (0x80..=0x9f).collect();
    let cipher = ChaCha20Poly1305::new_from_slice(&key).unwrap();
    let nonce_bytes = hex("070000004041424344454647");
    let nonce = Nonce::from_slice(&nonce_bytes);
    let aad = hex("50515253c0c1c2c3c4c5c6c7");
    let plaintext = b"Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
    let expected = hex(concat!(
        "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d6",
        "3dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b36",
        "92ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc",
        "3ff4def08e4b7a9de576d26586cec64b6116"
    ));
    let expected_tag = hex("1ae10b594f09e26a7e902ecbd0600691");
    let mut encrypted = plaintext.to_vec();
    let tag = cipher.encrypt_in_place_detached(nonce, &aad, &mut encrypted).unwrap();
    assert_eq!(encrypted, expected);
    assert_eq!(tag.as_slice(), expected_tag);
    let mut opened = encrypted.clone();
    cipher.decrypt_in_place_detached(nonce, &aad, &mut opened, &tag).unwrap();
    assert_eq!(opened, plaintext);
    for altered in ["ciphertext", "aad", "tag", "nonce"] {
        let mut c = encrypted.clone();
        let mut a = aad.clone();
        let mut t = expected_tag.clone();
        let mut n = nonce_bytes.clone();
        match altered {
            "ciphertext" => c[0] ^= 1,
            "aad" => a[0] ^= 1,
            "tag" => t[0] ^= 1,
            "nonce" => n[0] ^= 1,
            _ => unreachable!(),
        }
        assert!(cipher.decrypt_in_place_detached(Nonce::from_slice(&n), &a, &mut c, Tag::from_slice(&t)).is_err(), "{altered}");
    }
}
