use super::client::{binary_name, MAX_BINARY, TARGET};
use anyhow::{bail, Context, Result};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use ssh_key::{Algorithm, PublicKey, SshSig};
use std::io::Read;

pub const PUBLIC_KEY: &str = include_str!("../../config/update-public-key.pub");
pub const NAMESPACE: &str = "core_menu-release";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub version: Version,
    pub target: String,
    pub asset: String,
    pub size: u64,
    pub sha256: String,
}

pub fn authenticate(manifest: &[u8], signature: &[u8]) -> Result<Manifest> {
    authenticate_with(manifest, signature, PUBLIC_KEY)
}

fn authenticate_with(manifest: &[u8], signature: &[u8], key: &str) -> Result<Manifest> {
    let key =
        PublicKey::from_openssh(key.trim()).context("Clave pública de actualización no válida")?;
    if key.algorithm() != Algorithm::Ed25519 {
        bail!("La clave de actualización debe ser Ed25519");
    }
    let signature = SshSig::from_pem(signature).context("Firma del manifiesto no válida")?;
    key.verify(NAMESPACE, manifest, &signature)
        .context("Firma de actualización no auténtica")?;
    let manifest: Manifest = serde_json::from_slice(manifest).context("Manifiesto no válido")?;
    if manifest.schema != 1
        || manifest.target != TARGET
        || manifest.asset != binary_name(&manifest.version)
        || !manifest.version.pre.is_empty()
        || !manifest
            .version
            .cmp_precedence(&Version::parse(env!("CARGO_PKG_VERSION"))?)
            .is_gt()
        || manifest.size == 0
        || manifest.size > MAX_BINARY
        || manifest.sha256.len() != 64
        || !manifest
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        bail!("Manifiesto incompatible, incompleto o versión no posterior");
    }
    Ok(manifest)
}

pub fn digest(reader: impl Read, maximum: u64) -> Result<(u64, String)> {
    let mut reader = reader;
    let mut hash = Sha256::new();
    let mut buffer = [0; 32 * 1024];
    let mut size = 0;
    loop {
        let length = reader.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        size += length as u64;
        if size > maximum {
            bail!("El binario supera el límite de tamaño");
        }
        hash.update(&buffer[..length]);
    }
    Ok((size, format!("{:x}", hash.finalize())))
}

pub fn binary(reader: impl Read, manifest: &Manifest) -> Result<()> {
    let (size, hash) = digest(reader, manifest.size)?;
    if size != manifest.size || hash != manifest.sha256 {
        bail!("Tamaño o SHA-256 del ejecutable incorrecto");
    }
    Ok(())
}

#[cfg(test)]
pub fn signed_fixture() -> (Vec<u8>, Vec<u8>, String, Vec<u8>) {
    use ssh_key::{private::Ed25519Keypair, HashAlg, LineEnding, PrivateKey};
    // A deterministic test-only key, unrelated to the production trust key.
    let key = PrivateKey::new(Ed25519Keypair::from_seed(&[7; 32]).into(), "test").unwrap();
    let binary = b"\x7fELF test executable".to_vec();
    let (_, hash) = digest(&binary[..], MAX_BINARY).unwrap();
    let manifest = Manifest {
        schema: 1,
        version: Version::parse("99.0.0").unwrap(),
        target: TARGET.into(),
        asset: binary_name(&Version::parse("99.0.0").unwrap()),
        size: binary.len() as u64,
        sha256: hash,
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let signature = key
        .sign(NAMESPACE, HashAlg::Sha512, &bytes)
        .unwrap()
        .to_pem(LineEnding::LF)
        .unwrap();
    (
        bytes,
        signature.into_bytes(),
        key.public_key().to_openssh().unwrap(),
        binary,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_but_incompatible_manifests_are_rejected() {
        use ssh_key::{private::Ed25519Keypair, HashAlg, LineEnding, PrivateKey};
        let key = PrivateKey::new(Ed25519Keypair::from_seed(&[7; 32]).into(), "test").unwrap();
        let public = key.public_key().to_openssh().unwrap();
        let (original, _, _, _) = signed_fixture();
        let cases = [
            ("schema", serde_json::json!(2)),
            ("target", serde_json::json!("wrong-target")),
            ("version", serde_json::json!(env!("CARGO_PKG_VERSION"))),
            ("version", serde_json::json!("0.0.1")),
            ("version", serde_json::json!("99.0.0-beta.1")),
            ("asset", serde_json::json!("other-file")),
            ("size", serde_json::json!(0)),
            ("size", serde_json::json!(MAX_BINARY + 1)),
            ("sha256", serde_json::json!("invalid")),
        ];
        for (field, value) in cases {
            let mut manifest: serde_json::Value = serde_json::from_slice(&original).unwrap();
            manifest[field] = value;
            if field == "version" {
                let version = Version::parse(manifest["version"].as_str().unwrap()).unwrap();
                manifest["asset"] = serde_json::json!(binary_name(&version));
            }
            let bytes = serde_json::to_vec(&manifest).unwrap();
            let signature = key
                .sign(NAMESPACE, HashAlg::Sha512, &bytes)
                .unwrap()
                .to_pem(LineEnding::LF)
                .unwrap();
            assert!(
                authenticate_with(&bytes, signature.as_bytes(), &public).is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn verifies_ed25519_signature_and_exact_binary() {
        let (bytes, signature, key, content) = signed_fixture();
        let manifest = authenticate_with(&bytes, &signature, &key).unwrap();
        binary(&content[..], &manifest).unwrap();
        assert!(binary(&b"tampered"[..], &manifest).is_err());
        assert!(binary(&content[..content.len() - 1], &manifest).is_err());
        assert!(authenticate(&bytes, &signature).is_err());
        let mut tampered = bytes;
        tampered[0] ^= 1;
        assert!(authenticate_with(&tampered, &signature, &key).is_err());
        assert!(authenticate_with(&tampered, b"bad signature", &key).is_err());
    }

    #[test]
    fn openssh_release_signatures_are_compatible() {
        use ssh_key::{private::Ed25519Keypair, LineEnding, PrivateKey};
        use std::{fs, io::Write, process::Command};
        let directory = tempfile::tempdir().unwrap();
        let mut private = tempfile::NamedTempFile::new_in(directory.path()).unwrap();
        let key = PrivateKey::new(Ed25519Keypair::from_seed(&[7; 32]).into(), "test").unwrap();
        private
            .write_all(key.to_openssh(LineEnding::LF).unwrap().as_bytes())
            .unwrap();
        let (manifest, _, public, content) = signed_fixture();
        let path = directory.path().join("manifest.json");
        fs::write(&path, &manifest).unwrap();
        let output = Command::new("ssh-keygen")
            .args(["-Y", "sign", "-f"])
            .arg(private.path())
            .args(["-n", NAMESPACE])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let signature = fs::read(directory.path().join("manifest.json.sig")).unwrap();
        let verified = authenticate_with(&manifest, &signature, &public).unwrap();
        binary(&content[..], &verified).unwrap();
    }

    #[test]
    fn supplied_production_public_key_is_ed25519() {
        assert_eq!(
            PublicKey::from_openssh(PUBLIC_KEY.trim())
                .unwrap()
                .algorithm(),
            Algorithm::Ed25519
        );
    }
}
