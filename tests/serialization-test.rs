use bund_blobstore::{SerializationFormat, SerializationHelper};

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestStruct {
        id: u32,
        name: String,
        data: Vec<u8>,
    }

    #[test]
    fn test_serialization_roundtrip() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let original = TestStruct {
            id: 42,
            name: "test".to_string(),
            data: vec![1, 2, 3],
        };

        for format in [
            SerializationFormat::Bincode,
            SerializationFormat::Json,
            SerializationFormat::MessagePack,
            SerializationFormat::Cbor,
        ] {
            let serialized = SerializationHelper::serialize(&original, format)?;
            let deserialized: TestStruct = SerializationHelper::deserialize(&serialized, format)?;
            assert_eq!(original, deserialized);
        }

        Ok(())
    }

    #[test]
    fn test_compression() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let data = vec![0u8; 10000]; // 10KB of zeros
        let compressed = SerializationHelper::compress(&data)?;
        assert!(compressed.len() < data.len());

        let decompressed = SerializationHelper::decompress(&compressed)?;
        assert_eq!(data, decompressed);

        Ok(())
    }
}
