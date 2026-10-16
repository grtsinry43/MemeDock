fn main() -> Result<(), Box<dyn std::error::Error>> {
    uniffi_bindgen_cs::main()?;
    Ok(())
}
