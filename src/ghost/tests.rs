//! Ghost Protocol v2 tests.
//!
//! SHAKE256 vectors were generated with Python's `hashlib`:
//!
//! ```text
//! python3 -c "import hashlib
//! for n in [0,1,135,136,137,300]:
//!     data = bytes(i % 251 for i in range(n))
//!     print(n, hashlib.shake_256(data).hexdigest(32), hashlib.shake_256(data).hexdigest(300))"
//! python3 -c "import hashlib; print(hashlib.shake_256(b'abc').hexdigest(32))"
//! ```
//!
//! The KDF and permutation known-answer values were produced by a Python
//! re-implementation of the construction (see the individual tests).

use std::num::NonZeroUsize;

use super::bijection::{forward, inverse, Image};
use super::feistel::{encoding_round_trip, Feistel};
use super::keccak::Shake256;
use super::mac::tags_equal;
use super::permutation::{apply, permutation, uniform_index, Direction, MAX_REJECTIONS};
use super::primitives::{new_xof, Xof, XofReader};
use super::transform::{
    all_tenses, domain, parse_single, Keystream, CLASS_GLYPH, CLASS_NUMERAL_DIGIT, CLASS_TENSE,
    NUMBERS,
};
use super::{
    decrypt_period, encrypt_period, Argon2idKdf, CryptoSuite, GhostEnvelope, GhostError,
    GhostParams, GhostSession, Kdf, MasterKey, Nonce, ReplayWindow, Shake256KeyDerivation,
};
use crate::ast::{
    ClauseBlock, PrimarySignal, RelationalSignal, SecondarySignal, SignalDeterminers, TemporalMode,
};
use crate::dti::{Dti, DtiKey};
use crate::session::Session;
use crate::span::Span;

type TestResult = Result<(), String>;

// ═══════════════════════════ Helpers ═══════════════════════════

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Bytes `i % 251` for `i in 0..len`.
fn pattern(len: usize) -> Vec<u8> {
    (0_u8..251).cycle().take(len).collect()
}

fn check<T: PartialEq + std::fmt::Debug>(what: &str, got: T, want: T) -> TestResult {
    if got == want {
        Ok(())
    } else {
        Err(format!("{what}: got {got:?}, want {want:?}"))
    }
}

fn expect_err<T: std::fmt::Debug>(
    what: &str,
    result: Result<T, GhostError>,
    accept: impl Fn(&GhostError) -> bool,
) -> TestResult {
    match result {
        Ok(value) => Err(format!("{what}: unexpectedly succeeded with {value:?}")),
        Err(error) if accept(&error) => Ok(()),
        Err(error) => Err(format!("{what}: unexpected error {error:?} ({error})")),
    }
}

fn key_from_tag(tag: &str) -> Result<DtiKey, String> {
    let mut body = [b'0'; 31];
    for (slot, byte) in body.iter_mut().zip(tag.bytes()) {
        *slot = byte;
    }
    DtiKey::from_random_body(body).map_err(|e| e.to_string())
}

const DOMAIN_SIZE: usize = 20;

struct Fixture {
    dti: Dti,
    /// Non-deprecated universal keys in declaration order.
    keys: Vec<DtiKey>,
    deprecated: DtiKey,
    local: DtiKey,
}

impl Fixture {
    fn key(&self, index: usize) -> Result<&str, String> {
        self.keys
            .get(index)
            .map(DtiKey::as_str)
            .ok_or_else(|| format!("fixture key {index} missing"))
    }

    /// One stage-2 valid period exercising every glyph form and several tile
    /// shapes.
    fn sample(&self) -> Result<String, String> {
        Ok(format!(
            "/^ taall {k0} a {k1}§{k2} : quatoalt ~ d ~ y deepzato {k3}§{k4} n ~~ \
ab ~~ abelm {k5} g {k1}§{k2}§{k6} ^/",
            k0 = self.key(0)?,
            k1 = self.key(1)?,
            k2 = self.key(2)?,
            k3 = self.key(3)?,
            k4 = self.key(4)?,
            k5 = self.key(5)?,
            k6 = self.key(6)?,
        ))
    }
}

fn fixture(version: u64) -> Result<Fixture, String> {
    fixture_with(version, Some(10))
}

/// Fixture whose universal snapshot declares a numeral system of base
/// `numeral_base` (digit `i` = fixture key `i`), or none.
fn fixture_with(version: u64, numeral_base: Option<u8>) -> Result<Fixture, String> {
    let keys = (0..DOMAIN_SIZE)
        .map(|i| key_from_tag(&format!("ghostkey{i:02}")))
        .collect::<Result<Vec<_>, _>>()?;
    let deprecated = key_from_tag("ghostold")?;
    let local = key_from_tag("ghostlocal")?;
    let first = keys.first().ok_or("empty fixture")?;
    let mut universal = format!("#awdti 1\n#role universal\n#version {version}\n#lang en\n");
    if let Some(base) = numeral_base {
        let digits: Vec<&str> = keys
            .iter()
            .take(usize::from(base))
            .map(DtiKey::as_str)
            .collect();
        universal.push_str(&format!(
            "#numeral base:{base} digits:{}\n",
            digits.join(",")
        ));
    }
    for (i, key) in keys.iter().enumerate() {
        universal.push_str(&format!("rec {key} concept{i}\n"));
    }
    universal.push_str(&format!(
        "rec {deprecated} oldconcept base:{first} superseded_by:{first}\n"
    ));
    let overlay = format!(
        "#awdti 1\n#role local\n#universal {version}\n#lang it\nuse {first} form:primo\n\
new {local} sensore base:{first}\n"
    );
    let dti = Dti::load(&universal, &overlay).map_err(|d| format!("{d:#?}"))?;
    Ok(Fixture {
        dti,
        keys,
        deprecated,
        local,
    })
}

fn master_key() -> MasterKey {
    let mut bytes = [0_u8; 32];
    for (byte, value) in bytes.iter_mut().zip(1_u8..) {
        *byte = value;
    }
    MasterKey::new(bytes)
}

fn nonce(seed: u8) -> Nonce {
    let mut bytes = [0_u8; 16];
    for (byte, value) in bytes.iter_mut().zip(0_u8..) {
        *byte = value.wrapping_mul(31).wrapping_add(seed);
    }
    Nonce(bytes)
}

fn window() -> Result<ReplayWindow, String> {
    NonZeroUsize::new(16)
        .map(ReplayWindow::new)
        .ok_or_else(|| "zero capacity".to_owned())
}

fn params(suite: CryptoSuite, permute_grammar_tiles: bool) -> GhostParams {
    GhostParams {
        suite,
        permute_grammar_tiles,
    }
}

fn available_suites() -> impl Iterator<Item = CryptoSuite> {
    CryptoSuite::ALL.into_iter().filter(|s| s.is_available())
}

fn ghost(result: Result<impl Sized, GhostError>) -> Result<(), String> {
    result.map(|_| ()).map_err(|e| e.to_string())
}

/// Test reader returning scripted `u64` values (then `u64::MAX` forever).
struct Scripted {
    values: Vec<u64>,
    next: usize,
}

impl XofReader for Scripted {
    fn squeeze(&mut self, out: &mut [u8]) {
        for chunk in out.chunks_mut(8) {
            let value = self.values.get(self.next).copied().unwrap_or(u64::MAX);
            self.next = self.next.saturating_add(1);
            for (target, source) in chunk.iter_mut().zip(value.to_le_bytes()) {
                *target = source;
            }
        }
    }
}

fn shake_reader(seed: &[u8]) -> Box<dyn XofReader> {
    let mut xof = Box::new(Shake256::new());
    xof.absorb(seed);
    xof.finalize_xof()
}

// ═══════════════════════════ SHAKE256 (G1) ═══════════════════════════

/// `(input length, SHAKE256 32-byte output, SHAKE256 300-byte output)` for input `i % 251`.
const SHAKE_VECTORS: [(usize, &str, &str); 6] = [
    (
        0,
        "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762f",
        "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762fd75dc4ddd8c0f200cb05019d67b592f6fc821c49479ab48640292eacb3b7c4be141e96616fb13957692cc7edd0b45ae3dc07223c8e92937bef84bc0eab862853349ec75546f58fb7c2775c38462c5010d846c185c15111e595522a6bcd16cf86f3d122109e3b1fdd943b6aec468a2d621a7c06c6a957c62b54dafc3be87567d677231395f6147293b68ceab7a9e0c58d864e8efde4e1b9a46cbe854713672f5caaae314ed9083dab4b099f8e300f01b8650f1f4b1d8fcf3f3cb53fb8e9eb2ea203bdc970f50ae55428a91f7f53ac266b28419c3778a15fd248d339ede785fb7f5a1aaa96d313eacc890936c173cdcd0fab882c45755feb3aed96d477ff96390bf9a66d1368b208e21f7c10d0",
    ),
    (
        1,
        "b8d01df855f7075882c636f6ddeacf41e5de0bbf30042ef0a86e36f4b8600d54",
        "b8d01df855f7075882c636f6ddeacf41e5de0bbf30042ef0a86e36f4b8600d546c516501a6a3c821678d3d9943fa9e74b9b99fccd47aecc91dd1f4946b8355b30a500d7bd8081e67ad4599a5c8e23706803f955aeff1686e54cdf48840e32dd2342c1a26fb27aaec2b4fe5b9111f6497143cc59be6ff2abeff59230ca332b31365af12ba4ee846ff4e55e8910b068a86c825a31832e6438a4058c580af06b8321acf9d21353158b3f74d9c6dd718edbbb5211231cf0dfb30a43be2e0c1303bc3e41049ed126cde17eb5872b15fdfc7897197566ebc389f8f10a8304889f64ee181df6c1da7088b2d5fe6295e5e5c4f83315ae4a60bb436c0da16108356e5121f071641b85f40a40fea0e77cffcbc5b75e5aa8fd76a18b72bec9906fb1bef9d3bca1f34f7e6a057ac3c5806d6",
    ),
    (
        135,
        "c45dae624ad8a2f5aa7bac9d7557737fd91c96eedb70a6be5574d57a844eade0",
        "c45dae624ad8a2f5aa7bac9d7557737fd91c96eedb70a6be5574d57a844eade07f4056bf081a1098101cea8132188c422136feb4687d1e2209f3fd28bedfb8f4468cba8501763511f507c9c14537403bf7804a89607b4c3f5afd484ec0c411c61e61d8784b2a0cb281ef9f44a4e32732adaba131875b0e34d587d1e63fea83b177a04230d041b8f96e77d6d9a7c142817cbf4cedfa17f386dc0206f4509ab4306763512d155dcbfa8ffeadb0a909da9464a28f01c9b5441ec85b534786c6a0ce90ec7721ed0f5a031b2caf7ae4f045c9aa1ffd346a5855500d7ce8981652a0d341005a8110c8f142b8e5c3f8fcfed96c9074c47e92c7f561ca73ab936d0b1a2bd65dabe82a1870f393db9c9a97a138194629fc4ba1b467acb533f52668759099525c4a6da6733c2eabb3bb4a",
    ),
    (
        136,
        "b7ff4073b3f5a8eabd6e17705ca7f6761a31058f9df781a6a47e3a3063b9d67a",
        "b7ff4073b3f5a8eabd6e17705ca7f6761a31058f9df781a6a47e3a3063b9d67a757e8dbf043dac48d2154e46d59c0b9e8bc36ba035153691fbe83b9eff5dae4a0aa01d73c984c49adc271297af1baa96931f24ef47a11781fed7722a293e223647e4be704fd5d63ee4e15a4a7cf7ad586b561b840e6225e6aae344dbe9a15fb155e4fa2ab7d7df09be06d83195c8892a2e6c5b56dadbb8f808ac517e305957e7e7cca407f39840a00bb60e35638bf0e2d551fb0e2703b4eb654c53427abb3932a40afb86b76373e6d8e3cfb91f3afa5412c6f1b9882876d7e458199d28d69fec8f6162b42243a85f2d2f63281c36f776623fb8cbd9e0664f492887059e38afbdee2b7861bbcb2ab0909ea36d91962dfbaa95eb700ce10e88cfa3b7911a24d648b1bf2b782c7c7a0867dbae51",
    ),
    (
        137,
        "01d90952c642a5eb2a8fc9d713f843a45d7ac05132dddcb2efc9bebc27e37bcb",
        "01d90952c642a5eb2a8fc9d713f843a45d7ac05132dddcb2efc9bebc27e37bcbe42130c36f3540250ab11796980e773683f28d07f0f838606fb9c45e452bd38fb9ed42c8994cbad998a1971cf3d7bc763f40cb04fefe876a20c27ece851d489539e1eaa5ecd62bb20bdad6526819462c6e4efb71a45c5b46dd012647abd1d899a03d1b514fb93828a21bc9368bc24fe63808d6be567248bae61f38ba3f9e676bbe8275ba47c2ff92d770468944b9933c96435488224af296b8b542f9fd3dc0f9f8f23a3e654af44e03876a4dcdd725baddf004ff41da3e5caf8590c3312ebf76e79acdc54fb80d39689119f19bcb78a43b64a63984d77b60dbfff9e42cc1be7e051cef9428c45e476610f91296aec260c660fb61a2c4e10a262ffa559292139c76cdbda6cd0a2754dfccd964",
    ),
    (
        300,
        "177c5689012e0ac1f06bea70d7946bf8dfc291cf53ed38086e0b6c81815e3800",
        "177c5689012e0ac1f06bea70d7946bf8dfc291cf53ed38086e0b6c81815e3800364a1a05690e3bab822c4959c3f11f90e6e4d2502487538281d90ed5fe2451f23c771dec6dcafdaccbeeba16ffffc057bb5a72a12d0cd2d05affdf892a28f22dfed62545a0fbccb7d779d9b0672442f08592da092d2489a5d5ea6836fd2145de5426343a03bb1cc45c14f03ad0b0136c3bea919d9ad6cc4c7c9c2c5b7f3f11e138dda3db9f7feb6c4c08df8a8590dbe1c1256d92134db25fb404467d974e601b20a2c34dca5d3b46a91f19cc618ef3ae708e8d86b4580633fa1640f944830835a402ce5994fac2d66319ae3728047939d1d110125f89df20ec4e19d602c38ae257182adec32cfcf5a08ac12ee0af5c3ba03419ca5e4a8beeb2e2d74a7aa7318b484c36d802e4a590227f7ba5",
    ),
];

#[test]
fn shake256_matches_python_hashlib_vectors() -> TestResult {
    for (length, short, long) in SHAKE_VECTORS {
        let data = pattern(length);
        let mut out32 = [0_u8; 32];
        Shake256::digest(&data, &mut out32);
        check(
            &format!("len {length}, 32 bytes"),
            hex(&out32).as_str(),
            short,
        )?;
        let mut out300 = [0_u8; 300];
        Shake256::digest(&data, &mut out300);
        check(
            &format!("len {length}, 300 bytes"),
            hex(&out300).as_str(),
            long,
        )?;
    }
    let mut abc = [0_u8; 32];
    Shake256::digest(b"abc", &mut abc);
    check(
        "abc",
        hex(&abc).as_str(),
        "483366601360a8771c6863080cc4114d8db44530f8f1e1ee4f94ea37e78b5739",
    )
}

/// Absorbs `data` in uneven chunks and squeezes `total` bytes in uneven chunks.
fn chunked(mut xof: Box<dyn Xof>, data: &[u8], total: usize) -> Vec<u8> {
    let mut rest = data;
    for size in [1_usize, 7, 128, 0, 136, 28, 3].into_iter().cycle() {
        if rest.is_empty() {
            break;
        }
        let (head, tail) = rest.split_at(size.min(rest.len()));
        xof.update(head);
        rest = tail;
    }
    let mut reader = xof.finalize_xof();
    let mut out = vec![0_u8; total];
    let mut window = out.as_mut_slice();
    for size in [1_usize, 135, 2, 136, 26, 64].into_iter().cycle() {
        if window.is_empty() {
            break;
        }
        let (head, tail) = window.split_at_mut(size.min(window.len()));
        reader.squeeze(head);
        window = tail;
    }
    out
}

fn one_shot(mut xof: Box<dyn Xof>, data: &[u8], total: usize) -> Vec<u8> {
    xof.update(data);
    let mut out = vec![0_u8; total];
    xof.finalize_xof().squeeze(&mut out);
    out
}

#[test]
fn shake256_incremental_absorb_and_squeeze_equal_one_shot() -> TestResult {
    for length in [0_usize, 1, 135, 136, 137, 300, 1000] {
        let data = pattern(length);
        let incremental = chunked(Box::new(Shake256::new()), &data, 300);
        let mut direct = vec![0_u8; 300];
        Shake256::digest(&data, &mut direct);
        check(&format!("length {length}"), hex(&incremental), hex(&direct))?;
    }
    let incremental = chunked(Box::new(Shake256::new()), &pattern(300), 300);
    let expected = SHAKE_VECTORS.last().map(|(_, _, long)| *long).unwrap_or("");
    check("300/300 vector", hex(&incremental).as_str(), expected)
}

#[test]
fn every_available_suite_xof_is_consistent_and_distinct() -> TestResult {
    let data = pattern(500);
    let mut outputs: Vec<(CryptoSuite, String)> = Vec::new();
    for suite in available_suites() {
        let a = chunked(new_xof(suite).map_err(|e| e.to_string())?, &data, 300);
        let b = one_shot(new_xof(suite).map_err(|e| e.to_string())?, &data, 300);
        check(&format!("{suite} chunked vs one-shot"), hex(&a), hex(&b))?;
        outputs.push((suite, hex(&a)));
    }
    for (i, (suite, output)) in outputs.iter().enumerate() {
        for (other_suite, other) in outputs.iter().skip(i.saturating_add(1)) {
            if output == other {
                return Err(format!(
                    "{suite} and {other_suite} produced the same stream"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(feature = "ghost-blake3")]
#[test]
fn blake3_adapter_matches_the_reference_empty_hash() -> TestResult {
    let out = one_shot(
        new_xof(CryptoSuite::Blake3).map_err(|e| e.to_string())?,
        b"",
        32,
    );
    check(
        "BLAKE3(\"\")",
        hex(&out).as_str(),
        "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262",
    )
}

#[test]
fn disabled_suites_report_suite_unavailable() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    for suite in CryptoSuite::ALL.into_iter().filter(|s| !s.is_available()) {
        expect_err("xof", new_xof(suite).map(|_| ()), |e| {
            *e == GhostError::SuiteUnavailable { suite }
        })?;
        expect_err(
            "encrypt",
            encrypt_period(
                &plain,
                &f.dti,
                &master_key(),
                nonce(1),
                &params(suite, false),
            ),
            |e| *e == GhostError::SuiteUnavailable { suite },
        )?;
    }
    check(
        "SHAKE256 always available",
        CryptoSuite::Shake256.is_available(),
        true,
    )
}

// ═══════════════════════════ Permutation (G2) ═══════════════════════════

#[test]
fn permutation_is_a_bijection_on_small_domains() -> TestResult {
    for size in 0_usize..=16 {
        let mut reader = shake_reader(&[u8::try_from(size).map_err(|e| e.to_string())?]);
        let images = permutation(reader.as_mut(), size).map_err(|e| e.to_string())?;
        for index in 0..size {
            let forward = apply(&images, index, Direction::Forward).ok_or("forward")?;
            let back = apply(&images, forward, Direction::Inverse).ok_or("inverse")?;
            check(&format!("size {size}, π⁻¹(π({index}))"), back, index)?;
        }
        check(
            "out of range",
            apply(&images, size, Direction::Forward),
            None,
        )?;
        let mut sorted = images;
        sorted.sort_unstable();
        check(
            &format!("size {size} is a permutation"),
            sorted,
            (0..size).collect::<Vec<_>>(),
        )?;
    }
    Ok(())
}

#[test]
fn glyph_mapping_is_a_bijection_on_the_fixture_domain() -> TestResult {
    let f = fixture(42)?;
    let domain = domain(&f.dti);
    check("domain size", domain.len(), DOMAIN_SIZE)?;
    let key = master_key();
    let keystream = Keystream {
        suite: CryptoSuite::Shake256,
        key: &key,
        nonce: nonce(9),
        snapshot_version: 42,
    };
    for position in 0_u64..4 {
        let images = keystream
            .permutation(CLASS_GLYPH, position, domain.len())
            .map_err(|e| e.to_string())?;
        let mut sorted = images;
        sorted.sort_unstable();
        check("bijection", sorted, (0..domain.len()).collect::<Vec<_>>())?;
    }
    Ok(())
}

/// Python reference (same construction, see the module docs):
/// `perm(bytes(range(32)), bytes(range(16, 32)), 42, b'glyph-atom', 3, 10)` and
/// `perm(..., b'signal-tense', 0, 13)` where the keystream is
/// `shake_256(f(label) + f(key) + nonce + pack('<Q', version) + f(cls) + pack('<Q', pos))`,
/// `f(b) = pack('<Q', len(b)) + b`, followed by Fisher–Yates with rejection sampling.
#[test]
fn keystream_permutation_matches_python_reference() -> TestResult {
    let mut key_bytes = [0_u8; 32];
    for (byte, value) in key_bytes.iter_mut().zip(0_u8..) {
        *byte = value;
    }
    let mut nonce_bytes = [0_u8; 16];
    for (byte, value) in nonce_bytes.iter_mut().zip(16_u8..) {
        *byte = value;
    }
    let key = MasterKey::new(key_bytes);
    let keystream = Keystream {
        suite: CryptoSuite::Shake256,
        key: &key,
        nonce: Nonce(nonce_bytes),
        snapshot_version: 42,
    };
    check(
        "glyph-atom P=3 n=10",
        keystream
            .permutation(CLASS_GLYPH, 3, 10)
            .map_err(|e| e.to_string())?,
        vec![2, 1, 5, 7, 6, 4, 3, 0, 8, 9],
    )?;
    check(
        "signal-tense P=0 n=13",
        keystream
            .permutation(CLASS_TENSE, 0, 13)
            .map_err(|e| e.to_string())?,
        vec![11, 6, 4, 3, 10, 12, 8, 9, 5, 0, 1, 2, 7],
    )
}

#[test]
fn rejection_sampling_never_yields_out_of_range_indices() -> TestResult {
    let mut reader = shake_reader(b"rejection");
    let large = [
        usize::MAX,
        usize::MAX / 3,
        (usize::MAX / 2).saturating_add(2),
        1 << 40,
        1_000_003,
    ];
    for bound in (1_usize..=64).chain(large) {
        for _ in 0..32 {
            let index = uniform_index(reader.as_mut(), bound).map_err(|e| e.to_string())?;
            if index >= bound {
                return Err(format!("index {index} out of range for bound {bound}"));
            }
        }
    }
    Ok(())
}

#[test]
fn rejection_sampling_rejects_the_biased_tail() -> TestResult {
    // For bound 3, 2^64 mod 3 = 1, so exactly the value u64::MAX is rejected.
    let mut scripted = Scripted {
        values: vec![u64::MAX, u64::MAX, 5],
        next: 0,
    };
    check(
        "skips two rejected draws",
        uniform_index(&mut scripted, 3),
        Ok(2),
    )?;
    check("draws consumed", scripted.next, 3)?;
    // u64::MAX - 1 is the last accepted value for bound 3.
    let mut edge = Scripted {
        values: vec![u64::MAX - 1],
        next: 0,
    };
    // (2^64 - 2) mod 3 = 2.
    check("edge accepted", uniform_index(&mut edge, 3), Ok(2))?;
    // Powers of two never reject.
    let mut power = Scripted {
        values: vec![u64::MAX],
        next: 0,
    };
    check("power of two", uniform_index(&mut power, 8), Ok(7))?;
    // A degenerate stream is reported instead of looping forever.
    let mut stuck = Scripted {
        values: Vec::new(),
        next: 0,
    };
    check(
        "degenerate stream",
        uniform_index(&mut stuck, 3),
        Err(GhostError::KeystreamExhausted),
    )?;
    check(
        "bounded attempts",
        stuck.next,
        usize::try_from(MAX_REJECTIONS).map_err(|e| e.to_string())?,
    )?;
    check(
        "bound zero",
        uniform_index(&mut stuck, 0),
        Err(GhostError::KeystreamExhausted),
    )
}

// ═══════════════════════════ Round trip (G2, G5) ═══════════════════════════

/// Ingests `texts` in order into a fresh session: every period must be valid at
/// stage 2 (no diagnostics).
fn stage2_valid(texts: &[&str]) -> TestResult {
    let mut session = Session::new();
    for text in texts {
        session
            .ingest(text)
            .map_err(|d| format!("not stage-2 valid: {text}: {d:?}"))?;
    }
    Ok(())
}

/// Ingests both streams into fresh sessions and compares the resolution and the
/// value of every pair of every period.
fn same_resolutions(plain: &[&str], cipher: &[&str]) -> TestResult {
    let mut left = Session::new();
    let mut right = Session::new();
    for (p, c) in plain.iter().zip(cipher) {
        let a = left.ingest(p).map_err(|d| format!("{p}: {d:?}"))?;
        let b = right.ingest(c).map_err(|d| format!("{c}: {d:?}"))?;
        for (x, y) in a.iter().zip(&b) {
            let (Some(x), Some(y)) = (left.analysis(*x), right.analysis(*y)) else {
                return Err("missing analysis".to_owned());
            };
            check("clause count", x.clauses().len(), y.clauses().len())?;
            for (cx, cy) in x.clauses().iter().zip(y.clauses()) {
                let rx: Vec<_> = cx
                    .slots()
                    .iter()
                    .map(|s| s.map(|s| (s.resolution(), s.value())))
                    .collect();
                let ry: Vec<_> = cy
                    .slots()
                    .iter()
                    .map(|s| s.map(|s| (s.resolution(), s.value())))
                    .collect();
                check(&format!("resolutions of {p} / {c}"), rx, ry)?;
            }
        }
    }
    Ok(())
}

#[test]
fn sample_is_stage2_valid() -> TestResult {
    let f = fixture(42)?;
    stage2_valid(&[&f.sample()?])
}

#[test]
fn round_trip_restores_the_exact_plaintext() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    let key = master_key();
    for suite in available_suites() {
        for permute in [false, true] {
            let p = params(suite, permute);
            let envelope = encrypt_period(&plain, &f.dti, &key, nonce(1), &p)
                .map_err(|e| format!("{suite}/{permute}: {e}"))?;
            if envelope.ciphertext == plain {
                return Err(format!("{suite}/{permute}: ciphertext equals plaintext"));
            }
            check("suite", envelope.suite, suite)?;
            check("version", envelope.snapshot_version, 42)?;
            check("sequence", envelope.sequence, 0)?;
            stage2_valid(&[&envelope.ciphertext])?;
            let mut replay = window()?;
            let decrypted = decrypt_period(&envelope, &f.dti, &key, &p, &mut replay)
                .map_err(|e| format!("{suite}/{permute}: {e}"))?;
            check(
                &format!("{suite}/{permute} round trip"),
                decrypted.as_str(),
                plain.as_str(),
            )?;
        }
    }
    Ok(())
}

#[test]
fn ciphertext_reparses_as_one_valid_period_with_the_same_shape() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    let domain = domain(&f.dti);
    for permute in [false, true] {
        let envelope = encrypt_period(
            &plain,
            &f.dti,
            &master_key(),
            nonce(2),
            &params(CryptoSuite::Shake256, permute),
        )
        .map_err(|e| e.to_string())?;
        ghost(parse_single(&envelope.ciphertext))?;
        stage2_valid(&[&envelope.ciphertext])?;
        same_resolutions(&[&plain], &[&envelope.ciphertext])?;
        let plain_tokens: Vec<&str> = plain.split(' ').collect();
        let cipher_tokens: Vec<&str> = envelope.ciphertext.split(' ').collect();
        check("token count", cipher_tokens.len(), plain_tokens.len())?;
        for (p, c) in plain_tokens.iter().zip(&cipher_tokens) {
            // Markers and connectors are never touched.
            if ["/^", "^/", ":", "y", "~", "~~"].contains(p) {
                check("structural token", *c, *p)?;
            }
            // Every ciphertext atom is a domain key.
            if p.len() >= 32 {
                let body = c.trim_end_matches('~');
                for atom in body.split('§') {
                    let key = DtiKey::parse(atom).map_err(|e| e.to_string())?;
                    if domain.binary_search(&key).is_err() {
                        return Err(format!("ciphertext atom {atom} outside the domain"));
                    }
                }
            }
            if !permute && p.len() < 32 {
                check("tiles untouched without the flag", *c, *p)?;
            }
        }
    }
    Ok(())
}

#[test]
fn grammar_tiles_change_with_the_flag() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    // Over several nonces at least one tile must change (a fixed point on every
    // tile for every nonce would mean the flag is ignored).
    let mut changed = false;
    for seed in 0_u8..8 {
        let envelope = encrypt_period(
            &plain,
            &f.dti,
            &master_key(),
            nonce(seed),
            &params(CryptoSuite::Shake256, true),
        )
        .map_err(|e| e.to_string())?;
        let tiles_differ = plain
            .split(' ')
            .zip(envelope.ciphertext.split(' '))
            .any(|(p, c)| p.len() < 32 && p != c);
        changed |= tiles_differ;
    }
    check("some grammar tile changed", changed, true)
}

#[test]
fn encryption_is_deterministic_for_equal_nonces() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    let p = GhostParams::default();
    let a =
        encrypt_period(&plain, &f.dti, &master_key(), nonce(3), &p).map_err(|e| e.to_string())?;
    let b =
        encrypt_period(&plain, &f.dti, &master_key(), nonce(3), &p).map_err(|e| e.to_string())?;
    check("same envelope", a, b)
}

#[test]
fn different_nonces_give_different_ciphertexts() -> TestResult {
    let f = fixture(42)?;
    let plain = f.sample()?;
    let p = GhostParams::default();
    let a =
        encrypt_period(&plain, &f.dti, &master_key(), nonce(4), &p).map_err(|e| e.to_string())?;
    let b =
        encrypt_period(&plain, &f.dti, &master_key(), nonce(5), &p).map_err(|e| e.to_string())?;
    if a.ciphertext == b.ciphertext {
        return Err("equal ciphertexts for different nonces".to_owned());
    }
    if a.mac_tag == b.mac_tag {
        return Err("equal tags for different nonces".to_owned());
    }
    // A different key also changes the ciphertext.
    let other_key = MasterKey::new([0xA5; 32]);
    let c = encrypt_period(&plain, &f.dti, &other_key, nonce(4), &p).map_err(|e| e.to_string())?;
    if a.ciphertext == c.ciphertext {
        return Err("equal ciphertexts for different keys".to_owned());
    }
    Ok(())
}

/// G1 (option 2): inside the visible scope equal plaintext atoms share one
/// cipher atom and different plaintext atoms never share one.
#[test]
fn visible_atoms_are_mapped_injectively_and_consistently() -> TestResult {
    let f = fixture(42)?;
    let k = |i: usize| f.key(i);
    let plain = format!(
        "/^ ta {} a {} d {}§{} g {} n {} ab {} ^/",
        k(7)?,
        k(7)?,
        k(1)?,
        k(7)?,
        k(2)?,
        k(3)?,
        k(4)?
    );
    for seed in 0_u8..16 {
        let envelope = round_trip_with(&f, &plain, seed, false)?;
        let mut pairs: Vec<(&str, &str)> = Vec::new();
        for (p, c) in plain.split(' ').zip(envelope.ciphertext.split(' ')) {
            if p.len() >= 32 {
                pairs.extend(atoms(p).into_iter().zip(atoms(c)));
            }
        }
        for (pa, ca) in &pairs {
            for (pb, cb) in &pairs {
                check(
                    &format!("seed {seed}: plain equality ⇔ cipher equality"),
                    pa == pb,
                    ca == cb,
                )?;
            }
        }
    }
    Ok(())
}

// ═══════════════════════════ Bound symbols (rules 1–2) ═══════════════════════════

/// A key outside the fixture snapshot (ephemeral session key).
fn ephemeral(tag: &str) -> Result<DtiKey, String> {
    key_from_tag(tag)
}

/// Cipher tokens at the positions where the plaintext token equals `token`.
fn tokens_at<'c>(plain: &str, cipher: &'c str, token: &str) -> Vec<&'c str> {
    plain
        .split(' ')
        .zip(cipher.split(' '))
        .filter(|(p, _)| *p == token)
        .map(|(_, c)| c)
        .collect()
}

fn all_equal(what: &str, tokens: &[&str], expected_count: usize) -> Result<String, String> {
    check(
        &format!("{what}: occurrences"),
        tokens.len(),
        expected_count,
    )?;
    let first = tokens.first().ok_or_else(|| format!("{what}: none"))?;
    if tokens.iter().any(|t| t != first) {
        return Err(format!("{what}: different cipher atoms {tokens:?}"));
    }
    Ok((*first).to_owned())
}

#[test]
fn def_name_references_and_operations_share_one_cipher_key() -> TestResult {
    let f = fixture(42)?;
    let (k1, k2, k3, k4) = (f.key(1)?, f.key(2)?, f.key(3)?, f.key(4)?);
    let variable = ephemeral("ghostvariable")?;
    for (name, is_ephemeral) in [(variable.as_str(), true), (f.key(9)?, false)] {
        let plain = format!(
            "/^ defu {name} a {k1} y ta {k2} nu {name} y defu {name} abfi {k3} y \
defu {name} absi {k4} y ta {k2} a {name} y defu {name} abse ~ ^/"
        );
        let mut changed = false;
        for seed in 0_u8..4 {
            for permute in [false, true] {
                let envelope = round_trip_with(&f, &plain, seed, permute)?;
                let cipher_name = all_equal(
                    "variable name",
                    &tokens_at(&plain, &envelope.ciphertext, name),
                    6,
                )?;
                changed |= cipher_name != name;
                let key = DtiKey::parse(&cipher_name).map_err(|e| e.to_string())?;
                let in_snapshot = f.dti.universal().concept(&key).is_some();
                check("ephemeral stays ephemeral", in_snapshot, !is_ephemeral)?;
                stage2_valid(&[&envelope.ciphertext])?;
                same_resolutions(&[&plain], &[&envelope.ciphertext])?;
            }
        }
        check("the variable name is encrypted", changed, true)?;
    }
    Ok(())
}

#[test]
fn anchors_rel_reloff_and_end_share_cipher_ids_atom_by_atom() -> TestResult {
    let f = fixture(42)?;
    let (k0, k1, k5) = (f.key(0)?, f.key(1)?, f.key(5)?);
    let port = ephemeral("ghostport")?;
    let p = port.as_str();
    let plains = [
        format!(
            "/^ ta {k0} : anchor {p}§{k5} : ta {k1} : rel {p}§{k5} : reloff {p}§{k5} : end ~ ^/"
        ),
        format!("/^ ta {k0} : anchorrel {p}§1 : ta {k1} : rel {p}§1 : end ~ ^/"),
        format!("/^ ta {k0} : anchor {k5} : ta {k1} : rel {k5} : end ~ ^/"),
    ];
    for plain in &plains {
        for seed in 0_u8..4 {
            for permute in [false, true] {
                let envelope = round_trip_with(&f, plain, seed, permute)?;
                let ids: Vec<&str> = plain
                    .split(' ')
                    .zip(envelope.ciphertext.split(' '))
                    .filter(|(p_token, _)| p_token.starts_with(p) || p_token.starts_with(k5))
                    .map(|(_, c)| c)
                    .collect();
                let first = ids.first().ok_or("no id")?;
                if ids.iter().any(|id| id != first) {
                    return Err(format!("anchor ids differ: {ids:?}"));
                }
                stage2_valid(&[&envelope.ciphertext])?;
                same_resolutions(&[plain.as_str()], &[&envelope.ciphertext])?;
            }
        }
    }
    Ok(())
}

/// v4.1: only the operators `~` and `~~` remain; they are structural tokens,
/// kept as written, and the ciphertext resolves like the plaintext.
#[test]
fn anaphora_operators_are_kept_and_resolve_alike() -> TestResult {
    let f = fixture(42)?;
    let k = (0..8).map(|i| f.key(i)).collect::<Result<Vec<_>, _>>()?;
    let key_at = |i: usize| k.get(i).copied().ok_or_else(|| format!("key {i}"));
    let plain = format!(
        "/^ ta {k0}§{k1} a {k2}§{k3} d {k4} : ta ~ d {k0}§{k1}§{k5} g ~~ ab {k6}§{k7} ^/",
        k0 = key_at(0)?,
        k1 = key_at(1)?,
        k2 = key_at(2)?,
        k3 = key_at(3)?,
        k4 = key_at(4)?,
        k5 = key_at(5)?,
        k6 = key_at(6)?,
        k7 = key_at(7)?,
    );
    for seed in 0_u8..8 {
        for permute in [false, true] {
            let envelope = round_trip_with(&f, &plain, seed, permute)?;
            let tokens: Vec<&str> = envelope.ciphertext.split(' ').collect();
            let token = |i: usize| tokens.get(i).copied().ok_or_else(|| format!("token {i}"));
            check("~ kept", token(9)?, "~")?;
            check("~~ kept", token(13)?, "~~")?;
            stage2_valid(&[&envelope.ciphertext])?;
            same_resolutions(&[&plain], &[&envelope.ciphertext])?;
        }
    }
    Ok(())
}

// ═══════════════════════════ Collision avoidance (rule 2) ═══════════════════════════

/// The chain walk on a toy permutation: an unbound atom whose base image is a
/// bound cipher atom is walked away from it, the result is a bijection, and the
/// inverse walk recovers every atom.
#[test]
fn chain_walk_avoids_bound_cipher_atoms_and_stays_bijective() -> TestResult {
    const SIZE: usize = 12;
    // τ(x) = x + 1 mod 12.
    let tau = |x: &usize| -> Result<usize, GhostError> { Ok((*x + 1) % SIZE) };
    let tau_inverse = |x: &usize| -> Result<usize, GhostError> { Ok((*x + SIZE - 1) % SIZE) };
    // 7 ↔ 3 (so τ(2) = 3 is taken), 8 ↔ 9 (so the walk τ(7) = 8 … continues).
    let pairs = [(7_usize, 3_usize), (4, 8)];
    let refs: Vec<(&usize, &usize)> = pairs.iter().map(|(p, c)| (p, c)).collect();
    // σ(2): τ(2) = 3 = c(7) → τ(7) = 8 = c(4) → τ(4) = 5 (free).
    check("walked image", forward(&2, &refs, tau), Ok(Image::Free(5)))?;
    check("bound atom", forward(&7, &refs, tau), Ok(Image::Bound(0)))?;
    let mut images = Vec::new();
    for x in 0..SIZE {
        let y = match forward(&x, &refs, tau).map_err(|e| e.to_string())? {
            Image::Bound(j) => pairs.get(j).map(|(_, c)| *c).ok_or("binding")?,
            Image::Free(y) => {
                if pairs.iter().any(|(_, c)| *c == y) {
                    return Err(format!("σ({x}) = {y} is a bound cipher atom"));
                }
                y
            }
        };
        let back = match inverse(&y, &refs, tau_inverse).map_err(|e| e.to_string())? {
            Image::Bound(j) => pairs.get(j).map(|(p, _)| *p).ok_or("binding")?,
            Image::Free(x) => x,
        };
        check(&format!("σ⁻¹(σ({x}))"), back, x)?;
        images.push(y);
    }
    images.sort_unstable();
    check("bijection", images, (0..SIZE).collect::<Vec<_>>())
}

/// Expected Fisher–Yates image of `plain_key` at `glyph-atom` position
/// `position`, chain-walked over `bindings` (plain, cipher).
fn expected_glyph(
    keystream: &Keystream<'_>,
    domain: &[DtiKey],
    position: u64,
    plain_key: &str,
    bindings: &[(&str, &str)],
) -> Result<String, String> {
    let index_of = |text: &str| -> Result<usize, String> {
        let parsed = DtiKey::parse(text).map_err(|e| e.to_string())?;
        domain
            .binary_search(&parsed)
            .map_err(|_| format!("{text} not in domain"))
    };
    let images = keystream
        .permutation(CLASS_GLYPH, position, domain.len())
        .map_err(|e| e.to_string())?;
    let tau = |index: usize| -> Result<usize, String> {
        images
            .get(index)
            .copied()
            .ok_or_else(|| "index outside the domain".to_owned())
    };
    let pairs = bindings
        .iter()
        .map(|(p, c)| Ok((index_of(p)?, index_of(c)?)))
        .collect::<Result<Vec<_>, String>>()?;
    let start = index_of(plain_key)?;
    let image = match pairs.iter().find(|(p, _)| *p == start) {
        Some((_, c)) => *c,
        None => {
            let mut y = tau(start)?;
            while let Some((p, _)) = pairs.iter().find(|(_, c)| *c == y) {
                y = tau(*p)?;
            }
            y
        }
    };
    Ok(domain
        .get(image)
        .ok_or("image outside the domain")?
        .as_str()
        .to_owned())
}

/// Rule 2 at period level: while a variable is live no other atom takes its
/// cipher key, also when the base permutation would have mapped it there.
#[test]
fn ordinary_atoms_never_take_a_live_bound_cipher_key() -> TestResult {
    let f = fixture(42)?;
    let k = |i: usize| f.key(i);
    let plain = format!(
        "/^ def {} a {} y ta {} a {} d {} g {} n {} ab {} ^/",
        k(0)?,
        k(1)?,
        k(2)?,
        k(3)?,
        k(4)?,
        k(5)?,
        k(6)?,
        k(7)?
    );
    let domain = domain(&f.dti);
    let key = master_key();
    let mut avoided = 0_usize;
    for seed in 0_u8..20 {
        let envelope = round_trip_with(&f, &plain, seed, false)?;
        let tokens: Vec<&str> = envelope.ciphertext.split(' ').collect();
        let name = *tokens.get(2).ok_or("name")?;
        let keystream = Keystream {
            suite: CryptoSuite::Shake256,
            key: &key,
            nonce: nonce(seed),
            snapshot_version: 42,
        };
        // Ordinary atoms of the relational block: glyph positions 2..8.
        for (position, token_index) in (2_u64..).zip([7_usize, 9, 11, 13, 15, 17]) {
            let cipher = *tokens.get(token_index).ok_or("token")?;
            if cipher == name {
                return Err(format!("seed {seed}: atom took the variable cipher key"));
            }
            let plain_atom = plain.split(' ').nth(token_index).ok_or("plain token")?;
            let unconstrained = expected_glyph(&keystream, &domain, position, plain_atom, &[])?;
            avoided += usize::from(unconstrained == name);
        }
    }
    // With a 20-key domain the base permutation hits the bound key several times.
    if avoided == 0 {
        return Err("the avoidance path was never exercised".to_owned());
    }
    Ok(())
}

// ═══════════════════════════ GhostSession (G8) ═══════════════════════════

fn session_pair() -> (GhostSession, GhostSession) {
    (
        GhostSession::new(GhostParams::default()),
        GhostSession::new(GhostParams::default()),
    )
}

#[test]
fn ghost_session_carries_variables_and_antecedents_across_periods() -> TestResult {
    let f = fixture(42)?;
    let k = |i: usize| f.key(i);
    let variable = ephemeral("ghostsessionvar")?;
    let d = variable.as_str();
    let periods = [
        format!("/^ defu {d} a {}§{} ^/", k(1)?, k(2)?),
        format!("/^ ta {} nu {d} a {}§{} /^/", k(3)?, k(4)?, k(5)?),
        format!(
            "/^ ta ~ a {}§{} d {}§{}§{} ^/",
            k(4)?,
            k(5)?,
            k(4)?,
            k(5)?,
            k(6)?
        ),
        format!("/^ defu {d} abfi {} ^^/", k(7)?),
    ];
    let key = master_key();
    let (mut sender, mut receiver) = session_pair();
    let mut ciphertexts = Vec::new();
    for (index, plain) in (0_u8..).zip(&periods) {
        let envelope = sender
            .encrypt(plain, &f.dti, &key, nonce(index))
            .map_err(|e| format!("encrypt {plain}: {e}"))?;
        check("sequence", envelope.sequence, u64::from(index))?;
        let decrypted = receiver
            .decrypt(&envelope, &f.dti, &key)
            .map_err(|e| format!("decrypt {}: {e}", envelope.ciphertext))?;
        check("round trip", decrypted.as_str(), plain.as_str())?;
        ciphertexts.push(envelope.ciphertext);
    }
    let cipher_refs: Vec<&str> = ciphertexts.iter().map(String::as_str).collect();
    let plain_refs: Vec<&str> = periods.iter().map(String::as_str).collect();
    stage2_valid(&cipher_refs)?;
    same_resolutions(&plain_refs, &cipher_refs)?;
    // The variable keeps its cipher key across periods 1, 2 and 4.
    let name_tokens: Vec<&str> = plain_refs
        .iter()
        .zip(&cipher_refs)
        .flat_map(|(p, c)| tokens_at(p, c, d))
        .collect();
    all_equal("variable across periods", &name_tokens, 3)?;
    // `^^/` cleared everything on both sides.
    for side in [&sender, &receiver] {
        check("table empty", side.symbols().is_empty(), true)?;
        check("no variable", side.symbols().variable_count(), 0)?;
        check("no carried block", side.symbols().carried_count(), 0)?;
        check("next sequence", side.next_sequence(), 4)?;
    }
    // After `^^/` the name is no longer a variable, and nothing is carried.
    expect_err(
        "variable after ^^/",
        sender.encrypt(
            &format!("/^ ta {} nu {d} ^/", k(8)?),
            &f.dti,
            &key,
            nonce(9),
        ),
        |e| matches!(e, GhostError::UnboundEphemeralKey { .. }),
    )?;
    expect_err(
        "anaphora after ^^/",
        sender.encrypt("/^ ta ~ ^/", &f.dti, &key, nonce(9)),
        |e| matches!(e, GhostError::Stage2 { .. }),
    )?;
    check("failures change nothing", sender.next_sequence(), 4)
}

/// G1 (option 2): pruned DSP branches leave the table at once; with `/^/`
/// only the level-0 blocks (and live variables) stay held; `^/` releases them.
#[test]
fn ghost_session_releases_pruned_and_closed_antecedents() -> TestResult {
    let f = fixture(42)?;
    let k = |i: usize| f.key(i);
    let key = master_key();
    let (mut sender, mut receiver) = session_pair();
    let plain_atoms = |session: &GhostSession| -> Vec<String> {
        let mut atoms: Vec<String> = session
            .symbols()
            .pairs()
            .map(|(plain, _)| plain.to_string())
            .collect();
        atoms.sort_unstable();
        atoms
    };
    let periods = [
        format!(
            "/^ ta {}§{} : ta {}§{} : ta {} y defu {} a {} /^/",
            k(0)?,
            k(5)?,
            k(1)?,
            k(2)?,
            k(3)?,
            k(8)?,
            k(9)?
        ),
        format!("/^ ta ~ a {} ^/", k(4)?),
    ];
    let mut expected_after = vec![
        // Level-0 block (k0§k5) carried, variable k8 = k9 live; the pruned branch
        // `ta k1§k2` and the depth-1 block `ta k3` are released.
        vec![
            k(0)?.to_owned(),
            k(5)?.to_owned(),
            k(8)?.to_owned(),
            k(9)?.to_owned(),
        ],
        // `^/` releases the carried block; the variable stays.
        vec![k(8)?.to_owned(), k(9)?.to_owned()],
    ];
    for row in &mut expected_after {
        row.sort_unstable();
    }
    for ((index, plain), expected) in (0_u8..).zip(&periods).zip(&expected_after) {
        let envelope = sender
            .encrypt(plain, &f.dti, &key, nonce(index))
            .map_err(|e| format!("{plain}: {e}"))?;
        let decrypted = receiver
            .decrypt(&envelope, &f.dti, &key)
            .map_err(|e| e.to_string())?;
        check("round trip", decrypted.as_str(), plain.as_str())?;
        check(
            "sender table",
            plain_atoms(&sender).as_slice(),
            expected.as_slice(),
        )?;
        check(
            "receiver table",
            plain_atoms(&receiver).as_slice(),
            expected.as_slice(),
        )?;
    }
    Ok(())
}

#[test]
fn ghost_session_rejects_out_of_order_replayed_and_relabelled_messages() -> TestResult {
    let f = fixture(42)?;
    let k = |i: usize| f.key(i);
    let key = master_key();
    let (mut sender, mut receiver) = session_pair();
    let first = sender
        .encrypt(
            &format!("/^ defu {} a {} ^/", k(0)?, k(1)?),
            &f.dti,
            &key,
            nonce(1),
        )
        .map_err(|e| e.to_string())?;
    let second = sender
        .encrypt(
            &format!("/^ ta {} nu {} ^/", k(2)?, k(0)?),
            &f.dti,
            &key,
            nonce(2),
        )
        .map_err(|e| e.to_string())?;
    expect_err(
        "future message",
        receiver.decrypt(&second, &f.dti, &key),
        |e| {
            *e == GhostError::OutOfOrder {
                expected: 0,
                found: 1,
            }
        },
    )?;
    check("nothing accepted", receiver.next_sequence(), 0)?;
    ghost(receiver.decrypt(&first, &f.dti, &key))?;
    expect_err("replay", receiver.decrypt(&first, &f.dti, &key), |e| {
        *e == GhostError::Replay
    })?;
    // The sequence number is authenticated.
    let relabelled = GhostEnvelope {
        sequence: 1,
        ..sender
            .encrypt(&format!("/^ ta {} ^/", k(3)?), &f.dti, &key, nonce(3))
            .map_err(|e| e.to_string())?
    };
    expect_err(
        "relabelled",
        receiver.decrypt(&relabelled, &f.dti, &key),
        |e| *e == GhostError::MacMismatch,
    )?;
    ghost(receiver.decrypt(&second, &f.dti, &key))?;
    check("in sync", receiver.next_sequence(), 2)
}

// ═══════════════════════════ Ephemeral keys (rule 5) ═══════════════════════════

#[test]
fn feistel_is_a_bijection_on_a_sample_with_valid_check_characters() -> TestResult {
    let key = master_key();
    let keystream = Keystream {
        suite: CryptoSuite::Shake256,
        key: &key,
        nonce: nonce(30),
        snapshot_version: 42,
    };
    let feistel = Feistel::new(&keystream, 3);
    let mut sample: Vec<DtiKey> = (0..150)
        .map(|i| key_from_tag(&format!("e{i:03}x")))
        .collect::<Result<_, _>>()?;
    sample.push(DtiKey::from_random_body([b'a'; 31]).map_err(|e| e.to_string())?);
    sample.push(DtiKey::from_random_body([b'z'; 31]).map_err(|e| e.to_string())?);
    let mut first = [b'9'; 31];
    if let Some(byte) = first.first_mut() {
        *byte = b'z';
    }
    sample.push(DtiKey::from_random_body(first).map_err(|e| e.to_string())?);
    let mut images = Vec::new();
    for plain in &sample {
        check("encoding round trip", encoding_round_trip(plain), Ok(true))?;
        let image = feistel
            .apply(plain, Direction::Forward)
            .map_err(|e| e.to_string())?;
        DtiKey::parse(image.as_str()).map_err(|e| format!("invalid image: {e}"))?;
        let back = feistel
            .apply(&image, Direction::Inverse)
            .map_err(|e| e.to_string())?;
        check("F⁻¹(F(x))", back, *plain)?;
        images.push(image);
    }
    let count = images.len();
    images.sort_unstable();
    images.dedup();
    check("injective on the sample", images.len(), count)
}

#[test]
fn cycle_walking_keeps_ephemeral_images_outside_the_snapshot() -> TestResult {
    let key = master_key();
    let keystream = Keystream {
        suite: CryptoSuite::Shake256,
        key: &key,
        nonce: nonce(31),
        snapshot_version: 42,
    };
    // A huge artificial "snapshot": every key whose first letter is below `t`.
    let in_snapshot = |k: &DtiKey| k.as_str().as_bytes().first().is_some_and(|b| *b < b't');
    for position in 0_u64..40 {
        let feistel = Feistel::new(&keystream, position);
        let plain = key_from_tag(&format!("zz{position:02}"))?;
        let image = feistel
            .permute(&plain, Direction::Forward, &in_snapshot)
            .map_err(|e| e.to_string())?;
        check("outside S", in_snapshot(&image), false)?;
        let back = feistel
            .permute(&image, Direction::Inverse, &in_snapshot)
            .map_err(|e| e.to_string())?;
        check("cycle-walk inverse", back, plain)?;
    }
    Ok(())
}

#[test]
fn ephemeral_key_outside_a_bound_position_is_rejected() -> TestResult {
    let f = fixture(42)?;
    let k0 = f.key(0)?;
    let stray = ephemeral("ghoststray")?;
    let e = stray.as_str();
    for text in [
        format!("/^ ta {e} ^/"),
        format!("/^ ta {k0}§{e} ^/"),
        format!("/^ def {k0} a {e} ^/"),
        format!("/^ ta {k0} a {} ^/", f.local),
    ] {
        expect_err(&text, encrypt_text(&f, &text), |err| {
            matches!(err, GhostError::UnboundEphemeralKey { .. })
        })?;
    }
    // The same key is accepted as a bound symbol.
    round_trip_with(
        &f,
        &format!("/^ def {e} a {k0} y ta {k0} a {e} ^/"),
        1,
        false,
    )
    .map(|_| ())
}

// ═══════════════════════════ Rejected inputs and markers (G5) ═══════════════════════════

fn encrypt_text(f: &Fixture, text: &str) -> Result<GhostEnvelope, GhostError> {
    encrypt_period(
        text,
        &f.dti,
        &master_key(),
        nonce(12),
        &GhostParams::default(),
    )
}

#[test]
fn rejects_labels_bad_keys_and_stage2_errors() -> TestResult {
    let f = fixture(42)?;
    let k0 = f.key(0)?;
    expect_err(
        "label (v4.1)",
        encrypt_text(&f, &format!("/^ ta {k0} a LVAAAAHello ^/")),
        |e| matches!(e, GhostError::Label { .. }),
    )?;
    expect_err(
        "quoted word is an ordinary glyph, not a key",
        encrypt_text(&f, &format!("/^ ta {k0} a \"Hello\" ^/")),
        |e| matches!(e, GhostError::InvalidKey { .. }),
    )?;
    expect_err(
        "plain word",
        encrypt_text(&f, &format!("/^ ta {k0} a green ^/")),
        |e| matches!(e, GhostError::InvalidKey { .. }),
    )?;
    expect_err(
        "digit-led non-numeral",
        encrypt_text(
            &f,
            &format!("/^ ta {k0} a 4{} ^/", k0.get(1..).unwrap_or("")),
        ),
        |e| matches!(e, GhostError::InvalidKey { .. }),
    )?;
    let mut broken_check = k0.to_owned();
    broken_check.pop();
    broken_check.push(if k0.ends_with('0') { '1' } else { '0' });
    expect_err(
        "bad check character",
        encrypt_text(&f, &format!("/^ ta {broken_check} ^/")),
        |e| matches!(e, GhostError::InvalidKey { .. }),
    )?;
    expect_err(
        "deprecated key",
        encrypt_text(&f, &format!("/^ ta {k0} a {} ^/", f.deprecated)),
        |e| matches!(e, GhostError::KeyNotInDomain { .. }),
    )?;
    expect_err(
        "bad key inside a def block",
        encrypt_text(&f, &format!("/^ def {k0} a green ^/")),
        |e| matches!(e, GhostError::InvalidKey { .. }),
    )?;
    expect_err(
        "unresolved anaphora",
        encrypt_text(&f, &format!("/^ ta {k0} a ~ ^/")),
        |e| matches!(e, GhostError::Stage2 { .. }),
    )?;
    expect_err(
        "abfi on an undefined variable",
        encrypt_text(&f, &format!("/^ def {k0} abfi {k0} ^/")),
        |e| matches!(e, GhostError::Stage2 { .. }),
    )?;
    // An unknown key as portability id is an ephemeral bound symbol.
    let unknown = key_from_tag("notindti")?;
    round_trip_with(
        &f,
        &format!("/^ ta {k0} : anchor {unknown} : end ~ ^/"),
        1,
        false,
    )
    .map(|_| ())
}

#[test]
fn rejects_non_single_periods_and_syntax_errors() -> TestResult {
    let f = fixture(42)?;
    let k0 = f.key(0)?;
    let k1 = f.key(1)?;
    expect_err(
        "two periods",
        encrypt_text(&f, &format!("/^ ta {k0} ^/ /^ ta {k1} ^/")),
        |e| *e == GhostError::PeriodCount { found: 2 },
    )?;
    expect_err(
        "syntax error",
        encrypt_text(&f, &format!("/^ ta {k0} a ^/")),
        |e| matches!(e, GhostError::Stage1 { .. }),
    )?;
    expect_err(
        "double space",
        encrypt_text(&f, &format!("/^  ta {k0} ^/")),
        |e| matches!(e, GhostError::Stage1 { .. }),
    )?;
    expect_err("empty", encrypt_text(&f, ""), |e| {
        matches!(e, GhostError::Stage1 { .. })
    })
}

#[test]
fn bare_anaphora_markers_are_left_untouched() -> TestResult {
    let f = fixture(42)?;
    let (k0, k1) = (f.key(0)?, f.key(1)?);
    let plain = format!("/^ ta {k0} a {k1} : ta ~ d ~~ ^/");
    let envelope = round_trip_with(&f, &plain, 12, false)?;
    let tokens: Vec<&str> = envelope.ciphertext.split(' ').collect();
    check("~ kept", tokens.get(7).copied(), Some("~"))?;
    check("~~ kept", tokens.get(9).copied(), Some("~~"))
}

// ═══════════════════════════ Numerals (G5) ═══════════════════════════

fn round_trip_with(
    f: &Fixture,
    plain: &str,
    seed: u8,
    permute: bool,
) -> Result<GhostEnvelope, String> {
    let p = params(CryptoSuite::Shake256, permute);
    let envelope = encrypt_period(plain, &f.dti, &master_key(), nonce(seed), &p)
        .map_err(|e| format!("encrypt {plain}: {e}"))?;
    ghost(parse_single(&envelope.ciphertext))?;
    stage2_valid(&[&envelope.ciphertext])?;
    let mut replay = window()?;
    let decrypted = decrypt_period(&envelope, &f.dti, &master_key(), &p, &mut replay)
        .map_err(|e| format!("decrypt {}: {e}", envelope.ciphertext))?;
    check(&format!("round trip of {plain}"), decrypted.as_str(), plain)?;
    Ok(envelope)
}

/// Splits a glyph token into its atoms (trailing `~`/`~~` removed).
fn atoms(token: &str) -> Vec<&str> {
    token.trim_end_matches('~').split('§').collect()
}

fn is_numeral_below(atom: &str, base: u8) -> bool {
    !atom.is_empty()
        && atom
            .bytes()
            .all(|b| b.is_ascii_digit() && b.wrapping_sub(b'0') < base)
}

#[test]
fn numeral_digits_round_trip_with_same_length_and_base() -> TestResult {
    for base in [10_u8, 8, 2] {
        let f = fixture_with(42, Some(base))?;
        let k0 = f.key(0)?;
        let k1 = f.key(1)?;
        let digits: String = (0..base)
            .map(|d| char::from(b'0'.wrapping_add(d)))
            .collect();
        let plain = format!("/^ ta {k0} a 1 d {k1}§10 g {digits} : ta ~ n ~~ ^/");
        let mut changed = false;
        for seed in 0_u8..6 {
            for permute in [false, true] {
                let envelope = round_trip_with(&f, &plain, seed, permute)?;
                same_resolutions(&[&plain], &[&envelope.ciphertext])?;
                let plain_tokens: Vec<&str> = plain.split(' ').collect();
                let cipher_tokens: Vec<&str> = envelope.ciphertext.split(' ').collect();
                check("token count", cipher_tokens.len(), plain_tokens.len())?;
                for (p, c) in plain_tokens.iter().zip(&cipher_tokens) {
                    let (p_atoms, c_atoms) = (atoms(p), atoms(c));
                    check("atom count", c_atoms.len(), p_atoms.len())?;
                    for (pa, ca) in p_atoms.iter().zip(&c_atoms) {
                        if is_numeral_below(pa, base) {
                            check(&format!("length of {pa}"), ca.len(), pa.len())?;
                            check(
                                &format!("{ca} is a base-{base} numeral"),
                                is_numeral_below(ca, base),
                                true,
                            )?;
                            changed |= pa != ca;
                        }
                    }
                }
            }
        }
        check(&format!("base {base}: some numeral changed"), changed, true)?;
    }
    Ok(())
}

/// The digits use the `numeral-digit` class with one position per digit,
/// numerals never consume `glyph-atom` positions, and the first atom of a
/// period (empty symbol table) is the plain Fisher–Yates image.
#[test]
fn numeral_digit_positions_follow_the_class_scheme() -> TestResult {
    let f = fixture(42)?;
    let k0 = f.key(0)?;
    let k1 = f.key(1)?;
    let plain = format!("/^ ta {k0} a 42 d {k1}§7 ^/");
    let envelope = encrypt_period(
        &plain,
        &f.dti,
        &master_key(),
        nonce(21),
        &GhostParams::default(),
    )
    .map_err(|e| e.to_string())?;
    let key = master_key();
    let keystream = Keystream {
        suite: CryptoSuite::Shake256,
        key: &key,
        nonce: nonce(21),
        snapshot_version: 42,
    };
    let digit = |position: u64, plain_digit: usize| -> Result<String, String> {
        let images = keystream
            .permutation(CLASS_NUMERAL_DIGIT, position, 10)
            .map_err(|e| e.to_string())?;
        let image = images.get(plain_digit).ok_or("digit outside the base")?;
        Ok(image.to_string())
    };
    let domain = domain(&f.dti);
    let c0 = expected_glyph(&keystream, &domain, 0, k0, &[])?;
    // `k1` is mapped while `k0 ↔ c0` is held by the block being written.
    let c1 = expected_glyph(&keystream, &domain, 1, k1, &[(k0, &c0)])?;
    let expected = format!(
        "/^ ta {c0} a {}{} d {c1}§{} ^/",
        digit(0, 4)?,
        digit(1, 2)?,
        digit(2, 7)?,
    );
    check("ciphertext", envelope.ciphertext, expected)
}

#[test]
fn numeral_without_numeral_system_is_rejected() -> TestResult {
    let f = fixture_with(42, None)?;
    let k0 = f.key(0)?;
    let text = format!("/^ ta {k0} a 42 ^/");
    let start = u32::try_from(text.find("42").ok_or("no numeral")?).map_err(|e| e.to_string())?;
    expect_err("numeral", encrypt_text(&f, &text), |e| {
        *e == GhostError::NoNumeralSystem {
            span: Span::new(start, start.saturating_add(2)),
        }
    })?;
    expect_err(
        "numeral qualifier in a def value",
        encrypt_text(&f, &format!("/^ def {k0} a {k0}§7 ^/")),
        |e| matches!(e, GhostError::NoNumeralSystem { .. }),
    )?;
    // Without numerals the period still encrypts.
    round_trip_with(&f, &format!("/^ ta {k0} ^/"), 1, false).map(|_| ())
}

#[test]
fn numeral_digit_not_below_the_base_is_rejected() -> TestResult {
    let f = fixture_with(42, Some(8))?;
    let k0 = f.key(0)?;
    let text = format!("/^ ta {k0} a 1289 ^/");
    let start = u32::try_from(text.find("1289").ok_or("no numeral")?.saturating_add(2))
        .map_err(|e| e.to_string())?;
    expect_err("digit 8 in base 8", encrypt_text(&f, &text), |e| {
        *e == GhostError::DigitOutOfBase {
            span: Span::new(start, start.saturating_add(1)),
            digit: 8,
            base: 8,
        }
    })?;
    expect_err(
        "digit 9 in a portability id",
        encrypt_text(&f, &format!("/^ ta {k0} : anchor {k0}§9 : end ~ ^/")),
        |e| {
            matches!(
                e,
                GhostError::DigitOutOfBase {
                    digit: 9,
                    base: 8,
                    ..
                }
            )
        },
    )
}

// ═══════════════════════════ Grammar tiles (G5, G2 option 1) ═══════════════════════════

/// Relational signal tiles of a period, in textual order.
fn signals(text: &str) -> Result<Vec<RelationalSignal>, String> {
    let period = parse_single(text).map_err(|e| e.to_string())?;
    let blocks =
        std::iter::once(&period.head).chain(period.tail.iter().filter_map(|linked| match linked {
            crate::ast::LinkedClause::Clause { block, .. } => Some(block),
            crate::ast::LinkedClause::Portability { .. } => None,
        }));
    Ok(blocks
        .filter_map(|block| match block {
            ClauseBlock::Relational(relational) => Some(relational.signal.tile.node),
            ClauseBlock::Definition(_) => None,
        })
        .collect())
}

#[test]
fn signal_tile_numbers_round_trip_with_grammar_permutation() -> TestResult {
    let f = fixture(42)?;
    let k0 = f.key(0)?;
    let k1 = f.key(1)?;
    let k2 = f.key(2)?;
    let plains = [
        format!("/^ tau {k0} a {k1} ^/"),
        format!("/^ deepallnum {k0} a {k1} ^/"),
        format!("/^ tam {k0} : quatoaltu {k1} y deepzatonum {k2} : ta {k0} ^/"),
        format!("/^ kasomerefu {k0} d {k1} ^/"),
    ];
    let mut changed = false;
    for plain in &plains {
        let before: Vec<_> = signals(plain)?
            .iter()
            .map(|s| s.determiners().number)
            .collect();
        for seed in 0_u8..12 {
            // Without the flag the signal tiles are untouched.
            let fixed = round_trip_with(&f, plain, seed, false)?;
            let kept: Vec<_> = signals(&fixed.ciphertext)?
                .iter()
                .map(|s| s.determiners().number)
                .collect();
            check("numbers kept", kept.as_slice(), before.as_slice())?;
            let permuted = round_trip_with(&f, plain, seed, true)?;
            let after: Vec<_> = signals(&permuted.ciphertext)?
                .iter()
                .map(|s| s.determiners().number)
                .collect();
            check("signal count", after.len(), before.len())?;
            changed |= after != before;
        }
    }
    check("some signal number changed", changed, true)
}

/// G2 (option 1): `oh` is never permuted, the primary stays within
/// `{ta, to, ka, ko, ot}`, and the tense stays within its partition (the bare
/// loop marker `abdur ~` keeps its written conditional), so the ciphertext is
/// stage-2 valid.
#[test]
fn grammar_permutation_preserves_oh_and_the_tense_partition() -> TestResult {
    let f = fixture(42)?;
    let (k0, k1, k2) = (f.key(0)?, f.key(1)?, f.key(2)?);
    let plain = format!("/^ ohalt {k0} a {k1} : tazato {k2} abdur ~ y ta {k1} ^/");
    stage2_valid(&[&plain])?;
    let mut primary_changed = false;
    for seed in 0_u8..16 {
        let envelope = round_trip_with(&f, &plain, seed, true)?;
        let before = signals(&plain)?;
        let after = signals(&envelope.ciphertext)?;
        check("tile count", after.len(), before.len())?;
        for (b, a) in before.iter().zip(&after) {
            match (b, a) {
                (
                    RelationalSignal::Primary {
                        signal: pb,
                        determiners: db,
                    },
                    RelationalSignal::Primary {
                        signal: pa,
                        determiners: da,
                    },
                ) => {
                    check(
                        "oh fixed",
                        *pb == PrimarySignal::Oh,
                        *pa == PrimarySignal::Oh,
                    )?;
                    primary_changed |= pb != pa;
                    let conditional =
                        |t: Option<TemporalMode>| matches!(t, Some(TemporalMode::Conditional(_)));
                    check(
                        "tense partition",
                        conditional(db.tense),
                        conditional(da.tense),
                    )?;
                }
                _ => return Err(format!("unexpected tiles {b:?} / {a:?}")),
            }
        }
        same_resolutions(&[&plain], &[&envelope.ciphertext])?;
    }
    check("some primary changed", primary_changed, true)
}

/// Every image of the signal-tile permutation (primary × tense × number, and
/// `deep` × tense × number) prints to a tile that reparses to the same signal,
/// so `permute_grammar_tiles` can never produce an unparsable tile.
#[test]
fn every_permutable_signal_tile_reparses_identically() -> TestResult {
    let f = fixture(42)?;
    let (k0, k1) = (f.key(0)?, f.key(1)?);
    let mut all = Vec::new();
    for tense in all_tenses() {
        for number in NUMBERS {
            let determiners = SignalDeterminers {
                tense,
                number,
                ..SignalDeterminers::default()
            };
            all.push(RelationalSignal::Deep { determiners });
            for primary in PrimarySignal::ALL {
                all.push(RelationalSignal::Primary {
                    signal: primary,
                    determiners,
                });
                all.push(RelationalSignal::Secondary {
                    secondary: SecondarySignal::Que,
                    primary,
                    determiners,
                });
            }
        }
    }
    for signal in all {
        let tile = crate::printer::signal(&signal);
        let period = parse_single(&format!("/^ {tile} {k0} a {k1} ^/"))
            .map_err(|e| format!("{tile}: {e}"))?;
        match &period.head {
            ClauseBlock::Relational(relational) => {
                check(
                    &format!("reparse of {tile}"),
                    relational.signal.tile.node,
                    signal,
                )?;
            }
            ClauseBlock::Definition(_) => return Err(format!("{tile} parsed as def")),
        }
    }
    Ok(())
}

// ═══════════════════════════ Integrity and replay (G3, G6) ═══════════════════════════

fn sample_envelope(f: &Fixture, seed: u8) -> Result<GhostEnvelope, String> {
    encrypt_period(
        &f.sample()?,
        &f.dti,
        &master_key(),
        nonce(seed),
        &GhostParams::default(),
    )
    .map_err(|e| e.to_string())
}

fn decrypt_fresh(f: &Fixture, envelope: &GhostEnvelope) -> Result<String, GhostError> {
    let mut replay = NonZeroUsize::new(4)
        .map(ReplayWindow::new)
        .ok_or(GhostError::Internal {
            context: "capacity",
        })?;
    decrypt_period(
        envelope,
        &f.dti,
        &master_key(),
        &GhostParams::default(),
        &mut replay,
    )
}

#[test]
fn mac_rejects_tampered_ciphertext_nonce_and_tag() -> TestResult {
    let f = fixture(42)?;
    let original = sample_envelope(&f, 7)?;
    ghost(decrypt_fresh(&f, &original))?;

    // Ciphertext: swap the first glyph for another valid domain key (still a valid period).
    let first_glyph = original
        .ciphertext
        .split(' ')
        .nth(2)
        .ok_or("no glyph token")?;
    let replacement = f
        .keys
        .iter()
        .map(DtiKey::as_str)
        .find(|k| *k != first_glyph)
        .ok_or("no replacement key")?;
    let tampered = GhostEnvelope {
        ciphertext: original.ciphertext.replacen(first_glyph, replacement, 1),
        ..sample_envelope(&f, 7)?
    };
    ghost(parse_single(&tampered.ciphertext))?;
    expect_err("ciphertext", decrypt_fresh(&f, &tampered), |e| {
        *e == GhostError::MacMismatch
    })?;

    // Nonce.
    let tampered = GhostEnvelope {
        nonce: nonce(8),
        ..sample_envelope(&f, 7)?
    };
    expect_err("nonce", decrypt_fresh(&f, &tampered), |e| {
        *e == GhostError::MacMismatch
    })?;

    // Tag: flip each byte in turn.
    for position in 0..original.mac_tag.len() {
        let mut tag = original.mac_tag;
        if let Some(byte) = tag.get_mut(position) {
            *byte ^= 0x01;
        }
        let tampered = GhostEnvelope {
            mac_tag: tag,
            ..sample_envelope(&f, 7)?
        };
        expect_err("tag", decrypt_fresh(&f, &tampered), |e| {
            *e == GhostError::MacMismatch
        })?;
    }

    // The grammar-tile flag is authenticated too.
    let mut replay = window()?;
    expect_err(
        "flag mismatch",
        decrypt_period(
            &original,
            &f.dti,
            &master_key(),
            &params(CryptoSuite::Shake256, true),
            &mut replay,
        ),
        |e| *e == GhostError::MacMismatch,
    )?;

    // Wrong key.
    expect_err(
        "wrong key",
        decrypt_period(
            &original,
            &f.dti,
            &MasterKey::new([7; 32]),
            &GhostParams::default(),
            &mut replay,
        ),
        |e| *e == GhostError::MacMismatch,
    )?;
    check("failed attempts record nothing", replay.is_empty(), true)
}

#[test]
fn replayed_nonce_is_rejected_after_success_only() -> TestResult {
    let f = fixture(42)?;
    let envelope = sample_envelope(&f, 10)?;
    let key = master_key();
    let p = GhostParams::default();
    let mut replay = window()?;

    // A forged envelope with the same nonce does not burn the nonce.
    let forged = GhostEnvelope {
        mac_tag: [0; 32],
        ..sample_envelope(&f, 10)?
    };
    expect_err(
        "forged",
        decrypt_period(&forged, &f.dti, &key, &p, &mut replay),
        |e| *e == GhostError::MacMismatch,
    )?;
    check(
        "nonce not recorded",
        replay.contains(&envelope.nonce),
        false,
    )?;

    ghost(decrypt_period(&envelope, &f.dti, &key, &p, &mut replay))?;
    check("nonce recorded", replay.contains(&envelope.nonce), true)?;
    expect_err(
        "replay",
        decrypt_period(&envelope, &f.dti, &key, &p, &mut replay),
        |e| *e == GhostError::Replay,
    )
}

#[test]
fn replay_window_evicts_in_fifo_order() -> TestResult {
    let mut replay = NonZeroUsize::new(2)
        .map(ReplayWindow::new)
        .ok_or("capacity")?;
    check("first", replay.record(nonce(1)), true)?;
    check("duplicate", replay.record(nonce(1)), false)?;
    check("second", replay.record(nonce(2)), true)?;
    check("third", replay.record(nonce(3)), true)?;
    check("len", replay.len(), 2)?;
    check("oldest evicted", replay.contains(&nonce(1)), false)?;
    check("kept 2", replay.contains(&nonce(2)), true)?;
    check("kept 3", replay.contains(&nonce(3)), true)?;
    check("capacity", replay.capacity().get(), 2)
}

#[test]
fn wrong_snapshot_version_or_suite_is_rejected() -> TestResult {
    let f = fixture(42)?;
    let envelope = sample_envelope(&f, 11)?;
    let key = master_key();
    let mut replay = window()?;

    // Same keys, newer snapshot.
    let newer = fixture(43)?;
    expect_err(
        "newer snapshot",
        decrypt_period(
            &envelope,
            &newer.dti,
            &key,
            &GhostParams::default(),
            &mut replay,
        ),
        |e| {
            *e == GhostError::SnapshotVersionMismatch {
                expected: 43,
                found: 42,
            }
        },
    )?;
    // Envelope claiming another version.
    let relabelled = GhostEnvelope {
        snapshot_version: 41,
        ..sample_envelope(&f, 11)?
    };
    expect_err(
        "relabelled envelope",
        decrypt_period(
            &relabelled,
            &f.dti,
            &key,
            &GhostParams::default(),
            &mut replay,
        ),
        |e| {
            *e == GhostError::SnapshotVersionMismatch {
                expected: 42,
                found: 41,
            }
        },
    )?;
    // Suite mismatch between envelope and session.
    let other_suite = GhostEnvelope {
        suite: CryptoSuite::Blake3,
        ..sample_envelope(&f, 11)?
    };
    expect_err(
        "suite mismatch",
        decrypt_period(
            &other_suite,
            &f.dti,
            &key,
            &GhostParams::default(),
            &mut replay,
        ),
        |e| {
            *e == GhostError::SuiteMismatch {
                expected: CryptoSuite::Shake256,
                found: CryptoSuite::Blake3,
            }
        },
    )?;
    check("nothing recorded", replay.is_empty(), true)
}

#[test]
fn tags_compare_in_constant_time_semantics() -> TestResult {
    let a = [3_u8; 32];
    let mut b = a;
    check("equal", tags_equal(&a, &b), true)?;
    if let Some(last) = b.last_mut() {
        *last ^= 0x80;
    }
    check("last byte differs", tags_equal(&a, &b), false)?;
    let mut c = a;
    if let Some(first) = c.first_mut() {
        *first ^= 0x01;
    }
    check("first byte differs", tags_equal(&a, &c), false)
}

// ═══════════════════════════ Keys and KDF (G1, G7) ═══════════════════════════

#[test]
fn master_key_debug_does_not_leak() -> TestResult {
    let rendered = format!("{:?}", MasterKey::new([0xAB; 32]));
    check("redacted", rendered.as_str(), "MasterKey(<redacted>)")
}

/// Python reference:
/// `shake_256(f(b'ApriWave-Ghost-v2/kdf-shake256') + f(b'correct horse battery staple')
/// + f(bytes(range(16)))).hexdigest(32)` with `f(b) = struct.pack('<Q', len(b)) + b`.
#[test]
fn shake256_key_derivation_matches_python_reference() -> TestResult {
    let mut salt = [0_u8; 16];
    for (byte, value) in salt.iter_mut().zip(0_u8..) {
        *byte = value;
    }
    let mut out = [0_u8; 32];
    Shake256KeyDerivation
        .derive(b"correct horse battery staple", &salt, &mut out)
        .map_err(|e| e.to_string())?;
    check(
        "kdf",
        hex(&out).as_str(),
        "00dbd7de17103c33eb2ec90f5ee0f1c37debd974bd79d023b5279e5bf366851f",
    )?;
    // Usable as a MasterKey source in the default build.
    let f = fixture(42)?;
    let key = MasterKey::derive(&Shake256KeyDerivation, b"high entropy secret", &salt)
        .map_err(|e| e.to_string())?;
    let envelope = encrypt_period(
        &f.sample()?,
        &f.dti,
        &key,
        nonce(13),
        &GhostParams::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut replay = window()?;
    let decrypted = decrypt_period(
        &envelope,
        &f.dti,
        &key,
        &GhostParams::default(),
        &mut replay,
    )
    .map_err(|e| e.to_string())?;
    check("round trip", decrypted, f.sample()?)
}

#[test]
fn argon2id_presets_follow_rfc9106() -> TestResult {
    let second = Argon2idKdf::default();
    check(
        "default = second option",
        second,
        Argon2idKdf::rfc9106_second_recommended(),
    )?;
    check(
        "second option",
        (second.t_cost(), second.p_cost(), second.m_cost_kib()),
        (3, 4, 65_536),
    )?;
    let first = Argon2idKdf::rfc9106_first_recommended();
    check(
        "first option",
        (first.t_cost(), first.p_cost(), first.m_cost_kib()),
        (1, 4, 2_097_152),
    )?;
    check("salt", Argon2idKdf::SALT_LENGTH, 16)?;
    check("tag", Argon2idKdf::TAG_LENGTH, 32)
}

#[cfg(not(feature = "ghost-argon2"))]
#[test]
fn argon2id_without_feature_is_unavailable() -> TestResult {
    let mut out = [0_u8; 32];
    check(
        "unavailable",
        Argon2idKdf::default().derive(b"pw", &[0; 16], &mut out),
        Err(GhostError::KdfUnavailable),
    )
}

#[cfg(feature = "ghost-argon2")]
#[test]
fn argon2id_derives_deterministic_salted_keys() -> TestResult {
    // Small parameters keep the debug-mode test fast.
    let kdf = Argon2idKdf::with_params(1, 1, 64);
    let salt_a = [1_u8; 16];
    let salt_b = [2_u8; 16];
    let mut a1 = [0_u8; 32];
    let mut a2 = [0_u8; 32];
    let mut b = [0_u8; 32];
    ghost(kdf.derive(b"passphrase", &salt_a, &mut a1))?;
    ghost(kdf.derive(b"passphrase", &salt_a, &mut a2))?;
    ghost(kdf.derive(b"passphrase", &salt_b, &mut b))?;
    check("deterministic", a1, a2)?;
    if a1 == b {
        return Err("salt has no effect".to_owned());
    }
    check(
        "short salt",
        kdf.derive(b"passphrase", &[0; 8], &mut b),
        Err(GhostError::SaltTooShort {
            minimum: 16,
            found: 8,
        }),
    )?;
    expect_err(
        "invalid params",
        Argon2idKdf::with_params(0, 1, 64).derive(b"pw", &salt_a, &mut b),
        |e| matches!(e, GhostError::Kdf { .. }),
    )?;
    // End to end with a derived key.
    let f = fixture(42)?;
    let key = MasterKey::derive(&kdf, b"passphrase", &salt_a).map_err(|e| e.to_string())?;
    let envelope = encrypt_period(
        &f.sample()?,
        &f.dti,
        &key,
        nonce(14),
        &GhostParams::default(),
    )
    .map_err(|e| e.to_string())?;
    let mut replay = window()?;
    let decrypted = decrypt_period(
        &envelope,
        &f.dti,
        &key,
        &GhostParams::default(),
        &mut replay,
    )
    .map_err(|e| e.to_string())?;
    check("round trip", decrypted, f.sample()?)
}

#[test]
fn errors_render_human_readable_messages() -> TestResult {
    let samples = [
        GhostError::SuiteUnavailable {
            suite: CryptoSuite::Ascon,
        },
        GhostError::MacMismatch,
        GhostError::Replay,
        GhostError::SnapshotVersionMismatch {
            expected: 1,
            found: 2,
        },
        GhostError::KdfUnavailable,
        GhostError::NoNumeralSystem {
            span: Span::new(3, 5),
        },
        GhostError::DigitOutOfBase {
            span: Span::new(4, 5),
            digit: 9,
            base: 8,
        },
    ];
    for error in samples {
        if error.to_string().is_empty() {
            return Err(format!("empty message for {error:?}"));
        }
    }
    Ok(())
}
