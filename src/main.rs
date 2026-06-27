use crate::tree::{
    build_huffman_tree, decode, encode, generate_codes, load_compressed, save_compressed,
};
use clap::{Args, Parser};
use std::{
    collections::HashMap,
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
};

// mod header;
mod tree;

#[derive(Parser, Debug)]
#[command(name = "hfm", about = "Huffman encoding/decoding for files")]
struct Cli {
    #[command(flatten)]
    mode: Mode,
    source: PathBuf,
    output: PathBuf,
}

#[derive(Args, Debug)]
#[group(required = true, multiple = false)]
struct Mode {
    #[arg(short = 'e', long)]
    encode: bool,
    #[arg(short = 'd', long)]
    decode: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Cli::parse();

    if args.mode.encode {
        let data = fs::read(args.source)?;
        let tree = build_huffman_tree(&data).expect("Empty text");

        let mut encode_map = HashMap::new();
        generate_codes(&tree, "".into(), &mut encode_map);

        let encoded_bits = encode(&data, &encode_map);
        save_compressed(args.output.to_str().unwrap(), &encoded_bits, &encode_map)?;
    } else {
        let (decode_map, bits, total_bits) = load_compressed(args.source.to_str().unwrap())?;
        let decoded = decode(&bits.clone().into_vec(), total_bits, &decode_map);

        let mut file = BufWriter::new(File::create(args.output)?);
        file.write_all(&decoded)?;
        file.flush()?;
    }

    Ok(())
}
