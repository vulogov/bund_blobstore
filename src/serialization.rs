use crate::BlobStore;
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

/// Serialization format options
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SerializationFormat {
    Bincode,     // Fast, compact binary format
    Json,        // Human-readable, larger
    MessagePack, // Efficient binary format
    Cbor,        // CBOR format
}

/// Helper for serialization/deserialization operations
pub struct SerializationHelper;

impl SerializationHelper {
    /// Serialize any serializable type to Vec<u8>
    pub fn serialize<T: Serialize>(
        value: &T,
        format: SerializationFormat,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        match format {
            SerializationFormat::Bincode => Ok(bincode::serialize(value)?),
            SerializationFormat::Json => Ok(serde_json::to_vec(value)?),
            SerializationFormat::MessagePack => Ok(rmp_serde::to_vec(value)?),
            SerializationFormat::Cbor => Ok(serde_cbor::to_vec(value)?),
        }
    }

    /// Deserialize from bytes to any deserializable type
    pub fn deserialize<T: for<'de> Deserialize<'de>>(
        data: &[u8],
        format: SerializationFormat,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        match format {
            SerializationFormat::Bincode => Ok(bincode::deserialize(data)?),
            SerializationFormat::Json => Ok(serde_json::from_slice(data)?),
            SerializationFormat::MessagePack => Ok(rmp_serde::from_slice(data)?),
            SerializationFormat::Cbor => Ok(serde_cbor::from_slice(data)?),
        }
    }

    /// Compress data using zlib
    pub fn compress(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(data)?;
        Ok(encoder.finish()?)
    }

    /// Decompress zlib-compressed data
    pub fn decompress(data: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let mut decoder = ZlibDecoder::new(data);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed)?;
        Ok(decompressed)
    }

    /// Serialize with compression
    pub fn serialize_compressed<T: Serialize>(
        value: &T,
        format: SerializationFormat,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error + Send + Sync>> {
        let serialized = Self::serialize(value, format)?;
        Ok(Self::compress(&serialized)?)
    }

    /// Deserialize with decompression
    pub fn deserialize_compressed<T: for<'de> Deserialize<'de>>(
        data: &[u8],
        format: SerializationFormat,
    ) -> Result<T, Box<dyn std::error::Error + Send + Sync>> {
        let decompressed = Self::decompress(data)?;
        Ok(Self::deserialize(&decompressed, format)?)
    }

    /// Store a serialized object directly to blob store with optional compression
    pub fn store_serialized<T: Serialize>(
        store: &mut BlobStore,
        key: &str,
        value: &T,
        format: SerializationFormat,
        compressed: bool,
        prefix: Option<&str>,
    ) -> Result<(), redb::Error> {
        let data = if compressed {
            Self::serialize_compressed(value, format).map_err(|e| {
                redb::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?
        } else {
            Self::serialize(value, format).map_err(|e| {
                redb::Error::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    e.to_string(),
                ))
            })?
        };

        store.put(key, &data, prefix)
    }

    /// Load and deserialize an object from blob store
    pub fn load_deserialized<T: for<'de> Deserialize<'de>>(
        store: &BlobStore,
        key: &str,
        format: SerializationFormat,
        compressed: bool,
    ) -> Result<Option<T>, Box<dyn std::error::Error + Send + Sync>> {
        if let Some(data) = store.get(key)? {
            let deserialized = if compressed {
                Self::deserialize_compressed(&data, format)?
            } else {
                Self::deserialize(&data, format)?
            };
            Ok(Some(deserialized))
        } else {
            Ok(None)
        }
    }
}
