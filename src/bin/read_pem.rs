use std::{fs::File, io::BufReader, iter, env};

use rustls_pemfile::{read_one, Item};

fn main() {
    let mut reader = BufReader::new(File::open(
        env::args().nth(1).expect("pem file not provided in args"),
    ).expect("pem file not readable"));

    // Assume `reader` is any std::io::BufRead implementor
    for item in iter::from_fn(|| read_one(&mut reader).transpose()) {
        match item.unwrap() {
            Item::X509Certificate(cert) => println!("certificate {:?}", cert),
            Item::RSAKey(key) => println!("rsa pkcs1 key {:?}", key),
            Item::PKCS8Key(key) => println!("pkcs8 key {:?}", key),
        }
    }
}
