use near_sdk::borsh::{BorshDeserialize, BorshSerialize};
use near_sdk::serde::{Deserialize, Serialize};

const PREFIX: &str = "ed25519:";

pub struct Signature {
    signature: [u8; 64],
}

impl AsRef<[u8; 64]> for Signature {
    fn as_ref(&self) -> &[u8; 64] {
        &self.signature
    }
}

impl BorshSerialize for Signature {
    fn serialize<W: std::io::Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(&self.signature)
    }
}

impl BorshDeserialize for Signature {
    fn deserialize_reader<R: std::io::Read>(reader: &mut R) -> std::io::Result<Self> {
        let mut signature = [0u8; 64];
        reader.read_exact(&mut signature)?;
        Ok(Signature { signature })
    }
}

impl Serialize for Signature {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: near_sdk::serde::Serializer,
    {
        serializer.serialize_str(
            [
                PREFIX,
                bs58::encode(self.signature.as_ref()).into_string().as_str(),
            ]
            .concat()
            .as_str(),
        )
    }
}

impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: near_sdk::serde::Deserializer<'de>,
    {
        let s = <std::string::String as Deserialize>::deserialize(deserializer)?;
        if !s.starts_with(PREFIX) {
            return Err(near_sdk::serde::de::Error::custom(
                "Invalid signature prefix",
            ));
        }
        let s = s.trim_start_matches(PREFIX);
        let signature = bs58::decode(s)
            .into_vec()
            .map_err(near_sdk::serde::de::Error::custom)?;
        Ok(Signature {
            signature: signature
                .try_into()
                .map_err(|_| near_sdk::serde::de::Error::custom("Invalid signature length"))?,
        })
    }
}
