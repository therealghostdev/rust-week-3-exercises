use serde::{Deserialize, Serialize};
use std::fmt;
use std::ops::Deref;
use std::convert::TryInto;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct CompactSize {
    pub value: u64,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum BitcoinError {
    InsufficientBytes,
    InvalidFormat,
}

impl CompactSize {
    pub fn new(value: u64) -> Self {
        Self { value }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let v = self.value;
        if v <= 0xFC {
            vec![v as u8]
        } else if v <= 0xFFFF {
            let mut res = vec![0xFD];
            res.extend(&(v as u16).to_le_bytes());
            res
        } else if v <= 0xFFFF_FFFF {
            let mut res = vec![0xFE];
            res.extend(&(v as u32).to_le_bytes());
            res
        } else {
            let mut res = vec![0xFF];
            res.extend(&(v as u64).to_le_bytes());
            res
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.is_empty() {
            return Err(BitcoinError::InsufficientBytes);
        }
        match bytes[0] {
            n @ 0x00..=0xFC => Ok((CompactSize::new(n as u64), 1)),
            0xFD => {
                if bytes.len() < 3 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u16::from_le_bytes(bytes[1..3].try_into().unwrap()) as u64;
                Ok((CompactSize::new(val), 3))
            }
            0xFE => {
                if bytes.len() < 5 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u32::from_le_bytes(bytes[1..5].try_into().unwrap()) as u64;
                Ok((CompactSize::new(val), 5))
            }
            0xFF => {
                if bytes.len() < 9 {
                    return Err(BitcoinError::InsufficientBytes);
                }
                let val = u64::from_le_bytes(bytes[1..9].try_into().unwrap());
                Ok((CompactSize::new(val), 9))
            }
            _ => Err(BitcoinError::InvalidFormat),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Txid(pub [u8; 32]);

impl Serialize for Txid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: serde::Serializer {
        serializer.serialize_str(&hex::encode(&self.0))
    }
}

impl<'de> Deserialize<'de> for Txid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where D: serde::Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        let decoded = hex::decode(&s).map_err(serde::de::Error::custom)?;
        if decoded.len() != 32 {
            return Err(serde::de::Error::custom("Invalid txid length"));
        }
        let arr: [u8; 32] = decoded.try_into().unwrap();
        Ok(Txid(arr))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct OutPoint {
    pub txid: Txid,
    pub vout: u32,
}

impl OutPoint {
    pub fn new(txid: [u8; 32], vout: u32) -> Self {
        Self { txid: Txid(txid), vout }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = Vec::new();
        res.extend(&self.txid.0);
        res.extend(&self.vout.to_le_bytes());
        res
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 36 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let txid: [u8; 32] = bytes[0..32].try_into().unwrap();
        let vout = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
        Ok((OutPoint::new(txid, vout), 36))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Script {
    pub bytes: Vec<u8>,
}

impl Script {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = CompactSize::new(self.bytes.len() as u64).to_bytes();
        res.extend(&self.bytes);
        res
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (cs, used) = CompactSize::from_bytes(bytes)?;
        let len = cs.value as usize;
        if bytes.len() < used + len {
            return Err(BitcoinError::InsufficientBytes);
        }
        let script_bytes = bytes[used..used+len].to_vec();
        Ok((Script::new(script_bytes), used+len))
    }
}

impl Deref for Script {
    type Target = Vec<u8>;
    fn deref(&self) -> &Self::Target {
        &self.bytes
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct TransactionInput {
    pub previous_output: OutPoint,
    pub script_sig: Script,
    pub sequence: u32,
}

impl TransactionInput {
    pub fn new(previous_output: OutPoint, script_sig: Script, sequence: u32) -> Self {
        Self { previous_output, script_sig, sequence }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = self.previous_output.to_bytes();
        res.extend(self.script_sig.to_bytes());
        res.extend(&self.sequence.to_le_bytes());
        res
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        let (outpoint, used1) = OutPoint::from_bytes(bytes)?;
        let (script, used2) = Script::from_bytes(&bytes[used1..])?;
        if bytes.len() < used1 + used2 + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let sequence = u32::from_le_bytes(bytes[used1+used2..used1+used2+4].try_into().unwrap());
        Ok((
            TransactionInput::new(outpoint, script, sequence),
            used1 + used2 + 4
        ))
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct BitcoinTransaction {
    pub version: u32,
    pub inputs: Vec<TransactionInput>,
    pub lock_time: u32,
}

impl BitcoinTransaction {
    pub fn new(version: u32, inputs: Vec<TransactionInput>, lock_time: u32) -> Self {
        Self { version, inputs, lock_time }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut res = self.version.to_le_bytes().to_vec();
        res.extend(CompactSize::new(self.inputs.len() as u64).to_bytes());
        for inp in &self.inputs {
            res.extend(inp.to_bytes());
        }
        res.extend(&self.lock_time.to_le_bytes());
        res
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<(Self, usize), BitcoinError> {
        if bytes.len() < 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let version = u32::from_le_bytes(bytes[0..4].try_into().unwrap());
        let (cs, used1) = CompactSize::from_bytes(&bytes[4..])?;
        let mut offset = 4 + used1;
        let mut inputs = Vec::new();
        for _ in 0..cs.value {
            let (input, used) = TransactionInput::from_bytes(&bytes[offset..])?;
            inputs.push(input);
            offset += used;
        }
        if bytes.len() < offset + 4 {
            return Err(BitcoinError::InsufficientBytes);
        }
        let lock_time = u32::from_le_bytes(bytes[offset..offset+4].try_into().unwrap());
        Ok((
            BitcoinTransaction::new(version, inputs, lock_time),
            offset+4
        ))
    }
}

impl fmt::Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Version: {}", self.version)?;
        for inp in &self.inputs {
            writeln!(f, "Previous Output Vout: {}", inp.previous_output.vout)?;
            writeln!(f, "ScriptSig Length: {}", inp.script_sig.bytes.len())?;
            writeln!(f, "ScriptSig Bytes: {:?}", inp.script_sig.bytes)?;
        }
        writeln!(f, "Lock Time: {}", self.lock_time)
    }
}
