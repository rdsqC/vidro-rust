use std::fs::File;
use std::io::{BufReader, BufWriter};

use crate::eval::AiModel;

pub fn save_model(model: &AiModel, file_path: &str) -> std::io::Result<()> {
    let file = File::create(file_path)?;
    let writer = BufWriter::new(file);

    bincode::serialize_into(writer, model)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    println!("Saved model: {}", file_path);
    Ok(())
}

pub fn load_model(file_path: &str) -> std::io::Result<AiModel> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let model: AiModel = bincode::deserialize_from(reader)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

    println!("Loaded model");
    Ok(model)
}

