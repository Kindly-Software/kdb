use std::fs;
use std::path::PathBuf;

use atomic_ledger_entry::{derive_genesis_hash, verify_chain, AleEntry, AleKey, VerifyError};
use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum InputFormat {
    RawLe,
    RawBe,
    Hex,
}

#[derive(Parser, Debug)]
#[command(
    name = "alechk",
    about = "Verify ALE-128 ledger files",
    version,
    disable_help_subcommand = true
)]
struct Args {
    /// Path to the ledger file (raw 16-byte entries by default)
    #[arg(value_name = "LEDGER")]
    ledger: PathBuf,

    /// Secret key used for the ledger (hex-encoded)
    #[arg(long, conflicts_with = "key_file")]
    key: Option<String>,

    /// Path to a file containing the secret key (raw or hex)
    #[arg(long, value_name = "PATH", conflicts_with = "key")]
    key_file: Option<PathBuf>,

    /// Context string used to derive the genesis hash
    #[arg(long = "context", default_value = "ALE|day|stream|boot")]
    genesis_context: String,

    /// Input format (raw-le, raw-be, or hex lines)
    #[arg(long, value_enum, default_value_t = InputFormat::RawLe)]
    format: InputFormat,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = Args::parse();
    let key = load_key(&args)?;
    let genesis = derive_genesis_hash(&key, args.genesis_context.as_bytes());
    let entries = load_entries(&args)?;
    if entries.is_empty() {
        println!("ok: ledger empty (genesis hash 0x{genesis:016x})");
        return Ok(());
    }
    match verify_chain(&entries, &key, genesis) {
        Ok(()) => {
            let tail = AleEntry::from(*entries.last().unwrap());
            println!(
                "ok: verified {count} entries (tail prev_hash=0x{hash:016x}, seq={seq})",
                count = entries.len(),
                hash = tail.prev_hash(),
                seq = tail.meta().seq,
            );
            Ok(())
        }
        Err(VerifyError::Chain(mismatch)) => {
            let entry = entries.get(mismatch.index).copied().map(AleEntry::from);
            if let Some(entry) = entry {
                Err(format!(
                    "chain mismatch at #{idx}: expected 0x{exp:016x}, found 0x{act:016x}; meta={meta:?}",
                    idx = mismatch.index,
                    exp = mismatch.expected,
                    act = mismatch.actual,
                    meta = entry.meta(),
                ))
            } else {
                Err(format!(
                    "chain mismatch at #{idx}: expected 0x{exp:016x}, found 0x{act:016x}",
                    idx = mismatch.index,
                    exp = mismatch.expected,
                    act = mismatch.actual,
                ))
            }
        }
        Err(VerifyError::Sequence(gap)) => {
            let entry = entries.get(gap.index).copied().map(AleEntry::from);
            if let Some(entry) = entry {
                Err(format!(
                    "sequence gap at #{idx}: expected {exp}, found {act}; meta={meta:?}",
                    idx = gap.index,
                    exp = gap.expected,
                    act = gap.actual,
                    meta = entry.meta(),
                ))
            } else {
                Err(format!(
                    "sequence gap at #{idx}: expected {exp}, found {act}",
                    idx = gap.index,
                    exp = gap.expected,
                    act = gap.actual,
                ))
            }
        }
    }
}

fn load_key(args: &Args) -> Result<AleKey, String> {
    let bytes = if let Some(hex) = &args.key {
        decode_hex(hex.as_str())?
    } else if let Some(path) = &args.key_file {
        let data = fs::read(path).map_err(|e| format!("failed to read key file: {e}"))?;
        parse_key_blob(&data)?
    } else {
        return Err("either --key or --key-file must be provided".into());
    };
    AleKey::from_slice(&bytes).map_err(|e| format!("invalid key length: {e:?}"))
}

fn load_entries(args: &Args) -> Result<Vec<u128>, String> {
    let data = fs::read(&args.ledger).map_err(|e| format!("failed to read ledger: {e}"))?;
    match args.format {
        InputFormat::RawLe => parse_raw(&data, true),
        InputFormat::RawBe => parse_raw(&data, false),
        InputFormat::Hex => parse_hex_lines(&data),
    }
}

fn parse_raw(data: &[u8], little_endian: bool) -> Result<Vec<u128>, String> {
    if data.len() % 16 != 0 {
        return Err(format!(
            "raw input length must be multiple of 16 bytes (got {})",
            data.len()
        ));
    }
    let mut out = Vec::with_capacity(data.len() / 16);
    for chunk in data.chunks_exact(16) {
        let mut buf = [0u8; 16];
        buf.copy_from_slice(chunk);
        let value = if little_endian {
            u128::from_le_bytes(buf)
        } else {
            u128::from_be_bytes(buf)
        };
        out.push(value);
    }
    Ok(out)
}

fn parse_hex_lines(data: &[u8]) -> Result<Vec<u128>, String> {
    let text =
        std::str::from_utf8(data).map_err(|e| format!("hex input must be valid UTF-8: {e}"))?;
    let mut out = Vec::new();
    for (idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let bytes = decode_hex(trimmed)?;
        if bytes.len() != 16 {
            return Err(format!(
                "line {}: expected 16 bytes (32 hex chars), found {}",
                idx + 1,
                bytes.len()
            ));
        }
        let mut buf = [0u8; 16];
        buf.copy_from_slice(&bytes);
        out.push(u128::from_be_bytes(buf));
    }
    Ok(out)
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let mut buf = Vec::with_capacity(input.len() / 2);
    let mut chars = input.chars().filter(|c| !c.is_ascii_whitespace());
    loop {
        let high = match chars.next() {
            Some(c) => c,
            None => break,
        };
        let low = chars
            .next()
            .ok_or_else(|| "hex string has odd length".to_string())?;
        let byte = ((hex_value(high)? as u8) << 4) | hex_value(low)? as u8;
        buf.push(byte);
    }
    Ok(buf)
}

fn hex_value(c: char) -> Result<u8, String> {
    c.to_digit(16)
        .map(|d| d as u8)
        .ok_or_else(|| format!("invalid hex character: {c}"))
}

fn parse_key_blob(data: &[u8]) -> Result<Vec<u8>, String> {
    if data.is_empty() {
        return Err("key file empty".into());
    }
    if data
        .iter()
        .all(|b| b.is_ascii_hexdigit() || b.is_ascii_whitespace())
    {
        let text = std::str::from_utf8(data).map_err(|e| format!("invalid key text: {e}"))?;
        return decode_hex(text.trim());
    }
    Ok(data.to_vec())
}
