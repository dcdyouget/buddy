//! Verify a release artifact with the same public key and verifier as the client.
use buddy_update::{PUBLIC_KEY, UpdateError};

#[path = "../src/verify.rs"]
mod verify;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    if arguments.len() != 2 {
        return Err("用法：verify-artifact <制品文件> <签名文件>".into());
    }
    let data = std::fs::read(&arguments[0])?;
    let signature = std::fs::read_to_string(&arguments[1])?;
    verify::verify_signature(&data, &signature, PUBLIC_KEY)?;
    println!("制品签名校验通过：{}", arguments[0].to_string_lossy());
    Ok(())
}
