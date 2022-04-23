## Intro

A http tunnel implementation using Rust & Tokio & Rustls

Note: Idea & inspiration -> https://medium.com/swlh/writing-a-modern-http-s-tunnel-in-rust-56e70d898700

Technical details:
* Use tokio-util crate to implement a very basic http codec
* Use rustls for handling https stuff
* Use async trait (async-trait crate)
* Use thiserror crate for easy error handling in codec code

## Howto

### [Tcp]

* Run:
  * `cargo run -- 127.0.0.1:6161`
* Test:
  * `curl http://www.google.com -p -x http://127.0.0.1:6161`

### [Tcp/Tls]

* Setup:
  * `openssl req -x509 -newkey rsa:4096 -keyout server.key -out server.crt -days 365 -sha256 --subj '/CN=localhost/'`
  
  * or
    * `openssl req -x509 -sha256 -newkey rsa:4096 -keyout key.pem -out cert.pem -days 365 --subj '/CN=127.0.0.1/'`
    * `openssl rsa -in key.pem -out key_decrypted.pem`

* Run:
  * `cargo run -- 127.0.0.1:6161 cert.pem key_decrypted.pem`
* Test:
  * `curl -v http://www.google.com -p --proxy-insecure -x https://127.0.0.1:6161`
  * `curl -v http://www.google.com -p --proxy-cacert ./cert.pem -x https://127.0.0.1:6161`

Please note that this can be easily tested with Firefox (Parameters / Proxy settings)

## Unit tests

* `cargo test`