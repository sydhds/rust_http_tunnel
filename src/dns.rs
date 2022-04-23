// std
use std::io::{Error, ErrorKind};
use std::net::SocketAddr;

// third parties
use tokio::io;

use async_trait::async_trait;

// internal

// Dns Resolver
// Tunnel will first recv an http trame like: "CONNECT URL:PORT HTTP/1.1\r\n"
// using dns to: URL => IP

#[async_trait]
pub trait DnsResolver {
    async fn resolve(&mut self, target: &str) -> io::Result<SocketAddr>;
}

#[derive(Clone)]
pub struct SimpleDnsResolver {}

#[async_trait]
impl DnsResolver for SimpleDnsResolver {
    // TODO: generic str param?
    async fn resolve(&mut self, target: &str) -> io::Result<SocketAddr> {
        let resolved: Vec<SocketAddr> = SimpleDnsResolver::resolve(target).await?;
        // Note: not sure if resolved can be an empty vec
        match resolved.get(0) {
            Some(r) => Ok(*r),
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData, "Empty resolve".to_string())),
        }
    }
}

impl SimpleDnsResolver {
    pub fn new() -> Self {
        Self {}
    }

    async fn resolve(target: &str) -> io::Result<Vec<SocketAddr>> {

        let resolved: Vec<_> = tokio::net::lookup_host(target).await?.collect();
        if resolved.is_empty() {
            return Err(Error::from(ErrorKind::AddrNotAvailable));
        }
        Ok(resolved)
    }
}

// End Dns Resolver


#[cfg(test)]
mod tests {

    use crate::dns::SimpleDnsResolver;
    use crate::dns::DnsResolver;

    #[tokio::test]
    async fn test_dns_resolve_ok() -> Result<(), std::io::Error> {

        let mut dns_r = SimpleDnsResolver::new();
        let res = dns_r.resolve("google.com:80").await?;
        assert!(res.to_string().is_empty() == false);
        Ok(())
    }

    #[tokio::test]
    async fn test_dns_resolve_error_missing_port() {

        let mut dns_r = SimpleDnsResolver::new();
        // Note: missing port
        match dns_r.resolve("google.com").await {
            Ok(_) => panic!("Unexpected!"),
            Err(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::InvalidInput);
            }
        }
    }

    #[tokio::test]
    async fn test_dns_resolve_error_unknown_url() {
        let mut dns_r = SimpleDnsResolver::new();
        match dns_r.resolve("http://fooooooooooooooooooooooooooo.com:80").await {
            Ok(_) => panic!("Unexpected!"),
            Err(e) => {
                // Return "Uncategorized" error kind that cannot be matched...
                // assert_eq!(e.kind(), std::io::ErrorKind::Other);
                assert!(true);
            }
        }
    }
}
