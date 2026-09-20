use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use cryptoki::{
    context::{CInitializeArgs, CInitializeFlags, Pkcs11},
    mechanism::{eddsa::EddsaParams, eddsa::EddsaSignatureScheme, Mechanism, MechanismType},
    object::{Attribute, AttributeType, KeyType, ObjectClass, ObjectHandle},
    session::{Session, UserType},
    slot::Slot,
    types::AuthPin,
};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::Serialize;

use crate::{protocol, KeyConfig, Region, SignerConfig, SignerError, SignerResult};

// SoftHSM uses the PKCS#11-defined PrintableString form for edwards25519.
const ED25519_EC_PARAMS: &[u8] = b"\x13\x0cedwards25519";

#[derive(Clone, Debug, Serialize)]
pub struct PublicKeyRecord {
    pub kid: String,
    pub public_key_ed25519: String,
}

#[derive(Debug, Serialize)]
pub struct PublicKeySet {
    pub version: u8,
    pub keys: Vec<PublicKeyRecord>,
}

struct KeyPair {
    private: ObjectHandle,
    public: VerifyingKey,
}

pub struct KeyStore {
    _context: Pkcs11,
    session: Session,
    active_private: ObjectHandle,
    active_public: VerifyingKey,
    key_pairs: Vec<(String, ObjectHandle, VerifyingKey)>,
    public_keys: Vec<PublicKeyRecord>,
    region: Region,
    active_kid: String,
}

impl KeyStore {
    pub fn initialize_token(
        config: &SignerConfig,
        user_pin: &[u8],
        so_pin: &[u8],
    ) -> SignerResult<()> {
        let context = Pkcs11::new(&config.pkcs11_module_path).map_err(pkcs11_error)?;
        context
            .initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
            .map_err(pkcs11_error)?;
        let mut uninitialized = Vec::new();
        let mut initialized = 0_usize;
        for slot in context.get_slots_with_token().map_err(pkcs11_error)? {
            let info = context.get_token_info(slot).map_err(pkcs11_error)?;
            if info.label() == config.token_label {
                return Err(SignerError::Pkcs11(
                    "token initialization refuses to overwrite the configured token label".into(),
                ));
            }
            if !info.token_initialized() {
                uninitialized.push(slot);
            } else {
                initialized += 1;
            }
        }
        if initialized != 0 || uninitialized.len() != 1 {
            return Err(SignerError::Pkcs11(
                "token initialization requires an empty store with exactly one uninitialized slot"
                    .into(),
            ));
        }
        let so_pin = auth_pin(so_pin)?;
        let user_pin = auth_pin(user_pin)?;
        let slot = uninitialized[0];
        context
            .init_token(slot, &so_pin, &config.token_label)
            .map_err(pkcs11_error)?;
        let session = context.open_rw_session(slot).map_err(pkcs11_error)?;
        session
            .login(UserType::So, Some(&so_pin))
            .map_err(pkcs11_error)?;
        session.init_pin(&user_pin).map_err(pkcs11_error)?;
        Ok(())
    }

    pub fn open(config: &SignerConfig, pin: &[u8]) -> SignerResult<Self> {
        let (context, slot) = open_context(config)?;
        let session = login(&context, slot, pin, false)?;
        let mut active = None;
        let mut public_keys = Vec::with_capacity(config.keys.len());
        let mut key_pairs = Vec::with_capacity(config.keys.len());
        for key in &config.keys {
            let pair = load_pair(&session, key)?;
            if key.kid == config.active_kid {
                active = Some((pair.private, pair.public));
            }
            public_keys.push(public_record(key, &pair.public));
            key_pairs.push((key.kid.clone(), pair.private, pair.public));
        }
        let (active_private, active_public) = active.ok_or_else(|| {
            SignerError::Pkcs11("active key is missing after key validation".into())
        })?;
        let store = Self {
            _context: context,
            session,
            active_private,
            active_public,
            key_pairs,
            public_keys,
            region: config.region,
            active_kid: config.active_kid.clone(),
        };
        store.readiness_canary()?;
        Ok(store)
    }

    pub fn provision(
        config: &SignerConfig,
        pin: &[u8],
        key: &KeyConfig,
    ) -> SignerResult<PublicKeyRecord> {
        if config.key(&key.kid).is_none() {
            return Err(SignerError::Config(
                "provisioning kid is not in the configured rotation pair".into(),
            ));
        }
        let (context, slot) = open_context(config)?;
        let session = login(&context, slot, pin, true)?;
        require_id_and_label_absent(&session, key)?;
        let id = key.object_id()?;
        let label = key.kid.as_bytes().to_vec();
        let public_template = [
            Attribute::Token(true),
            Attribute::Private(false),
            Attribute::Label(label.clone()),
            Attribute::Id(id.clone()),
            Attribute::Verify(true),
            Attribute::Derive(false),
            Attribute::EcParams(ED25519_EC_PARAMS.to_vec()),
        ];
        let private_template = [
            Attribute::Token(true),
            Attribute::Private(true),
            Attribute::Label(label),
            Attribute::Id(id),
            Attribute::Sensitive(true),
            Attribute::Extractable(false),
            Attribute::Sign(true),
            Attribute::Derive(false),
        ];
        session
            .generate_key_pair(
                &Mechanism::EccEdwardsKeyPairGen,
                &public_template,
                &private_template,
            )
            .map_err(pkcs11_error)?;
        let pair = load_pair(&session, key)?;
        let canonical = protocol::canary(config.region, &key.kid)?;
        sign_and_verify(&session, pair.private, &pair.public, &canonical)?;
        Ok(public_record(key, &pair.public))
    }

    pub fn sign_locator(&self, canonical: &[u8; 268]) -> SignerResult<[u8; 64]> {
        protocol::validate_locator_domain(canonical, self.region, &self.active_kid)?;
        sign_and_verify(
            &self.session,
            self.active_private,
            &self.active_public,
            canonical,
        )
    }

    pub fn readiness_canary(&self) -> SignerResult<()> {
        for (kid, private, public) in &self.key_pairs {
            let canonical = protocol::canary(self.region, kid)?;
            sign_and_verify(&self.session, *private, public, &canonical)?;
        }
        Ok(())
    }

    pub fn public_key_set(&self) -> PublicKeySet {
        PublicKeySet {
            version: 1,
            keys: self.public_keys.clone(),
        }
    }
}

fn open_context(config: &SignerConfig) -> SignerResult<(Pkcs11, Slot)> {
    let context = Pkcs11::new(&config.pkcs11_module_path).map_err(pkcs11_error)?;
    context
        .initialize(CInitializeArgs::new(CInitializeFlags::OS_LOCKING_OK))
        .map_err(pkcs11_error)?;
    let mut matches = Vec::new();
    for slot in context
        .get_slots_with_initialized_token()
        .map_err(pkcs11_error)?
    {
        let info = context.get_token_info(slot).map_err(pkcs11_error)?;
        if info.label() == config.token_label {
            matches.push(slot);
        }
    }
    if matches.len() != 1 {
        return Err(SignerError::Pkcs11(
            "token_label must select exactly one initialized token".into(),
        ));
    }
    let slot = matches[0];
    let generation = context
        .get_mechanism_info(slot, MechanismType::ECC_EDWARDS_KEY_PAIR_GEN)
        .map_err(pkcs11_error)?;
    let signing = context
        .get_mechanism_info(slot, MechanismType::EDDSA)
        .map_err(pkcs11_error)?;
    if !generation.generate_key_pair() || !signing.sign() || !signing.verify() {
        return Err(SignerError::Pkcs11(
            "token does not provide required Ed25519 generation/sign/verify mechanisms".into(),
        ));
    }
    Ok((context, slot))
}

fn login(context: &Pkcs11, slot: Slot, pin: &[u8], writable: bool) -> SignerResult<Session> {
    let pin = auth_pin(pin)?;
    let session = if writable {
        context.open_rw_session(slot)
    } else {
        context.open_ro_session(slot)
    }
    .map_err(pkcs11_error)?;
    session
        .login(UserType::User, Some(&pin))
        .map_err(pkcs11_error)?;
    Ok(session)
}

fn auth_pin(pin: &[u8]) -> SignerResult<AuthPin> {
    let pin = std::str::from_utf8(pin)
        .map_err(|_| SignerError::SecureFile("PIN must be UTF-8".into()))?;
    Ok(AuthPin::new(pin.to_owned().into()))
}

fn require_id_and_label_absent(session: &Session, key: &KeyConfig) -> SignerResult<()> {
    let by_id = session
        .find_objects(&[Attribute::Id(key.object_id()?)])
        .map_err(pkcs11_error)?;
    let by_label = session
        .find_objects(&[Attribute::Label(key.kid.as_bytes().to_vec())])
        .map_err(pkcs11_error)?;
    if !by_id.is_empty() || !by_label.is_empty() {
        return Err(SignerError::Pkcs11(
            "provisioning refuses to overwrite an existing object id or label".into(),
        ));
    }
    Ok(())
}

fn load_pair(session: &Session, key: &KeyConfig) -> SignerResult<KeyPair> {
    let id = key.object_id()?;
    let objects = session
        .find_objects(&[Attribute::Id(id.clone())])
        .map_err(pkcs11_error)?;
    let labeled = session
        .find_objects(&[Attribute::Label(key.kid.as_bytes().to_vec())])
        .map_err(pkcs11_error)?;
    if objects.len() != 2
        || labeled.len() != 2
        || objects.iter().any(|item| !labeled.contains(item))
    {
        return Err(SignerError::Pkcs11(
            "each configured object id and label must identify one isolated key pair".into(),
        ));
    }
    let public = find_class(session, &id, ObjectClass::PUBLIC_KEY)?;
    let private = find_class(session, &id, ObjectClass::PRIVATE_KEY)?;
    let public_attrs = session
        .get_attributes(
            public,
            &[
                AttributeType::Class,
                AttributeType::Token,
                AttributeType::Private,
                AttributeType::Label,
                AttributeType::Id,
                AttributeType::KeyType,
                AttributeType::Verify,
                AttributeType::Derive,
                AttributeType::EcParams,
                AttributeType::EcPoint,
                AttributeType::Local,
                AttributeType::KeyGenMechanism,
            ],
        )
        .map_err(pkcs11_error)?;
    require_attributes(
        &public_attrs,
        &[
            Attribute::Class(ObjectClass::PUBLIC_KEY),
            Attribute::Token(true),
            Attribute::Private(false),
            Attribute::Label(key.kid.as_bytes().to_vec()),
            Attribute::Id(id.clone()),
            Attribute::KeyType(KeyType::EC_EDWARDS),
            Attribute::Verify(true),
            Attribute::Derive(false),
            Attribute::EcParams(ED25519_EC_PARAMS.to_vec()),
            Attribute::Local(true),
            Attribute::KeyGenMechanism(MechanismType::ECC_EDWARDS_KEY_PAIR_GEN),
        ],
    )?;
    let private_attrs = session
        .get_attributes(
            private,
            &[
                AttributeType::Class,
                AttributeType::Token,
                AttributeType::Private,
                AttributeType::Label,
                AttributeType::Id,
                AttributeType::KeyType,
                AttributeType::Sensitive,
                AttributeType::Extractable,
                AttributeType::AlwaysSensitive,
                AttributeType::NeverExtractable,
                AttributeType::Sign,
                AttributeType::Derive,
                AttributeType::Local,
                AttributeType::KeyGenMechanism,
            ],
        )
        .map_err(pkcs11_error)?;
    require_attributes(
        &private_attrs,
        &[
            Attribute::Class(ObjectClass::PRIVATE_KEY),
            Attribute::Token(true),
            Attribute::Private(true),
            Attribute::Label(key.kid.as_bytes().to_vec()),
            Attribute::Id(id),
            Attribute::KeyType(KeyType::EC_EDWARDS),
            Attribute::Sensitive(true),
            Attribute::Extractable(false),
            Attribute::AlwaysSensitive(true),
            Attribute::NeverExtractable(true),
            Attribute::Sign(true),
            Attribute::Derive(false),
            Attribute::Local(true),
            Attribute::KeyGenMechanism(MechanismType::ECC_EDWARDS_KEY_PAIR_GEN),
        ],
    )?;
    let point = public_attrs
        .iter()
        .find_map(|attribute| match attribute {
            Attribute::EcPoint(value) => Some(value.as_slice()),
            _ => None,
        })
        .ok_or_else(|| SignerError::Pkcs11("public key has no CKA_EC_POINT".into()))?;
    let bytes = decode_ed25519_point(point)?;
    let public = VerifyingKey::from_bytes(&bytes)
        .map_err(|_| SignerError::Pkcs11("public key is not a valid Ed25519 point".into()))?;
    Ok(KeyPair { private, public })
}

fn find_class(session: &Session, id: &[u8], class: ObjectClass) -> SignerResult<ObjectHandle> {
    let objects = session
        .find_objects(&[Attribute::Id(id.to_vec()), Attribute::Class(class)])
        .map_err(pkcs11_error)?;
    if objects.len() != 1 {
        return Err(SignerError::Pkcs11(
            "object id must select exactly one public and one private object".into(),
        ));
    }
    Ok(objects[0])
}

fn require_attributes(actual: &[Attribute], required: &[Attribute]) -> SignerResult<()> {
    if required.iter().any(|attribute| !actual.contains(attribute)) {
        return Err(SignerError::Pkcs11(
            "key attributes do not satisfy the non-extractable locator signing profile".into(),
        ));
    }
    Ok(())
}

fn decode_ed25519_point(point: &[u8]) -> SignerResult<[u8; 32]> {
    let raw = match point {
        [0x04, 0x20, raw @ ..] if raw.len() == 32 => raw,
        raw if raw.len() == 32 => raw,
        _ => {
            return Err(SignerError::Pkcs11(
                "CKA_EC_POINT is not a canonical Ed25519 public key".into(),
            ))
        }
    };
    raw.try_into()
        .map_err(|_| SignerError::Pkcs11("invalid Ed25519 public key length".into()))
}

fn sign_and_verify(
    session: &Session,
    private: ObjectHandle,
    public: &VerifyingKey,
    message: &[u8],
) -> SignerResult<[u8; 64]> {
    let params = EddsaParams::new(EddsaSignatureScheme::Ed25519);
    let signature = session
        .sign(&Mechanism::Eddsa(params), private, message)
        .map_err(pkcs11_error)?;
    let signature = Signature::from_slice(&signature)
        .map_err(|_| SignerError::Pkcs11("HSM returned a non-Ed25519 signature".into()))?;
    public
        .verify_strict(message, &signature)
        .map_err(|_| SignerError::Pkcs11("HSM signature failed software verification".into()))?;
    Ok(signature.to_bytes())
}

fn public_record(key: &KeyConfig, public: &VerifyingKey) -> PublicKeyRecord {
    PublicKeyRecord {
        kid: key.kid.clone(),
        public_key_ed25519: URL_SAFE_NO_PAD.encode(public.to_bytes()),
    }
}

fn pkcs11_error(error: impl std::fmt::Display) -> SignerError {
    SignerError::Pkcs11(error.to_string())
}
