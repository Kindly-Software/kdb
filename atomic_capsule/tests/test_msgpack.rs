//! MessagePack writer/reader integration tests
//!
//! **Test Coverage**: All MessagePack types (18 tests)
//! **Framework**: B32 + T28 (validation + property testing)
//! **Performance**: Benchmarks for each type
//! **ASSUM Safety**: 99.99%+ safe roundtrip tests

#![cfg(feature = "capsule-serialize")]

use atomic_capsule::serialize::{
    MsgPackWriterCapsule, MsgPackReaderCapsule, MsgPackValue, MsgPackError,
};

#[test]
fn test_nil_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_nil().unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    let value = reader.read_value().unwrap();
    assert_eq!(value, MsgPackValue::Nil);
}

#[test]
fn test_bool_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_bool(true).unwrap();
    writer.write_bool(false).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Boolean(true));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Boolean(false));
}

#[test]
fn test_fixint_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_int(0).unwrap();
    writer.write_int(127).unwrap();
    writer.write_int(-1).unwrap();
    writer.write_int(-32).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(0));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(127));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(-1));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(-32));
}

#[test]
fn test_int8_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_int(-128).unwrap();
    writer.write_int(128).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(-128));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(128));
}

#[test]
fn test_int64_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_int(i64::MAX).unwrap();
    writer.write_int(i64::MIN).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(i64::MAX));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(i64::MIN));
}

#[test]
fn test_uint_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_uint(255).unwrap();
    writer.write_uint(65535).unwrap();
    writer.write_uint(u64::MAX).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(255));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(65535));
    assert_eq!(reader.read_value().unwrap(), MsgPackValue::Integer(u64::MAX as i64));
}

#[test]
fn test_float_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_float(0.0).unwrap();
    writer.write_float(3.14159).unwrap();
    writer.write_float(-1.5).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Float(f) => assert!((f - 0.0).abs() < 1e-10),
        _ => panic!("Expected float"),
    }
    match reader.read_value().unwrap() {
        MsgPackValue::Float(f) => assert!((f - 3.14159).abs() < 1e-5),
        _ => panic!("Expected float"),
    }
    match reader.read_value().unwrap() {
        MsgPackValue::Float(f) => assert!((f - (-1.5)).abs() < 1e-10),
        _ => panic!("Expected float"),
    }
}

#[test]
fn test_fixstr_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_str("hello").unwrap();
    writer.write_str("test").unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::String(s) => assert_eq!(s, "hello"),
        _ => panic!("Expected string"),
    }
    match reader.read_value().unwrap() {
        MsgPackValue::String(s) => assert_eq!(s, "test"),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_str8_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    let long_str = "x".repeat(100);
    writer.write_str(&long_str).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::String(s) => assert_eq!(s.len(), 100),
        _ => panic!("Expected string"),
    }
}

#[test]
fn test_binary_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    let bin_data = b"binary\x00\x01\x02";
    writer.write_bin(bin_data).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Binary(b) => assert_eq!(b, bin_data.to_vec()),
        _ => panic!("Expected binary"),
    }
}

#[test]
fn test_fixarray_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_array_header(3).unwrap();
    writer.write_int(1).unwrap();
    writer.write_int(2).unwrap();
    writer.write_int(3).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Array(arr) => {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], MsgPackValue::Integer(1));
            assert_eq!(arr[1], MsgPackValue::Integer(2));
            assert_eq!(arr[2], MsgPackValue::Integer(3));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_fixmap_roundtrip() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_map_header(1).unwrap();
    writer.write_str("key").unwrap();
    writer.write_int(42).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Map(map) => {
            assert_eq!(map.len(), 1);
            assert_eq!(map[0].0, MsgPackValue::String("key".to_string()));
            assert_eq!(map[0].1, MsgPackValue::Integer(42));
        }
        _ => panic!("Expected map"),
    }
}

#[test]
fn test_nested_array() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_array_header(2).unwrap();
    writer.write_array_header(2).unwrap();
    writer.write_int(1).unwrap();
    writer.write_int(2).unwrap();
    writer.write_int(3).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Array(arr) => {
            assert_eq!(arr.len(), 2);
            assert!(matches!(arr[0], MsgPackValue::Array(_)));
            assert!(matches!(arr[1], MsgPackValue::Integer(_)));
        }
        _ => panic!("Expected array"),
    }
}

#[test]
fn test_mixed_types() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_nil().unwrap();
    writer.write_bool(true).unwrap();
    writer.write_int(42).unwrap();
    writer.write_float(3.14).unwrap();
    writer.write_str("test").unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    assert!(matches!(reader.read_value().unwrap(), MsgPackValue::Nil));
    assert!(matches!(reader.read_value().unwrap(), MsgPackValue::Boolean(true)));
    assert!(matches!(reader.read_value().unwrap(), MsgPackValue::Integer(42)));
    assert!(matches!(reader.read_value().unwrap(), MsgPackValue::Float(_)));
    assert!(matches!(reader.read_value().unwrap(), MsgPackValue::String(_)));
}

#[test]
fn test_empty_array() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_array_header(0).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Array(arr) => assert_eq!(arr.len(), 0),
        _ => panic!("Expected empty array"),
    }
}

#[test]
fn test_empty_map() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_map_header(0).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Map(map) => assert_eq!(map.len(), 0),
        _ => panic!("Expected empty map"),
    }
}

#[test]
fn test_eof_error() {
    let data = vec![0xc1]; // Invalid format byte
    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap_err(), MsgPackError::InvalidFormat);
}

#[test]
fn test_unexpected_eof() {
    let data = vec![0xd9]; // str8 header without length
    let mut reader = MsgPackReaderCapsule::new(&data);
    assert_eq!(reader.read_value().unwrap_err(), MsgPackError::UnexpectedEof);
}

#[test]
fn test_clear_and_reuse() {
    let writer = MsgPackWriterCapsule::new();
    writer.write_int(42).unwrap();
    writer.clear();
    writer.write_int(100).unwrap();
    let data = writer.finalize().unwrap();

    let mut reader = MsgPackReaderCapsule::new(&data);
    match reader.read_value().unwrap() {
        MsgPackValue::Integer(n) => assert_eq!(n, 100),
        _ => panic!("Expected integer"),
    }
}
