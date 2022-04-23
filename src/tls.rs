use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use rustls_pemfile::{certs, rsa_private_keys};
use tokio_rustls::rustls::{Certificate, PrivateKey};

pub fn load_certs<P>(path: P) -> std::io::Result<Vec<Certificate>> where P: AsRef<Path> {
    certs(&mut BufReader::new(File::open(path.as_ref())?))
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid certificate")
        })
        .map(|mut certs| certs.drain(..).map(Certificate).collect())
}

pub fn load_keys<P>(path: P) -> std::io::Result<Vec<PrivateKey>> where P: AsRef<Path> {

    // println!("{:?}", rsa_private_keys(&mut BufReader::new(File::open(path.as_ref())?)));
    rsa_private_keys(&mut BufReader::new(File::open(path.as_ref())?))
        .map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid key")
        })
        .map(|mut keys| keys.drain(..).map(PrivateKey).collect())
}