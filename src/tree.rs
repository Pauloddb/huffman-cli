use std::{
    cmp::Ordering,
    collections::{BinaryHeap, HashMap},
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
};

use bitvec::{order::Msb0, vec::BitVec};

pub type EncodeMap = HashMap<u8, String>; // símbolo → código (para codificar)
pub type DecodeMap = HashMap<String, u8>; // código → símbolo (para decodificar)

#[derive(Debug, Clone)]
pub enum HuffmanNode {
    Internal {
        left: Box<HuffmanNode>,
        right: Box<HuffmanNode>,
        freq: usize,
    },
    Leaf {
        freq: usize,
        symbol: u8,
    },
}

impl HuffmanNode {
    pub fn frequency(&self) -> usize {
        match self {
            HuffmanNode::Leaf { freq, .. } => *freq,
            HuffmanNode::Internal { freq, .. } => *freq,
        }
    }
}

// Implementação de Ord para a min-heap (invertemos porque BinaryHeap é max-heap)
impl PartialEq for HuffmanNode {
    fn eq(&self, other: &Self) -> bool {
        self.frequency() == other.frequency()
    }
}

impl Eq for HuffmanNode {}

impl PartialOrd for HuffmanNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        // Invertemos para ter min-heap
        other.frequency().partial_cmp(&self.frequency())
    }
}

impl Ord for HuffmanNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other.frequency().cmp(&self.frequency())
    }
}

pub fn build_huffman_tree(data: &[u8]) -> Option<HuffmanNode> {
    let mut freq_table = HashMap::<u8, usize>::new();
    for byte in data.iter() {
        *freq_table.entry(*byte).or_insert(0) += 1;
    }

    let mut heap = freq_table
        .into_iter()
        .map(|(symbol, freq)| HuffmanNode::Leaf { freq, symbol })
        .collect::<BinaryHeap<_>>();

    while heap.len() > 1 {
        let left = Box::new(heap.pop().unwrap());
        let right = Box::new(heap.pop().unwrap());
        let freq = left.frequency() + right.frequency();

        heap.push(HuffmanNode::Internal { left, right, freq });
    }
    heap.pop()
}

pub fn generate_codes(node: &HuffmanNode, prefix: String, codes: &mut EncodeMap) {
    match node {
        HuffmanNode::Leaf { symbol, .. } => {
            let p = if !prefix.is_empty() {
                prefix.clone()
            } else {
                "0".to_string()
            };
            codes.insert(*symbol, p.clone());
        }
        HuffmanNode::Internal { left, right, .. } => {
            generate_codes(left, format!("{}0", prefix), codes);
            generate_codes(right, format!("{}1", prefix), codes);
        }
    }
}

pub fn encode(data: &[u8], codes: &EncodeMap) -> BitVec<u8, Msb0> {
    let mut bits = BitVec::new();

    for byte in data.iter() {
        let code = codes
            .get(&byte)
            .expect(&format!("Byte without code: 0x{:02X}", byte));

        for bit_char in code.chars() {
            bits.push(bit_char == '1');
        }
    }

    bits
}

pub fn decode(bytes: &[u8], total_bits: usize, code_map: &DecodeMap) -> Vec<u8> {
    let bits = BitVec::<u8, Msb0>::from_slice(bytes);
    let mut result = vec![];
    let mut buf = String::new();

    for bit in bits.iter().by_vals().take(total_bits) {
        buf.push(if bit { '1' } else { '0' });

        if let Some(&symbol) = code_map.get(&buf) {
            result.push(symbol);
            buf.clear();
        }
    }

    result
}

const MAGIC: &[u8] = b"HUFF";
const VERSION: u8 = 1;

pub fn save_compressed(
    output_path: &str,
    encoded_bits: &BitVec<u8, Msb0>,
    encode_map: &EncodeMap,
) -> anyhow::Result<()> {
    let mut file = BufWriter::new(File::create(output_path)?);

    // ── 1. magic + version (5 bytes) ──
    file.write_all(MAGIC)?;
    file.write_all(&[VERSION])?;

    // ── 2. total_bits (8 bytes, u64) ──
    let total_bits = encoded_bits.len() as u64;
    file.write_all(&total_bits.to_be_bytes())?;

    // ── 3. número de entradas na tabela (2 bytes, u16) ──
    let num_entries = encode_map.len() as u16;
    file.write_all(&num_entries.to_be_bytes())?;

    // ── 4. tabela de códigos ──
    for (symbol, code) in encode_map {
        // [símbolo: u8] — apenas 1 byte, sem variação de tamanho
        file.write_all(&[*symbol])?;

        // [len do código em bits: u8][código como string ASCII '0'/'1']
        file.write_all(&[code.len() as u8])?;
        file.write_all(code.as_bytes())?;
    }

    // ── 4. dados codificados (bytes dos bits) ──
    let bytes = encoded_bits.clone().into_vec();
    file.write_all(&bytes)?;

    file.flush()?;
    Ok(())
}

pub fn load_compressed(input_path: &str) -> anyhow::Result<(DecodeMap, BitVec<u8, Msb0>, usize)> {
    let mut file = BufReader::new(File::open(input_path)?);

    // magic
    let mut buf4 = [0u8; 4];
    file.read_exact(&mut buf4)?;

    if buf4 != MAGIC {
        return Err(anyhow::anyhow!("Invalid magic"));
    }

    // version
    let mut buf1 = [0u8; 1];
    file.read_exact(&mut buf1)?;

    // total_bits
    let mut buf8 = [0u8; 8];
    file.read_exact(&mut buf8)?;
    let total_bits = u64::from_be_bytes(buf8) as usize;

    // num_entries
    let mut buf2 = [0u8; 2];
    file.read_exact(&mut buf2)?;
    let num_entries = u16::from_be_bytes(buf2) as usize;

    // decode map
    let mut decode_map = HashMap::with_capacity(num_entries);

    for _ in 0..num_entries {
        let mut symbol_buf = [0u8; 1];

        // símbolo
        file.read_exact(&mut symbol_buf)?;
        let symbol = symbol_buf[0];

        // len do código
        let mut code_len_buf = [0u8; 1];
        file.read_exact(&mut code_len_buf)?;
        let code_len = code_len_buf[0] as usize;

        let mut code_bytes = vec![0u8; code_len];
        file.read_exact(&mut code_bytes)?;
        let code = String::from_utf8(code_bytes).unwrap();

        decode_map.insert(code, symbol);
    }

    // calcula quantos bytes precisa ler
    let data_bytes_len = (total_bits + 7) / 8;
    let mut data_bytes = vec![0u8; data_bytes_len];
    file.read_exact(&mut data_bytes)?;

    let mut bits = BitVec::<u8, Msb0>::from_vec(data_bytes);
    bits.truncate(total_bits);

    Ok((decode_map, bits, total_bits))
}
